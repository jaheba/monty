//! Pure function speculation keeps the original frame intact until successful return.
use smallvec::SmallVec;

use super::{HeapData, HeapReadOutput, RunError, VM, Value};
use crate::{
    builtins::{Builtins, BuiltinsFunctions},
    bytecode::typed_ir::{
        self, BuiltinHint,
        execute::{Compiled, Context, Helpers, Operands, Outcome, View},
        ir::{Element, Kind, Plan},
    },
    hash::{HashValue, hash_python_str},
    types::{BuiltinType, PyTrait, tuple::allocate_tuple},
};

struct Invocation<'vm, 'code> {
    vm: &'vm mut VM<'code>,
    error: Option<RunError>,
    check_limits: bool,
    poll_print: bool,
    plan: &'code Plan,
    buffers: SmallVec<[Vec<u64>; 4]>,
}

impl VM<'_> {
    pub(super) fn try_typed_ir(
        &mut self,
        countdown: &mut u8,
        check_limits: bool,
        poll_print: bool,
    ) -> Result<(), RunError> {
        if self.current_frame.ip != 0
            || self.current_frame.function_id.is_none()
            || self.current_frame.is_parked
            || self.current_frame.typed_ir_attempted
        {
            return Ok(());
        }
        self.current_frame.typed_ir_attempted = true;
        // Builtin guards address session slots, not an explicit globals dictionary.
        if self.current_frame.namespace.is_some() {
            return Ok(());
        }
        let code = self.current_frame.code;
        let Some(typed) = code.typed_ir() else {
            return Ok(());
        };
        let Some(plan) = typed
            .plan
            .get_or_init(|| Plan::compile(code, &typed.hints).map(Box::new))
        else {
            return Ok(());
        };
        if plan.locals != usize::from(self.current_frame.locals_count) {
            return Ok(());
        }
        let base = self.current_frame.stack_base();
        if self.stack.len() != base + plan.locals {
            return Ok(());
        }
        for &(slot, builtin) in &plan.builtin_globals {
            let expected = match builtin {
                BuiltinHint::Range => Builtins::Type(BuiltinType::Range),
                BuiltinHint::Len => Builtins::Function(BuiltinsFunctions::Len),
            };
            // Undefined module slots use the builtin fallback in load_global_callable.
            if !matches!(self.globals.get(slot), Some(Value::Undefined))
                && !matches!(self.globals.get(slot),Some(Value::Builtin(actual)) if *actual==expected)
            {
                typed_ir::record(|s| s.fallbacks += 1);
                return Ok(());
            }
        }
        let mut arguments = SmallVec::<[u64; 8]>::new();
        for (index, kind) in plan.parameters.iter().enumerate() {
            let bits = match (kind, &self.stack[base + index]) {
                (Kind::Int, Value::Int(value)) => value.cast_unsigned(),
                (Kind::Float, Value::Float(value)) => value.to_bits(),
                (Kind::Bool, Value::Bool(value)) => u64::from(*value),
                (Kind::None, Value::None) | (Kind::Unknown, _) => 0,
                (kind, value) if kind.reference() && matches_reference(*kind, value, self) => index as u64,
                _ => {
                    typed_ir::record(|s| s.fallbacks += 1);
                    return Ok(());
                }
            };
            arguments.push(bits);
        }
        let Some(compiled) = typed.compiled.get_or_init(|| {
            let compiled = Compiled::compile(plan, &self.heap.tracker)?;
            typed_ir::record(|s| {
                s.compilations += 1;
            });
            Some(Box::new(compiled))
        }) else {
            return Ok(());
        };
        if self
            .heap
            .tracker
            .check_allocation(compiled.registers * size_of::<u64>())
            .is_err()
        {
            return Ok(());
        }
        let sources = if plan.needs_views { plan.parameters.len() } else { 0 };
        if self
            .heap
            .tracker
            .check_allocation(sources * (size_of::<View>() + size_of::<Vec<u64>>()))
            .is_err()
        {
            return Ok(());
        }
        let mut views = SmallVec::<[View; 4]>::new();
        views.resize(sources, View::default());
        let mut registers = SmallVec::<[u64; 64]>::new();
        registers.resize(compiled.registers, 0);
        for (index, bits) in arguments.into_iter().enumerate() {
            registers[3 * index] = bits;
            registers[3 * index + 1] = 1;
        }
        for &(index, bits) in &plan.initial_values {
            registers[index] = bits;
        }
        let mut invocation = Invocation {
            vm: self,
            error: None,
            check_limits,
            poll_print,
            plan,
            buffers: (0..sources).map(|_| Vec::new()).collect(),
        };
        let mut context = Context {
            registers: &mut registers,
            views: &mut views,
            remaining: usize::from(*countdown),
            ip: 0,
            result: 0,
            result_kind: 0,
            result_start: 0,
            instructions: u64::from(*countdown),
        };
        let outcome = compiled.run(&mut context, &mut invocation);
        let error = invocation.error.take();
        *countdown = u8::try_from(context.remaining).expect("typed IR countdown");
        typed_ir::record(|s| {
            s.entries += 1;
            s.instructions += context.instructions - context.remaining as u64;
            if outcome == Outcome::Return {
                s.returns += 1;
            } else {
                s.fallbacks += 1;
            }
        });
        if let Some(error) = error {
            return Err(error);
        }
        if outcome == Outcome::Return {
            let value = match context.result_kind {
                1 => Value::Int(context.result.cast_signed()),
                2 => Value::Float(f64::from_bits(context.result)),
                3 => Value::Bool(context.result != 0),
                4 => Value::None,
                kind if kind & 255 >= 17 && kind & 255 <= 20 => {
                    let count = kind >> 8;
                    let element = kind & 255;
                    let mut values = SmallVec::new();
                    for &bits in &context.registers[context.result_start..context.result_start + count] {
                        values.push(match element {
                            17 => Value::Int(bits.cast_signed()),
                            18 => Value::Float(f64::from_bits(bits)),
                            19 => Value::Bool(bits != 0),
                            20 => Value::None,
                            _ => unreachable!(),
                        });
                    }
                    allocate_tuple(values, self.heap)
                }
                _ => unreachable!("verified typed IR return kind"),
            };
            self.push(value);
            // Reuse the VM's return handler for generators, async tasks and frame ownership.
            self.current_frame.ip = context.ip;
        }
        Ok(())
    }
}

