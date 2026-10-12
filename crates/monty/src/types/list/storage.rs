use std::{mem, ops::Deref};

use monty_types::{ResourceError, ResourceTracker};

use super::List;
use crate::{
    heap::{ContainsHeap, DropWithContext, Heap},
    value::{VALUE_SIZE, Value},
};

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

impl List {
    /// Takes ownership of values without incrementing their reference counts.
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
            Self::Generic {
                contains_refs: values.iter().any(|v| matches!(v, Value::Ref(_))),
                items: values,
            }
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Int(v) => v.len(),
            Self::Float(v) => v.len(),
            Self::Generic { items: v, .. } => v.len(),
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
            Self::Generic { items: v, .. } => v.get(index).map(Item::Borrowed),
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = Item<'_>> + DoubleEndedIterator + Clone {
        (0..self.len()).map(|i| self.get(i).expect("index within storage"))
    }

    fn accepts(&self, value: &Value) -> bool {
        match self {
            Self::Empty | Self::Generic { .. } => true,
            Self::Int(_) => matches!(value, Value::Int(_)),
            Self::Float(_) => matches!(value, Value::Float(_)),
        }
    }

    pub(super) fn element_size(&self) -> usize {
        match self {
            Self::Empty | Self::Int(_) | Self::Float(_) => 8,
            Self::Generic { .. } => VALUE_SIZE,
        }
    }

    fn capacity(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Int(v) => v.capacity(),
            Self::Float(v) => v.capacity(),
            Self::Generic { items: v, .. } => v.capacity(),
        }
    }

    /// Promotion allocates the entire generic buffer while the numeric buffer is live.
    pub(super) fn check_insert(&self, value: &Value, tracker: &ResourceTracker) -> Result<(), ResourceError> {
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

    pub(super) fn check_replace(&self, value: &Value, tracker: &ResourceTracker) -> Result<(), ResourceError> {
        if self.accepts(value) {
            Ok(())
        } else {
            tracker.check_allocation(self.len().max(4).saturating_mul(VALUE_SIZE))
        }
    }

    pub(super) fn check_extend(&self, values: &[Value], tracker: &ResourceTracker) -> Result<(), ResourceError> {
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
        if matches!(self, Self::Generic { .. }) {
            return;
        }
        let mut values = Vec::with_capacity(self.len().saturating_add(additional).max(4));
        match mem::take(self) {
            Self::Int(v) => values.extend(v.into_iter().map(Value::Int)),
            Self::Float(v) => values.extend(v.into_iter().map(Value::Float)),
            Self::Empty => {}
            Self::Generic { .. } => unreachable!(),
        }
        *self = Self::Generic {
            items: values,
            contains_refs: false,
        };
    }

    pub(super) fn push(&mut self, value: Value) {
        if matches!(self, Self::Empty) {
            *self = match &value {
                Value::Int(_) => Self::Int(Vec::new()),
                Value::Float(_) => Self::Float(Vec::new()),
                _ => Self::Generic {
                    items: Vec::new(),
                    contains_refs: false,
                },
            };
        }
        if !self.accepts(&value) {
            self.promote(1);
        }
        match (self, value) {
            (Self::Int(v), Value::Int(i)) => v.push(i),
            (Self::Float(v), Value::Float(f)) => v.push(f),
            (
                Self::Generic {
                    items: v,
                    contains_refs,
                },
                value,
            ) => {
                *contains_refs |= matches!(value, Value::Ref(_));
                v.push(value);
            }
            _ => unreachable!("storage accepts value"),
        }
    }

    pub(super) fn insert(&mut self, index: usize, value: Value) {
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
            (
                Self::Generic {
                    items: v,
                    contains_refs,
                },
                value,
            ) => {
                *contains_refs |= matches!(value, Value::Ref(_));
                v.insert(index, value);
            }
            _ => unreachable!("storage accepts value"),
        }
    }

    pub(super) fn replace(&mut self, index: usize, value: &mut Value) {
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
            Self::Generic {
                items: v,
                contains_refs,
            } => {
                *contains_refs |= matches!(*value, Value::Ref(_));
                mem::swap(&mut v[index], value);
            }
            Self::Empty => unreachable!("index validated"),
        }
    }

    /// Takes ownership of preflighted values, preserving their reference counts.
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
            Self::Generic {
                items: v,
                contains_refs,
            } => {
                *contains_refs |= values.iter().any(|v| matches!(v, Value::Ref(_)));
                v.reserve_exact(values.len());
                v.extend(values);
            }
            Self::Empty => unreachable!(),
        }
    }

    pub(super) fn remove(&mut self, index: usize) -> Value {
        match self {
            Self::Int(v) => Value::Int(v.remove(index)),
            Self::Float(v) => Value::Float(v.remove(index)),
            Self::Generic { items: v, .. } => v.remove(index),
            Self::Empty => unreachable!("index validated"),
        }
    }

    pub(super) fn reverse(&mut self) {
        match self {
            Self::Empty => {}
            Self::Int(v) => v.reverse(),
            Self::Float(v) => v.reverse(),
            Self::Generic { items: v, .. } => v.reverse(),
        }
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        match self {
            Self::Empty => unreachable!("indices validated"),
            Self::Int(v) => v.swap(a, b),
            Self::Float(v) => v.swap(a, b),
            Self::Generic { items: v, .. } => v.swap(a, b),
        }
    }

    pub(super) fn copy(&self, heap: &Heap) -> Self {
        match self {
            Self::Empty => Self::Empty,
            Self::Int(v) => Self::Int(v.clone()),
            Self::Float(v) => Self::Float(v.clone()),
            Self::Generic {
                items: v,
                contains_refs,
            } => Self::Generic {
                items: v.iter().map(|v| v.clone_with_heap(heap)).collect(),
                contains_refs: *contains_refs,
            },
        }
    }

    pub(super) fn concat_element_size(&self, other: &Self) -> usize {
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

    pub(super) fn concat(&self, other: &Self, heap: &Heap) -> Self {
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
                Self::Generic {
                    items: values,
                    contains_refs: self.contains_refs() || other.contains_refs(),
                }
            }
        }
    }

    pub(super) fn into_values(self) -> Vec<Value> {
        match self {
            Self::Empty => Vec::new(),
            Self::Int(v) => v.into_iter().map(Value::Int).collect(),
            Self::Float(v) => v.into_iter().map(Value::Float).collect(),
            Self::Generic { items: v, .. } => v,
        }
    }
}

