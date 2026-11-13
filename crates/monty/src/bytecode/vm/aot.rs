//! Pure function speculation keeps the original frame intact until successful return.
#![allow(unsafe_code)]

use std::{ffi::c_void, ptr};

use smallvec::SmallVec;

use super::{RunError, VM, Value};
use crate::{
    builtins::{Builtins, BuiltinsFunctions},
    bytecode::aot::{
        self, BuiltinHint, abi,
        backend::Artifact,
        ir::{Element, Kind, Plan},
    },
    function::ExactPositionalCall,
    hash::HashValue,
    types::{PyTrait, Range, Type, tuple::allocate_tuple},
};

struct Invocation<'vm, 'code> {
    vm: &'vm mut VM<'code>,
    error: Option<RunError>,
    check_limits: bool,
    poll_print: bool,
    plan: &'code Plan,
    buffers: SmallVec<[Vec<u64>; 4]>,
}

unsafe extern "C" fn checkpoint(opaque: *mut c_void, ip: usize) -> bool {
    // SAFETY: the synchronous caller retains Invocation and its exclusive VM borrow.
    let invocation = unsafe { &mut *opaque.cast::<Invocation<'_, '_>>() };
    invocation.vm.instruction_ip = ip;
    match invocation
        .vm
        .dispatch_checkpoint(invocation.check_limits, invocation.poll_print)
    {
        Ok(()) => true,
        Err(error) => {
            invocation.error = Some(error);
            false
        }
    }
}

unsafe extern "C" fn range_init(context: *mut abi::Context, patch: *const abi::Patch) -> bool {
    // SAFETY: called only with the current invocation and an immutable artifact patch.
    let (context, patch) = unsafe { (&mut *context, &*patch) };
    let count = usize::try_from(patch.immediate & 255).expect("range argument count");

    if !(1..=3).contains(&count)
        || patch.dst.checked_add(2).is_none_or(|end| end >= context.length)
        || [patch.left, patch.right, patch.third]
            .iter()
            .any(|&i| i >= context.length)
    {
        return false;
    }
    // SAFETY: bounds above cover every register access and the initialized range slot.
    unsafe {
        let a = context.registers.add(patch.left).read().cast_signed();
        let b = context.registers.add(patch.right).read().cast_signed();
        let c = context.registers.add(patch.third).read().cast_signed();
        let (start, stop, step) = match count {
            1 => (0, a, 1),
            2 => (a, b, 1),
            _ => (a, b, c),
        };
        if step == 0 {
            return false;
        }
        let remaining = Range::new(start, stop, step).len() as u64;
        context.registers.add(patch.dst).write(start.cast_unsigned());
        context.registers.add(patch.dst + 1).write(step.cast_unsigned());
        context.registers.add(patch.dst + 2).write(remaining);
    }
    true
}