fn original<'vm>(vm: &'vm VM<'_>, parameters: usize, index: usize) -> Option<&'vm Value> {
    if index < parameters {
        vm.stack.get(vm.current_frame.stack_base() + index)
    } else {
        vm.current_frame.code.constants().get(index - parameters)
    }
}

fn matches_reference(kind: Kind, value: &Value, vm: &VM<'_>) -> bool {
    match (kind, value) {
        (Kind::Str, Value::InternString(_) | Value::InlineString(_)) => true,
        (Kind::Str, Value::Ref(id)) => matches!(vm.heap.get(*id), HeapData::Str(_)),
        (Kind::List(_), Value::Ref(id)) => matches!(vm.heap.get(*id), HeapData::List(_)),
        (Kind::Tuple(_), Value::Ref(id)) => matches!(vm.heap.get(*id), HeapData::Tuple(_)),
        (Kind::Sequence(_), Value::Ref(id)) => matches!(vm.heap.get(*id), HeapData::List(_) | HeapData::Tuple(_)),
        (Kind::Dict(_), Value::Ref(id)) => {
            matches!(vm.heap.get(*id),HeapData::Dict(dict) if !dict.is_counter() && !dict.is_defaultdict())
        }
        _ => false,
    }
}

fn scalar(value: &Value, element: Element) -> Option<u64> {
    match (element, value) {
        (Element::Int, Value::Int(value)) => Some(value.cast_unsigned()),
        (Element::Float, Value::Float(value)) => Some(value.to_bits()),
        (Element::Bool, Value::Bool(value)) => Some(u64::from(*value)),
        (Element::None, Value::None) => Some(0),
        _ => None,
    }
}

fn string_hash(vm: &mut VM<'_>, parameters: usize, source: usize) -> Option<HashValue> {
    let id = match original(vm, parameters, source)? {
        Value::InternString(id) => return Some(vm.interns.str_hash(*id)),
        Value::InlineString(value) => return Some(hash_python_str(value.as_str())),
        Value::Ref(id) => *id,
        _ => return None,
    };
    if !matches!(vm.heap.get(id), super::HeapData::Str(_)) {
        return None;
    }
    // Exact strings use the existing cache without invoking Python protocols.
    match vm.heap.read(id) {
        HeapReadOutput::Str(heap_string) => heap_string.py_hash(vm).ok().flatten(),
        _ => None,
    }
}

impl Helpers for Invocation<'_, '_> {
    fn checkpoint(&mut self, ip: usize) -> bool {
        self.vm.instruction_ip = ip;
        match self.vm.dispatch_checkpoint(self.check_limits, self.poll_print) {
            Ok(()) => true,
            Err(error) => {
                self.error = Some(error);
                false
            }
        }
    }