impl<C: ContainsHeap> DropWithContext<C> for List {
    fn drop_with(self, ctx: &mut C) {
        if let Self::Generic { items: values, .. } = self {
            values.drop_with(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_storage_has_eight_byte_slots_and_promotes_without_coercion() {
        let mut ints = List::new((0..128).map(Value::Int).collect());
        assert_eq!(ints.element_size(), 8);
        assert_eq!(ints.capacity(), 128);
        ints.push(Value::Bool(true));
        assert!(matches!(ints, List::Generic { .. }));
        assert!(matches!(*ints.get(0).unwrap(), Value::Int(0)));
        assert!(matches!(*ints.get(128).unwrap(), Value::Bool(true)));
        ints.remove(128);
        assert!(matches!(ints, List::Generic { .. }));
        ints.drop_with(&mut Heap::new(16, ResourceTracker::default()));
    }

    #[test]
    fn snapshots_preserve_list_variants() {
        for list in [
            List::Empty,
            List::Int(vec![i64::MIN, 0, i64::MAX]),
            List::Int(Vec::new()),
            List::Float(vec![-0.0, f64::INFINITY, f64::NAN]),
            List::Float(Vec::new()),
            List::Generic {
                items: vec![Value::None, Value::Bool(true)],
                contains_refs: false,
            },
        ] {
            let encoded = minicbor_serde::to_vec(&list).unwrap();
            let restored: List = minicbor_serde::from_slice(&encoded).unwrap();
            assert_eq!(mem::discriminant(&list), mem::discriminant(&restored));
            assert_eq!(encoded, minicbor_serde::to_vec(&restored).unwrap());
            let mut heap = Heap::new(16, ResourceTracker::default());
            list.drop_with(&mut heap);
            restored.drop_with(&mut heap);
        }
    }
}