impl VM<'_> {
    pub(super) fn try_aot(&mut self, countdown: &mut u8, check_limits: bool, poll_print: bool) -> Result<(), RunError> {
        if self.current_frame.ip != 0
            || self.current_frame.function_id.is_none()
            || self.current_frame.is_parked
            || self.current_frame.aot_attempted
        {
            return Ok(());
        }
        self.current_frame.aot_attempted = true;
        let code = self.current_frame.code;
        let Some(typed) = code.aot() else {
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
                BuiltinHint::Range => Builtins::Type(Type::Range),
                BuiltinHint::Len => Builtins::Function(BuiltinsFunctions::Len),
            };
            // Undefined module slots use the builtin fallback in load_global_callable.
            if !matches!(self.globals.get(slot), Some(Value::Undefined))
                && !matches!(self.globals.get(slot),Some(Value::Builtin(actual)) if *actual==expected)
            {
                aot::record(|s| s.fallbacks += 1);
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
                    aot::record(|s| s.fallbacks += 1);
                    return Ok(());
                }
            };
            arguments.push(bits);
        }
        let Some(artifact) = typed.artifact.get_or_init(|| {
            let artifact = Artifact::compile(plan, &self.heap.tracker)?;
            aot::record(|s| {
                s.compilations += 1;
                s.code_bytes += artifact.code_bytes() as u64;
            });
            Some(Box::new(artifact))
        }) else {
            return Ok(());
        };
        if self
            .heap
            .tracker
            .check_allocation(artifact.registers * size_of::<u64>())
            .is_err()
        {
            return Ok(());
        }
        let sources = if plan.needs_views { plan.parameters.len() } else { 0 };
        if self
            .heap
            .tracker
            .check_allocation(sources * (size_of::<abi::View>() + size_of::<Vec<u64>>()))
            .is_err()
        {
            return Ok(());
        }
        let mut views = SmallVec::<[abi::View; 4]>::new();
        views.resize(sources, abi::View::default());
        let mut registers = SmallVec::<[u64; 64]>::new();
        registers.resize(artifact.registers, 0);
        for (index, bits) in arguments.into_iter().enumerate() {
            registers[3 * index] = bits;
            registers[3 * index + 1] = 1;
        }
        for &(index, bits) in &plan.initial_values {
            registers[index] = bits;
        }
        let (entry, patch) = artifact.entry();
        let mut invocation = Invocation {
            vm: self,
            error: None,
            check_limits,
            poll_print,
            plan,
            buffers: (0..sources).map(|_| Vec::new()).collect(),
        };
        let mut context = abi::Context {
            registers: registers.as_mut_ptr(),
            length: registers.len(),
            remaining: usize::from(*countdown),
            opaque: ptr::from_mut(&mut invocation).cast(),
            checkpoint,
            ip: 0,
            result: 0,
            result_kind: 0,
            result_start: 0,
            instructions: u64::from(*countdown),
            range_init,
            views: views.as_mut_ptr(),
            views_len: views.len(),
            prepare_view,
            read,
        };
        // SAFETY: verified typed templates access only this fixed buffer. Callback borrows end
        // before execution continues; original arguments and code constants remain VM-owned.
        let outcome = unsafe { entry(&raw mut context, patch) };
        let error = invocation.error.take();
        *countdown = u8::try_from(context.remaining).expect("AOT countdown");
        aot::record(|s| {
            s.entries += 1;
            s.instructions += context.instructions - context.remaining as u64;
            if outcome == abi::RETURN {
                s.returns += 1;
            } else {
                s.fallbacks += 1;
            }
        });
        if let Some(error) = error {
            return Err(error);
        }
        if outcome == abi::RETURN {
            let value = match context.result_kind {
                1 => Value::Int(context.result.cast_signed()),
                2 => Value::Float(f64::from_bits(context.result)),
                3 => Value::Bool(context.result != 0),
                4 => Value::None,
                kind if kind & 255 >= 17 && kind & 255 <= 20 => {
                    let count = kind >> 8;
                    let element = kind & 255;
                    let mut values = SmallVec::new();
                    for &bits in &registers[context.result_start..context.result_start + count] {
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
                _ => unreachable!("verified AOT return kind"),
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
    use super::HeapData;
    match (kind, value) {
        (Kind::Str, Value::InternString(_)) => true,
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

unsafe extern "C" fn prepare_view(context: *mut abi::Context, patch: *const abi::Patch) -> bool {
    // SAFETY: both pointers are retained by the synchronous native invocation.
    let (context, patch) = unsafe { (&mut *context, &*patch) };
    if patch.left >= context.length {
        return false;
    }
    // SAFETY: the source register is in the fixed invocation buffer.
    let bits = unsafe { context.registers.add(patch.left).read() };
    let Ok(index) = usize::try_from(bits) else {
        return false;
    };
    if index >= context.views_len {
        return false;
    }
    // SAFETY: opaque owns the VM borrow; this helper executes no Python or heap mutation.
    let invocation = unsafe { &mut *context.opaque.cast::<Invocation<'_, '_>>() };
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
        // SAFETY: index was checked against the fixed view table above.
        unsafe {
            context.views.add(index).write(abi::View {
                data: ptr::null(),
                length: items.len(),
                prepared: 2,
            });
        }
        return true;
    }
    let Some(bytes) = items.len().checked_mul(size_of::<u64>()) else {
        return false;
    };
    if vm.heap.tracker.check_allocation(bytes).is_err() {
        return false;
    }
    let mut values = Vec::with_capacity(items.len());
    for (offset, value) in items.iter().enumerate() {
        let Some(bits) = scalar(value, element) else {
            return false;
        };
        values.push(bits);
        if invocation.check_limits
            && offset % 4096 == 4095
            && let Err(error) = vm.heap.tracker.check_memory_time()
        {
            invocation.error = Some(error.into());
            return false;
        }
    }
    invocation.buffers[index] = values;
    let values = &invocation.buffers[index];
    // SAFETY: the view table is fixed and each prepared buffer is retained without resizing.
    unsafe {
        context.views.add(index).write(abi::View {
            data: values.as_ptr(),
            length: values.len(),
            prepared: 1,
        });
    }
    true
}

unsafe extern "C" fn read(context: *mut abi::Context, patch: *const abi::Patch) -> bool {
    // SAFETY: the invocation and patch are live for this synchronous callback.
    let (context, patch) = unsafe { (&mut *context, &*patch) };
    if patch.left >= context.length || patch.right >= context.length || patch.dst >= context.length {
        return false;
    }
    // SAFETY: both source registers were bounds-checked above.
    let (object, key) = unsafe {
        (
            context.registers.add(patch.left).read(),
            context.registers.add(patch.right).read(),
        )
    };
    let Ok(object) = usize::try_from(object) else {
        return false;
    };
    // SAFETY: opaque borrows the caller's live Invocation for this callback.
    let invocation = unsafe { &mut *context.opaque.cast::<Invocation<'_, '_>>() };
    let parameters = invocation.plan.parameters.len();
    let vm = &mut *invocation.vm;
    let Some(Value::Ref(id)) = original(vm, parameters, object) else {
        return false;
    };
    let id = *id;
    let key_hash = if patch.immediate != 256 && matches!(vm.heap.get(id), super::HeapData::Dict(_)) {
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
    let bits = if patch.immediate == 256 {
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
        let element = match patch.immediate {
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
            // SAFETY: the destination register was bounds-checked at callback entry.
            unsafe {
                context.registers.add(patch.dst).write(bits);
            }
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
        let Some(value) = dict.native_string_get(key, hash, vm.heap, vm.interns) else {
            return false;
        };
        let Some(bits) = scalar(value, element) else {
            return false;
        };
        bits
    };
    // SAFETY: the destination is in the fixed initialized register buffer.
    unsafe {
        context.registers.add(patch.dst).write(bits);
    }
    true
}

fn string_hash(vm: &mut VM<'_>, parameters: usize, source: usize) -> Option<HashValue> {
    let id = match original(vm, parameters, source)? {
        Value::InternString(id) => return Some(vm.interns.str_hash(*id)),
        Value::Ref(id) => *id,
        _ => return None,
    };
    if !matches!(vm.heap.get(id), super::HeapData::Str(_)) {
        return None;
    }
    // Exact strings use the existing cache without invoking Python protocols.
    vm.heap.read(id).py_hash(vm).ok().flatten()
}

pub(super) enum ScalarCall {
    Unsupported,
    Miss,
    Value(Value),
}

unsafe extern "C" fn no_checkpoint(_: *mut c_void, _: usize) -> bool {
    false
}
unsafe extern "C" fn no_helper(_: *mut abi::Context, _: *const abi::Patch) -> bool {
    false
}

impl VM<'_> {
    /// Finite arithmetic leaves need no physical frame or callbacks. Every guard
    /// failure leaves the callable and arguments on the stack for ordinary binding.
    pub(super) fn try_scalar_aot_call(
        &mut self,
        arguments: usize,
        countdown: &mut u8,
        check_limits: bool,
        poll_print: bool,
    ) -> Result<ScalarCall, RunError> {
        let Some(base) = self.stack.len().checked_sub(arguments + 1) else {
            return Ok(ScalarCall::Unsupported);
        };
        let Value::DefFunction(id) = self.stack[base] else {
            return Ok(ScalarCall::Unsupported);
        };
        let function = self.interns.get_function(id);
        if !matches!(function.exact_positional_call(),Some(ExactPositionalCall::Sync(count)) if count==arguments) {
            return Ok(ScalarCall::Unsupported);
        }
        let code = &function.code;
        let Some(typed) = code.aot() else {
            return Ok(ScalarCall::Unsupported);
        };
        // Avoid lazy compilation when the ordinary call must fail recursion admission.
        if self.heap.tracker.check_recursion_depth(self.recursion_depth).is_err() {
            return Ok(ScalarCall::Unsupported);
        }
        let Some(plan) = typed
            .plan
            .get_or_init(|| Plan::compile(code, &typed.hints).map(Box::new))
        else {
            return Ok(ScalarCall::Unsupported);
        };
        let Some(cost) = plan.scalar_call_cost else {
            return Ok(ScalarCall::Unsupported);
        };
        if plan.parameters.len() != arguments || plan.locals != function.namespace_size {
            return Ok(ScalarCall::Unsupported);
        }
        let mut values = SmallVec::<[u64; 8]>::new();
        for (kind, value) in plan.parameters.iter().zip(&self.stack[base + 1..]) {
            let bits = match (kind, value) {
                (Kind::Int, Value::Int(value)) => value.cast_unsigned(),
                (Kind::Float, Value::Float(value)) => value.to_bits(),
                (Kind::Bool, Value::Bool(value)) => u64::from(*value),
                (Kind::None, Value::None) => 0,
                _ => {
                    aot::record(|s| s.fallbacks += 1);
                    return Ok(ScalarCall::Miss);
                }
            };
            values.push(bits);
        }
        let Some(artifact) = typed.artifact.get_or_init(|| {
            let artifact = Artifact::compile(plan, &self.heap.tracker)?;
            aot::record(|s| {
                s.compilations += 1;
                s.code_bytes += artifact.code_bytes() as u64;
            });
            Some(Box::new(artifact))
        }) else {
            return Ok(ScalarCall::Unsupported);
        };
        if self
            .heap
            .tracker
            .check_allocation(artifact.registers * size_of::<u64>())
            .is_err()
        {
            return Ok(ScalarCall::Unsupported);
        }
        let mut recursion = self.recursion_guard()?;
        let vm = &mut *recursion;
        if usize::from(*countdown) < cost {
            vm.dispatch_checkpoint(check_limits, poll_print)?;
            *countdown = 255;
        }
        let mut registers = SmallVec::<[u64; 64]>::new();
        registers.resize(artifact.registers, 0);
        for (index, bits) in values.into_iter().enumerate() {
            registers[3 * index] = bits;
            registers[3 * index + 1] = 1;
        }
        for &(index, bits) in &plan.initial_values {
            registers[index] = bits;
        }
        let mut context = abi::Context {
            registers: registers.as_mut_ptr(),
            length: registers.len(),
            remaining: usize::from(*countdown),
            opaque: ptr::null_mut(),
            checkpoint: no_checkpoint,
            ip: 0,
            result: 0,
            result_kind: 0,
            result_start: 0,
            instructions: u64::from(*countdown),
            range_init: no_helper,
            views: ptr::null_mut(),
            views_len: 0,
            prepare_view: no_helper,
            read: no_helper,
        };
        let (entry, patch) = artifact.entry();
        // SAFETY: the plan has one finite scalar block, no helper operations and a
        // complete instruction reservation. The fixed register buffer outlives the call.
        let outcome = unsafe { entry(&raw mut context, patch) };
        *countdown = u8::try_from(context.remaining).expect("scalar AOT countdown");
        aot::record(|s| {
            s.entries += 1;
            s.instructions += context.instructions - context.remaining as u64;
        });
        if outcome != abi::RETURN {
            aot::record(|s| s.fallbacks += 1);
            return Ok(ScalarCall::Miss);
        }
        let value = match context.result_kind {
            1 => Value::Int(context.result.cast_signed()),
            2 => Value::Float(f64::from_bits(context.result)),
            3 => Value::Bool(context.result != 0),
            4 => Value::None,
            _ => return Ok(ScalarCall::Miss),
        };
        // Account for the ReturnValue handler which this path replaces.
        *countdown = countdown.checked_sub(1).expect("reserved scalar return instruction");
        for value in vm.stack.drain(base..) {
            value.drop_with(vm.heap);
        }
        aot::record(|s| {
            s.returns += 1;
            s.fast_calls += 1;
        });
        Ok(ScalarCall::Value(value))
    }
}