    fn prepare_view(&mut self, context: &mut Context<'_>, operands: &Operands) -> bool {
        if operands.left >= context.registers.len() {
            return false;
        }
        let bits = context.registers[operands.left];
        let Ok(index) = usize::try_from(bits) else {
            return false;
        };
        if index >= context.views.len() {
            return false;
        }
        let invocation = self;
        let Some(element) = invocation
            .plan
            .parameters
            .get(index)
            .and_then(|kind| kind.sequence_element())
        else {
            return false;
        };
        let vm = &*invocation.vm;
        let Some(Value::Ref(id)) = original(vm, invocation.plan.parameters.len(), index) else {
            return false;
        };
        let items = match vm.heap.get(*id) {
            super::HeapData::List(list) => list.as_slice(),
            super::HeapData::Tuple(tuple) => tuple.as_slice(),
            _ => return false,
        };
        if items.len() > 4096 {
            // Sparse reads should not copy an arbitrarily large input. Its original
            // VM-owned list or tuple remains rooted while checked reads use the helper.
            context.views[index] = View::Direct(items.len());
            return true;
        }
        let Some(bytes) = items.len().checked_mul(size_of::<u64>()) else {
            return false;
        };
        if vm.heap.tracker.check_allocation(bytes).is_err() {
            return false;
        }
        let mut values = Vec::with_capacity(items.len());
        for value in items {
            let Some(bits) = scalar(value, element) else {
                return false;
            };
            values.push(bits);
        }
        invocation.buffers[index] = values;
        let values = &invocation.buffers[index];
        context.views[index] = View::Buffered(values.len());
        true
    }

    fn read(&mut self, context: &mut Context<'_>, operands: &Operands) -> bool {
        if operands.left >= context.registers.len()
            || operands.right >= context.registers.len()
            || operands.dst >= context.registers.len()
        {
            return false;
        }
        let (object, key) = (context.registers[operands.left], context.registers[operands.right]);
        let Ok(object) = usize::try_from(object) else {
            return false;
        };
        let invocation = self;
        let parameters = invocation.plan.parameters.len();
        let vm = &mut *invocation.vm;
        let Some(Value::Ref(id)) = original(vm, parameters, object) else {
            return false;
        };
        let id = *id;
        let key_hash = if operands.immediate != 256 && matches!(vm.heap.get(id), super::HeapData::Dict(_)) {
            let Ok(key_index) = usize::try_from(key) else {
                return false;
            };
            let Some(hash) = string_hash(vm, parameters, key_index) else {
                return false;
            };
            Some(hash)
        } else {
            None
        };
        let data = vm.heap.get(id);
        let bits = if operands.immediate == 256 {
            let length = match data {
                super::HeapData::List(list) => list.as_slice().len(),
                super::HeapData::Tuple(tuple) => tuple.as_slice().len(),
                super::HeapData::Dict(dict) => dict.len(),
                _ => return false,
            };
            let Ok(length) = i64::try_from(length) else {
                return false;
            };
            length.cast_unsigned()
        } else {
            let element = match operands.immediate {
                1 => Element::Int,
                2 => Element::Float,
                3 => Element::Bool,
                4 => Element::None,
                _ => return false,
            };
            if let super::HeapData::List(_) | super::HeapData::Tuple(_) = data {
                let items = match data {
                    super::HeapData::List(list) => list.as_slice(),
                    super::HeapData::Tuple(tuple) => tuple.as_slice(),
                    _ => unreachable!(),
                };
                let Ok(length) = i64::try_from(items.len()) else {
                    return false;
                };
                let index = key.cast_signed();
                let index = if index < 0 {
                    index.checked_add(length)
                } else {
                    Some(index)
                };
                let Some(index) = index.and_then(|index| usize::try_from(index).ok()) else {
                    return false;
                };
                let Some(bits) = items.get(index).and_then(|value| scalar(value, element)) else {
                    return false;
                };
                context.registers[operands.dst] = bits;
                return true;
            }
            let super::HeapData::Dict(dict) = data else {
                return false;
            };
            let Ok(key) = usize::try_from(key) else {
                return false;
            };
            let Some(key) = original(vm, parameters, key) else {
                return false;
            };
            let Some(hash) = key_hash else {
                return false;
            };
            let Some(value) = dict.guarded_string_get(key, hash, vm.heap, vm.interns) else {
                return false;
            };
            let Some(bits) = scalar(value, element) else {
                return false;
            };
            bits
        };
        context.registers[operands.dst] = bits;
        true
    }

    fn buffer(&self, index: usize) -> Option<&[u64]> {
        self.buffers.get(index).map(Vec::as_slice)
    }
}
