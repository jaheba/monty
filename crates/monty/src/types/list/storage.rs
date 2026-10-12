use std::{mem, ops::Deref};

use monty_types::{ResourceError, ResourceTracker};
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::SerializeSeq};

use crate::{
    heap::{ContainsHeap, DropWithContext, Heap},
    value::{VALUE_SIZE, Value},
};

/// Numeric buffers contain immediate values only. Generic buffers own their values.
#[derive(Debug, Default)]
pub(super) enum Storage {
    #[default]
    Empty,
    Int(Vec<i64>),
    Float(Vec<f64>),
    Generic(Vec<Value>),
}

/// A numeric element materialized on the stack, or a borrowed generic element.
pub(crate) enum Item<'a> {
    Immediate(Value),
    Borrowed(&'a Value),
}

impl Deref for Item<'_> {
    type Target = Value;

    fn deref(&self) -> &Value {
        match self {
            Self::Immediate(value) => value,
            Self::Borrowed(value) => value,
        }
    }
}

impl Storage {
    pub fn new(values: Vec<Value>) -> Self {
        if values.is_empty() {
            Self::Empty
        } else if values.iter().all(|v| matches!(v, Value::Int(_))) {
            let mut ints: Vec<i64> = values
                .into_iter()
                .map(|v| match v {
                    Value::Int(i) => i,
                    _ => unreachable!(),
                })
                .collect();
            ints.shrink_to_fit();
            Self::Int(ints)
        } else if values.iter().all(|v| matches!(v, Value::Float(_))) {
            let mut floats: Vec<f64> = values
                .into_iter()
                .map(|v| match v {
                    Value::Float(f) => f,
                    _ => unreachable!(),
                })
                .collect();
            floats.shrink_to_fit();
            Self::Float(floats)
        } else {
            Self::Generic(values)
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Int(v) => v.len(),
            Self::Float(v) => v.len(),
            Self::Generic(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<Item<'_>> {
        match self {
            Self::Empty => None,
            Self::Int(v) => v.get(index).map(|&i| Item::Immediate(Value::Int(i))),
            Self::Float(v) => v.get(index).map(|&f| Item::Immediate(Value::Float(f))),
            Self::Generic(v) => v.get(index).map(Item::Borrowed),
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = Item<'_>> + DoubleEndedIterator + Clone {
        (0..self.len()).map(|i| self.get(i).expect("index within storage"))
    }

    fn accepts(&self, value: &Value) -> bool {
        match self {
            Self::Empty | Self::Generic(_) => true,
            Self::Int(_) => matches!(value, Value::Int(_)),
            Self::Float(_) => matches!(value, Value::Float(_)),
        }
    }

    pub fn element_size(&self) -> usize {
        match self {
            Self::Empty | Self::Int(_) | Self::Float(_) => 8,
            Self::Generic(_) => VALUE_SIZE,
        }
    }

    fn capacity(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Int(v) => v.capacity(),
            Self::Float(v) => v.capacity(),
            Self::Generic(v) => v.capacity(),
        }
    }

    /// Promotion allocates the entire generic buffer while the numeric buffer is live.
    pub fn check_insert(&self, value: &Value, tracker: &ResourceTracker) -> Result<(), ResourceError> {
        if !self.accepts(value) {
            return tracker.check_allocation(self.len().saturating_add(1).max(4).saturating_mul(VALUE_SIZE));
        }
        let size = if matches!(self, Self::Empty) && !matches!(value, Value::Int(_) | Value::Float(_)) {
            VALUE_SIZE
        } else {
            self.element_size()
        };
        tracker.check_growth(self.len(), self.capacity(), size)
    }

    pub fn check_replace(&self, value: &Value, tracker: &ResourceTracker) -> Result<(), ResourceError> {
        if self.accepts(value) {
            Ok(())
        } else {
            tracker.check_allocation(self.len().max(4).saturating_mul(VALUE_SIZE))
        }
    }

    pub fn check_extend(&self, values: &[Value], tracker: &ResourceTracker) -> Result<(), ResourceError> {
        if values.is_empty() || matches!(self, Self::Empty) {
            // Empty storage takes the admitted vector, compacting numeric values in place.
            return Ok(());
        }
        if values.iter().any(|v| !self.accepts(v)) {
            tracker.check_allocation(
                self.len()
                    .saturating_add(values.len())
                    .max(4)
                    .saturating_mul(VALUE_SIZE),
            )
        } else {
            let additional = self.len().saturating_add(values.len()).saturating_sub(self.capacity());
            tracker.check_pending_allocation(additional.saturating_mul(self.element_size()))
        }
    }

    /// Callers preflight before changing representation; no Python runs during conversion.
    fn promote(&mut self, additional: usize) {
        if matches!(self, Self::Generic(_)) {
            return;
        }
        let mut values = Vec::with_capacity(self.len().saturating_add(additional).max(4));
        match mem::take(self) {
            Self::Int(v) => values.extend(v.into_iter().map(Value::Int)),
            Self::Float(v) => values.extend(v.into_iter().map(Value::Float)),
            Self::Empty => {}
            Self::Generic(_) => unreachable!(),
        }
        *self = Self::Generic(values);
    }

    pub fn push(&mut self, value: Value) {
        if matches!(self, Self::Empty) {
            *self = match &value {
                Value::Int(_) => Self::Int(Vec::new()),
                Value::Float(_) => Self::Float(Vec::new()),
                _ => Self::Generic(Vec::new()),
            };
        }
        if !self.accepts(&value) {
            self.promote(1);
        }
        match (self, value) {
            (Self::Int(v), Value::Int(i)) => v.push(i),
            (Self::Float(v), Value::Float(f)) => v.push(f),
            (Self::Generic(v), value) => v.push(value),
            _ => unreachable!("storage accepts value"),
        }
    }

    pub fn insert(&mut self, index: usize, value: Value) {
        if index >= self.len() {
            self.push(value);
            return;
        }
        if !self.accepts(&value) {
            self.promote(1);
        }
        match (self, value) {
            (Self::Int(v), Value::Int(i)) => v.insert(index, i),
            (Self::Float(v), Value::Float(f)) => v.insert(index, f),
            (Self::Generic(v), value) => v.insert(index, value),
            _ => unreachable!("storage accepts value"),
        }
    }

    pub fn replace(&mut self, index: usize, value: &mut Value) {
        if !self.accepts(value) {
            self.promote(0);
        }
        match self {
            Self::Int(v) => {
                let Value::Int(i) = value else { unreachable!() };
                mem::swap(&mut v[index], i);
            }
            Self::Float(v) => {
                let Value::Float(f) = value else { unreachable!() };
                mem::swap(&mut v[index], f);
            }
            Self::Generic(v) => mem::swap(&mut v[index], value),
            Self::Empty => unreachable!("index validated"),
        }
    }

    pub fn extend_owned(&mut self, values: Vec<Value>) {
        if values.is_empty() {
            return;
        }
        if matches!(self, Self::Empty) {
            *self = Self::new(values);
            return;
        }
        if values.iter().any(|v| !self.accepts(v)) {
            self.promote(values.len());
        }
        match self {
            Self::Int(v) => {
                v.reserve_exact(values.len());
                v.extend(values.into_iter().map(|v| match v {
                    Value::Int(i) => i,
                    _ => unreachable!(),
                }));
            }
            Self::Float(v) => {
                v.reserve_exact(values.len());
                v.extend(values.into_iter().map(|v| match v {
                    Value::Float(f) => f,
                    _ => unreachable!(),
                }));
            }
            Self::Generic(v) => {
                v.reserve_exact(values.len());
                v.extend(values);
            }
            Self::Empty => unreachable!(),
        }
    }

    pub fn remove(&mut self, index: usize) -> Value {
        match self {
            Self::Int(v) => Value::Int(v.remove(index)),
            Self::Float(v) => Value::Float(v.remove(index)),
            Self::Generic(v) => v.remove(index),
            Self::Empty => unreachable!("index validated"),
        }
    }

    pub fn reverse(&mut self) {
        match self {
            Self::Empty => {}
            Self::Int(v) => v.reverse(),
            Self::Float(v) => v.reverse(),
            Self::Generic(v) => v.reverse(),
        }
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        match self {
            Self::Empty => unreachable!("indices validated"),
            Self::Int(v) => v.swap(a, b),
            Self::Float(v) => v.swap(a, b),
            Self::Generic(v) => v.swap(a, b),
        }
    }

    pub fn copy(&self, heap: &Heap) -> Self {
        match self {
            Self::Empty => Self::Empty,
            Self::Int(v) => Self::Int(v.clone()),
            Self::Float(v) => Self::Float(v.clone()),
            Self::Generic(v) => Self::Generic(v.iter().map(|v| v.clone_with_heap(heap)).collect()),
        }
    }

    pub fn concat_element_size(&self, other: &Self) -> usize {
        if self.is_empty() {
            other.element_size()
        } else if other.is_empty() {
            self.element_size()
        } else {
            match (self, other) {
                (Self::Int(_), Self::Int(_)) | (Self::Float(_), Self::Float(_)) => 8,
                _ => VALUE_SIZE,
            }
        }
    }

    pub fn concat(&self, other: &Self, heap: &Heap) -> Self {
        if self.is_empty() {
            return other.copy(heap);
        }
        if other.is_empty() {
            return self.copy(heap);
        }
        let len = self.len() + other.len();
        match (self, other) {
            (Self::Int(a), Self::Int(b)) => {
                let mut values = Vec::with_capacity(len);
                values.extend_from_slice(a);
                values.extend_from_slice(b);
                Self::Int(values)
            }
            (Self::Float(a), Self::Float(b)) => {
                let mut values = Vec::with_capacity(len);
                values.extend_from_slice(a);
                values.extend_from_slice(b);
                Self::Float(values)
            }
            _ => {
                let mut values = Vec::with_capacity(len);
                values.extend(self.iter().chain(other.iter()).map(|v| v.clone_with_heap(heap)));
                Self::Generic(values)
            }
        }
    }

    pub fn into_values(self) -> Vec<Value> {
        match self {
            Self::Empty => Vec::new(),
            Self::Int(v) => v.into_iter().map(Value::Int).collect(),
            Self::Float(v) => v.into_iter().map(Value::Float).collect(),
            Self::Generic(v) => v,
        }
    }
}

impl<C: ContainsHeap> DropWithContext<C> for Storage {
    fn drop_with(self, ctx: &mut C) {
        if let Self::Generic(values) = self {
            values.drop_with(ctx);
        }
    }
}

// Keep the persisted list field as a sequence of Values, independent of storage.
impl Serialize for Storage {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for item in self.iter() {
            seq.serialize_element(&*item)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for Storage {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<Value>::deserialize(deserializer).map(Self::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_storage_has_eight_byte_slots_and_promotes_without_coercion() {
        let mut ints = Storage::new((0..128).map(Value::Int).collect());
        assert_eq!(ints.element_size(), 8);
        assert_eq!(ints.capacity(), 128);
        ints.push(Value::Bool(true));
        assert!(matches!(ints, Storage::Generic(_)));
        assert!(matches!(*ints.get(0).unwrap(), Value::Int(0)));
        assert!(matches!(*ints.get(128).unwrap(), Value::Bool(true)));
        ints.remove(128);
        assert!(matches!(ints, Storage::Generic(_)));
        ints.drop_with(&mut Heap::new(16, ResourceTracker::default()));
    }

    #[test]
    fn numeric_storage_keeps_the_value_sequence_dump_encoding() {
        for values in [
            vec![Value::Int(i64::MIN), Value::Int(0), Value::Int(i64::MAX)],
            vec![Value::Float(-0.0), Value::Float(f64::INFINITY), Value::Float(f64::NAN)],
            vec![Value::Int(1), Value::None, Value::Bool(true)],
        ] {
            let old = minicbor_serde::to_vec(&values).unwrap();
            let storage = Storage::new(values);
            let new = minicbor_serde::to_vec(&storage).unwrap();
            assert_eq!(old, new);
            let restored: Storage = minicbor_serde::from_slice(&old).unwrap();
            assert_eq!(minicbor_serde::to_vec(&restored).unwrap(), old);
            let mut heap = Heap::new(16, ResourceTracker::default());
            restored.drop_with(&mut heap);
            storage.drop_with(&mut heap);
        }
    }
}
