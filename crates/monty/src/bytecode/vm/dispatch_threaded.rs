//! Threaded opcode handlers with bounded batches, including when tail calls are not optimized.

use std::mem;

#[cfg(all(monty_native, feature = "experimental-aot"))]
use super::aot::ScalarCall;
use super::{
    AwaitResult, CallResult, CellValue, Closure, ExcType, ExcTypeExt, FrameExit, FrameNamespace, FunctionDefaults,
    FunctionId, GeneratorConsumer, GeneratorReturn, HeapData, HeapReadOutput, LongInt, Opcode, PyTrait, RunError,
    SimpleException, StringId, VM, Value, decode_assert_flags, recursion,
};
#[cfg(all(monty_native, feature = "experimental-aot"))]
use crate::bytecode::aot as typed_aot;

macro_rules! yield_if_parked {
    ($self:expr, $context:expr) => {
        if $self.current_frame.is_parked {
            return $context.finish($self.pending_futures_exit());
        }
    };
}

/// Tries an operation and routes a Python exception through the active frames.
macro_rules! try_catch {
    ($self:expr, $context:expr, $expr:expr) => {
        if let Err(e) = $expr {
            if let Some(result) = $self.handle_exception(e) {
                return $context.error(result);
            }
            yield_if_parked!($self, $context);
        }
    };
}

/// Routes a Python exception through the active frames.
///
/// Wrapped in a block to allow use in match arm expressions.
macro_rules! catch {
    ($self:expr, $context:expr, $err:expr) => {{
        if let Some(result) = $self.handle_exception($err) {
            return $context.error(result);
        }
        yield_if_parked!($self, $context);
    }};
}

/// Applies a relative jump offset to the current frame IP.
///
/// Uses checked arithmetic to safely compute the new IP, panicking if the
/// jump would result in a negative or overflowing instruction pointer.
macro_rules! jump_relative {
    ($ip:expr, $offset:expr) => {{
        $ip = $ip
            .checked_add_signed($offset.into())
            .expect("jump resulted in negative or overflowing IP");
    }};
}

/// Handles the result of a load operation that may yield a `FrameExit::NameLookup`.
///
/// `load_local` and `load_global` return `Result<Option<FrameExit>, RunError>`:
/// - `Ok(None)`: load succeeded, value is on the stack
/// - `Ok(Some(FrameExit::NameLookup { .. }))`: unresolved name, yield to host
/// - `Err(e)`: exception (e.g., UnboundLocalError)
macro_rules! handle_load_result {
    ($self:expr, $context:expr, $result:expr) => {
        match $result {
            Ok(None) => {}
            Ok(Some(frame_exit)) => return $context.exit(frame_exit),
            Err(e) => catch!($self, $context, e),
        }
    };
}

/// Handles the result of a call operation that returns `CallResult`.
///
/// This macro eliminates the repetitive pattern of matching on `CallResult`
/// variants that appears in LoadAttr, CallFunction, CallFunctionKw, CallAttr,
/// CallAttrKw, and CallFunctionExtended opcodes.
///
/// Actions taken for each variant:
/// - `Push(value)`: Push the value onto the stack
/// - `FramePushed`: Continue with the newly current frame
/// - `External(ext_id, args)`: Return `FrameExit::ExternalCall` to yield to host
/// - `OsCall(call)`: Return `FrameExit::OsCall` to yield to host
/// - `MethodCall { .. }`: Return `FrameExit::MethodCall` to yield to host
/// - `AttrLookup { .. }`: Return `FrameExit::AttrLookup` to yield to host
/// - `AwaitValue(value)`: Push value, then implicitly await it via `exec_get_awaitable`
/// - `Err(err)`: Handle the exception via `catch!`
macro_rules! handle_call_result {
    ($self:expr, $context:expr, $result:expr) => {
        match $result {
            Ok(CallResult::Value(result)) => $self.push(result),
            Ok(CallResult::FramePushed) =>
            {
                #[cfg(all(monty_native, feature = "experimental-aot"))]
                if $context.aot_enabled && !$self.current_frame.aot_attempted {
                    let (check_limits, poll_print) = $context.limits_and_print;
                    if let Err(error) = $self.try_aot(&mut $context.fuel, check_limits, poll_print) {
                        return $context.error(error);
                    }
                }
            }
            Ok(CallResult::External(name, args)) => {
                let call_id = $self.allocate_call_id();
                let name_load_ip = $self.ext_function_load_ip.take();
                return $context.exit(FrameExit::ExternalCall {
                    function_name: name,
                    args,
                    call_id,
                    name_load_ip,
                });
            }
            Ok(CallResult::OsCall(call)) => match $self.prepare_os_call(call, None) {
                Ok(Some(exit)) => return $context.exit(exit),
                Ok(None) => {}
                Err(err) => catch!($self, $context, err),
            },
            Ok(CallResult::OsCallWithEffect { call, effect }) => match $self.prepare_os_call(call, Some(effect)) {
                Ok(Some(exit)) => return $context.exit(exit),
                Ok(None) => {}
                Err(err) => catch!($self, $context, err),
            },
            Ok(CallResult::MethodCall { name, args, object_id }) => {
                let call_id = $self.allocate_call_id();
                return $context.exit(FrameExit::MethodCall {
                    method_name: name,
                    args,
                    call_id,
                    object_id,
                });
            }
            Ok(CallResult::AttrLookup {
                name,
                class_name,
                object_id,
                type_object,
                effect,
            }) => {
                return $context.exit(FrameExit::AttrLookup {
                    name,
                    class_name,
                    object_id,
                    type_object,
                    effect,
                });
            }
            Ok(CallResult::AwaitValue(value)) => {
                // Push the value and implicitly await it (used by asyncio.run())
                $self.push(value);
                match $self.exec_get_awaitable() {
                    Ok(AwaitResult::ValueReady(value)) => $self.push(value),
                    Ok(AwaitResult::FramePushed) => {}
                    Ok(AwaitResult::Yield(pending_calls)) => {
                        return $context.exit(FrameExit::ResolveFutures(pending_calls));
                    }
                    Err(e) => catch!($self, $context, e),
                }
            }
            Err(err) => catch!($self, $context, err),
        }
    };
}

const CHECK_LIMITS: u32 = 1 << 8;
const POLL_PRINT: u32 = 1 << 9;
const FUEL_SHIFT: u32 = 16;
const FUEL_MASK: u32 = 255 << FUEL_SHIFT;
type Handler = for<'h> fn(&mut VM<'h>, &[u8], usize, &mut DispatchContext, u32, &DispatchTable) -> Status;

/// Only actual exits write a payload; continuation crosses helpers in one register.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Status {
    Continue,
    Exit,
    Error,
}

struct DispatchContext {
    chain_remaining: u8,
    fuel: u8,
    #[cfg(all(monty_native, feature = "experimental-aot"))]
    aot_enabled: bool,
    #[cfg(all(monty_native, feature = "experimental-aot"))]
    limits_and_print: (bool, bool),
    outcome: Option<Result<FrameExit, RunError>>,
}

impl DispatchContext {
    #[cold]
    fn exit(&mut self, exit: FrameExit) -> Status {
        self.outcome = Some(Ok(exit));
        Status::Exit
    }

    #[cold]
    fn error(&mut self, error: RunError) -> Status {
        self.outcome = Some(Err(error));
        Status::Error
    }

    #[cold]
    fn finish(&mut self, outcome: Result<FrameExit, RunError>) -> Status {
        match outcome {
            Ok(exit) => self.exit(exit),
            Err(error) => self.error(error),
        }
    }
}

#[repr(C)]
struct DispatchTable {
    handlers: [Handler; 256],
    checkpoints: [Handler; 4],
}

macro_rules! opcode_handlers {
    ($vm:ident, $context:ident, $check_limits:ident, $poll_print:ident; $( $(#[$attr:meta])* $opcode:ident => $name:ident $body:block )*) => {
        $( $(#[$attr])*
        #[inline(never)]
        fn $name(vm: &mut VM<'_>, _bytes: &[u8], ip: usize, context: &mut DispatchContext, state: u32, table: &DispatchTable) -> Status {
            // Finish all operation-local cleanup before dispatching the next instruction.
            #[inline(never)]
            fn execute($vm: &mut VM<'_>, $context: &mut DispatchContext, $check_limits: bool, $poll_print: bool) -> Status {
                let _ = (&$vm, &$context, $check_limits, $poll_print);
                $body;
                Status::Continue
            }
            vm.current_frame.ip = ip;
            vm.instruction_ip = ip - 1;
            context.chain_remaining=state.to_le_bytes()[0];
            context.fuel=state.to_le_bytes()[2];
            let status = execute(vm, context, state & CHECK_LIMITS != 0, state & POLL_PRINT != 0);
            if status != Status::Continue {
                return status;
            }
            let state=(state & !(255 | FUEL_MASK)) |u32::from(context.chain_remaining) |(u32::from(context.fuel)<<FUEL_SHIFT);
            next::<0>(vm, vm.current_frame.bytecode, vm.current_frame.ip, context, state, table)
        })*
        const SEMANTICS: [Handler; 256] = {
            let mut handlers = [invalid as Handler; 256];
            $( $(#[$attr])* { handlers[Opcode::$opcode as usize] = $name; } )*
            handlers
        };
    };
}

opcode_handlers! {
vm, context, check_limits, poll_print;
#[cfg(feature = "experimental-fusion")]
FusedLoadLocal0LoadLocal1BinaryAdd => fused_load_local0_load_local1_binary_add {
    try_catch!(vm, context, vm.fused_0(&mut context.fuel, check_limits, poll_print));
}
#[cfg(feature = "experimental-fusion")]
FusedLoadLocal1LoadLocal2BinaryAdd => fused_load_local1_load_local2_binary_add {
    try_catch!(vm, context, vm.fused_1(&mut context.fuel, check_limits, poll_print));
}
#[cfg(feature = "experimental-fusion")]
FusedLoadSmallIntBinaryAdd => fused_load_small_int_binary_add {
    try_catch!(vm, context, vm.fused_2(&mut context.fuel, check_limits, poll_print));
}
#[cfg(feature = "experimental-fusion")]
FusedLoadSmallIntBinarySub => fused_load_small_int_binary_sub {
    try_catch!(vm, context, vm.fused_3(&mut context.fuel, check_limits, poll_print));
}
#[cfg(feature = "experimental-fusion")]
FusedLoadSmallIntBinaryMul => fused_load_small_int_binary_mul {
    try_catch!(vm, context, vm.fused_4(&mut context.fuel, check_limits, poll_print));
}
#[cfg(feature = "experimental-fusion")]
FusedLoadSmallIntBinaryMod => fused_load_small_int_binary_mod {
    try_catch!(vm, context, vm.fused_5(&mut context.fuel, check_limits, poll_print));
}
#[cfg(feature = "experimental-fusion")]
FusedCompareLtJumpIfFalse => fused_compare_lt_jump_if_false {
    try_catch!(vm, context, vm.fused_6(&mut context.fuel, check_limits, poll_print));
}
#[cfg(feature = "experimental-fusion")]
FusedCompareEqJumpIfFalse => fused_compare_eq_jump_if_false {
    try_catch!(vm, context, vm.fused_7(&mut context.fuel, check_limits, poll_print));
}
Pop => pop {
    let value = vm.pop();
    value.drop_with(vm);
}
Dup => dup {
    let value = vm.peek().clone_with_heap(vm);
    vm.push(value);
}
Dup2 => dup2 {
    let len = vm.stack.len();
    let first = vm.stack[len - 2].clone_with_heap(vm);
    let second = vm.stack[len - 1].clone_with_heap(vm);
    vm.push(first);
    vm.push(second);
}
Rot2 => rot2 {
    // Swap top two: [a, b] → [b, a]
    let len = vm.stack.len();
    vm.stack.swap(len - 1, len - 2);
}
Rot3 => rot3 {
    // Rotate top three: [a, b, c] → [c, a, b]
    // Uses in-place rotation without cloning
    let len = vm.stack.len();
    // Move c out, then shift a→b→c, then put c at a's position
    // Equivalent to: [..rest, a, b, c] → [..rest, c, a, b]
    vm.stack[len - 3..].rotate_right(1);
}
LoadConst => load_const {
    let idx = vm.current_frame.fetch_u16();
    let value = vm.current_frame.code.constant(idx);
    // Handle InternLongInt specially - convert to heap-allocated LongInt
    if let Value::InternLongInt(long_int_id) = value {
        let bi = vm.interns.get_long_int(*long_int_id).clone();
        let long_value = LongInt::new(bi).into_value(vm.heap);
        vm.push(long_value);
    } else {
        vm.push(value.clone_with_heap(vm));
    }
}
LoadNone => load_none {
    vm.push(Value::None);
}
LoadTrue => load_true {
    vm.push(Value::Bool(true));
}
LoadFalse => load_false {
    vm.push(Value::Bool(false));
}
BuildCell => build_cell {
    let cell_id = vm.heap.allocate(HeapData::Cell(CellValue(Value::Undefined)));
    vm.push(Value::Ref(cell_id));
}
LoadSmallInt => load_small_int {
    let n = vm.current_frame.fetch_i8();
    vm.push(Value::Int(i64::from(n)));
}
LoadLocal0 => load_local0 {
    try_catch!(vm, context, vm.load_local(0));
}
LoadLocal1 => load_local1 {
    try_catch!(vm, context, vm.load_local(1));
}
LoadLocal2 => load_local2 {
    try_catch!(vm, context, vm.load_local(2));
}
LoadLocal3 => load_local3 {
    try_catch!(vm, context, vm.load_local(3));
}
LoadLocal => load_local {
    let slot = u16::from(vm.current_frame.fetch_u8());
    try_catch!(vm, context, vm.load_local(slot));
}
LoadLocalW => load_local_w {
    let slot = vm.current_frame.fetch_u16();
    try_catch!(vm, context, vm.load_local(slot));
}
StoreLocal => store_local {
    let slot = u16::from(vm.current_frame.fetch_u8());
    vm.store_local(slot);
}
StoreLocalW => store_local_w {
    let slot = vm.current_frame.fetch_u16();
    vm.store_local(slot);
}
LiftToTop => lift_to_top {
    let n = vm.current_frame.fetch_u8();
    // Move the item at TOS - n to TOS, shifting items in
    // between down by one. Single `rotate_left(1)` on the
    // affected slice does exactly that.
    let len = vm.stack.len();
    let src_idx = len - 1 - n as usize;
    vm.stack[src_idx..].rotate_left(1);
}
RaiseUnboundLocal => raise_unbound_local {
    let name_idx = vm.current_frame.fetch_u16();
    let name_id = StringId::from_index(name_idx);
    catch!(vm, context, vm.unbound_local_error(0, Some(name_id)));
}
DeleteLocal => delete_local {
    let slot = u16::from(vm.current_frame.fetch_u8());
    vm.delete_local(slot);
}
DeleteGlobal => delete_global {
    let slot = vm.current_frame.fetch_u16();
    try_catch!(vm, context, vm.delete_global(slot));
}
LoadName => load_name {
    let (slot, name_idx, flags) = vm.current_frame.fetch_u16_u16_u8();
    handle_load_result!(vm, context, vm.load_name(slot, StringId::from_index(name_idx), flags));
}
StoreName => store_name {
    let (slot, name_idx, flags) = vm.current_frame.fetch_u16_u16_u8();
    try_catch!(vm, context, vm.store_name(slot, StringId::from_index(name_idx), flags));
}
DeleteName => delete_name {
    let (slot, name_idx, flags) = vm.current_frame.fetch_u16_u16_u8();
    try_catch!(vm, context, vm.delete_name(slot, StringId::from_index(name_idx), flags));
}
LoadGlobal => load_global {
    let slot = vm.current_frame.fetch_u16();
    handle_load_result!(vm, context, vm.load_global(slot));
}
LoadGlobalCallable => load_global_callable {
    let (slot, name_idx) = vm.current_frame.fetch_u16_u16();
    let name_id = StringId::from_index(name_idx);
    vm.load_global_callable(slot, name_id);
}
StoreGlobal => store_global {
    let slot = vm.current_frame.fetch_u16();
    vm.store_global(slot);
}
LoadCell => load_cell {
    let slot = vm.current_frame.fetch_u16();
    try_catch!(vm, context, vm.load_cell(slot));
}
StoreCell => store_cell {
    let slot = vm.current_frame.fetch_u16();
    vm.store_cell(slot);
}
DeleteCell => delete_cell {
    let slot = vm.current_frame.fetch_u16();
    vm.delete_cell(slot);
}
BinaryAdd => binary_add {
    try_catch!(vm, context, vm.binary_add());
}
BinarySub => binary_sub {
    try_catch!(vm, context, vm.binary_sub());
}
BinaryMul => binary_mul {
    try_catch!(vm, context, vm.binary_mult());
}
BinaryDiv => binary_div {
    try_catch!(vm, context, vm.binary_div());
}
BinaryFloorDiv => binary_floor_div {
    try_catch!(vm, context, vm.binary_floordiv());
}
BinaryMod => binary_mod {
    try_catch!(vm, context, vm.binary_mod());
}
BinaryPow => binary_pow {
    try_catch!(vm, context, vm.binary_pow());
}
BinaryAnd => binary_and {
    try_catch!(vm, context, vm.binary_and());
}
BinaryOr => binary_or {
    try_catch!(vm, context, vm.binary_or());
}
BinaryXor => binary_xor {
    try_catch!(vm, context, vm.binary_xor());
}
BinaryLShift => binary_l_shift {
    try_catch!(vm, context, vm.binary_lshift());
}
BinaryRShift => binary_r_shift {
    try_catch!(vm, context, vm.binary_rshift());
}
BinaryMatMul => binary_mat_mul {
    try_catch!(vm, context, vm.binary_matmul());
}
CompareEq => compare_eq {
    try_catch!(vm, context, vm.compare_eq());
}
CompareNe => compare_ne {
    try_catch!(vm, context, vm.compare_ne());
}
CompareLt => compare_lt {
    try_catch!(vm, context, vm.compare_lt());
}
CompareLe => compare_le {
    try_catch!(vm, context, vm.compare_le());
}
CompareGt => compare_gt {
    try_catch!(vm, context, vm.compare_gt());
}
CompareGe => compare_ge {
    try_catch!(vm, context, vm.compare_ge());
}
CompareIs => compare_is {
    try_catch!(vm, context, vm.compare_is());
}
CompareIsNot => compare_is_not {
    try_catch!(vm, context, vm.compare_is_not());
}
CompareIn => compare_in {
    try_catch!(vm, context, vm.compare_in());
}
CompareNotIn => compare_not_in {
    try_catch!(vm, context, vm.compare_not_in());
}
UnaryNot => unary_not {
    let value = vm.pop();
    let result = value.py_bool(vm);
    value.drop_with(vm);
    match result {
        Ok(value) => vm.push(Value::Bool(!value)),
        Err(error) => catch!(vm, context, error),
    }
}
UnaryNeg => unary_neg {
    try_catch!(vm, context, vm.unary_neg());
}
UnaryPos => unary_pos {
    try_catch!(vm, context, vm.unary_pos());
}
UnaryInvert => unary_invert {
    // Bitwise NOT
    let value = vm.pop();
    match value {
        Value::Int(n) => vm.push(Value::Int(!n)),
        Value::Bool(b) => vm.push(Value::Int(!i64::from(b))),
        Value::Ref(id) => {
            if let HeapData::LongInt(li) = vm.heap.get(id) {
                // LongInt bitwise NOT: ~x = -(x + 1)
                let inverted = -(li.inner() + 1i32);
                value.drop_with(vm);
                let inverted_value = LongInt::new(inverted).into_value(vm.heap);
                vm.push(inverted_value);
            } else {
                let value_type = value.py_type_name(vm);
                value.drop_with(vm);
                catch!(vm, context, ExcType::unary_type_error("~", &value_type));
            }
        }
        _ => {
            let value_type = value.py_type_name(vm);
            value.drop_with(vm);
            catch!(vm, context, ExcType::unary_type_error("~", &value_type));
        }
    }
}
InplaceAdd => inplace_add {
    try_catch!(vm, context, vm.inplace_add());
}
InplaceSub => inplace_sub {
    try_catch!(vm, context, vm.inplace_sub());
}
InplaceMul => inplace_mul {
    try_catch!(vm, context, vm.binary_mult());
}
InplaceDiv => inplace_div {
    try_catch!(vm, context, vm.binary_div());
}
InplaceFloorDiv => inplace_floor_div {
    try_catch!(vm, context, vm.binary_floordiv());
}
InplaceMod => inplace_mod {
    try_catch!(vm, context, vm.binary_mod());
}
InplacePow => inplace_pow {
    try_catch!(vm, context, vm.binary_pow());
}
InplaceAnd => inplace_and {
    try_catch!(vm, context, vm.inplace_and());
}
InplaceOr => inplace_or {
    try_catch!(vm, context, vm.inplace_or());
}
InplaceXor => inplace_xor {
    try_catch!(vm, context, vm.binary_xor());
}
InplaceLShift => inplace_l_shift {
    try_catch!(vm, context, vm.binary_lshift());
}
InplaceRShift => inplace_r_shift {
    try_catch!(vm, context, vm.binary_rshift());
}
BuildList => build_list {
    let count = vm.current_frame.fetch_u16() as usize;
    vm.build_list(count);
}
BuildTuple => build_tuple {
    let count = vm.current_frame.fetch_u16() as usize;
    vm.build_tuple(count);
}
BuildDict => build_dict {
    let count = vm.current_frame.fetch_u16() as usize;
    try_catch!(vm, context, vm.build_dict(count));
}
BuildSet => build_set {
    let count = vm.current_frame.fetch_u16() as usize;
    try_catch!(vm, context, vm.build_set(count));
}
FormatValue => format_value {
    let flags = vm.current_frame.fetch_u8();
    try_catch!(vm, context, vm.format_value(flags));
}
BuildFString => build_f_string {
    let count = vm.current_frame.fetch_u16() as usize;
    try_catch!(vm, context, vm.build_fstring(count));
}
BuildSlice => build_slice {
    try_catch!(vm, context, vm.build_slice());
}
ListExtend => list_extend {
    try_catch!(vm, context, vm.list_extend());
}
ListToTuple => list_to_tuple {
    try_catch!(vm, context, vm.list_to_tuple());
}
DictMerge => dict_merge {
    let func_name_id = vm.current_frame.fetch_u16();
    try_catch!(vm, context, vm.dict_merge(func_name_id));
}
MethodDictMerge => method_dict_merge {
    let func_name_id = vm.current_frame.fetch_u16();
    try_catch!(vm, context, vm.method_dict_merge(func_name_id));
}
DictUpdate => dict_update {
    let depth = vm.current_frame.fetch_u8() as usize;
    try_catch!(vm, context, vm.dict_update(depth));
}
SetExtend => set_extend {
    let depth = vm.current_frame.fetch_u8() as usize;
    try_catch!(vm, context, vm.set_extend(depth));
}
ListAppend => list_append {
    let depth = vm.current_frame.fetch_u8() as usize;
    try_catch!(vm, context, vm.list_append(depth));
}
SetAdd => set_add {
    let depth = vm.current_frame.fetch_u8() as usize;
    try_catch!(vm, context, vm.set_add(depth));
}
DictSetItem => dict_set_item {
    let depth = vm.current_frame.fetch_u8() as usize;
    try_catch!(vm, context, vm.dict_set_item(depth));
}
BinarySubscr => binary_subscr {
    let index = vm.pop();
    let obj = vm.pop();
    let result = obj.py_getitem(&index, vm);
    obj.drop_with(vm);
    index.drop_with(vm);
    match result {
        Ok(v) => vm.push(v),
        Err(e) => catch!(vm, context, e),
    }
}
StoreSubscr => store_subscr {
    // Stack order: value, obj, index (TOS)
    let index = vm.pop();
    let mut obj = vm.pop();
    let value = vm.pop();
    let result = obj.py_setitem(index, value, vm);
    obj.drop_with(vm);
    if let Err(e) = result {
        catch!(vm, context, e);
    }
}
LoadAttr => load_attr {
    let name_idx = vm.current_frame.fetch_u16();
    let name_id = StringId::from_index(name_idx);
    handle_call_result!(vm, context, vm.load_attr(name_id));
}
LoadAttrImport => load_attr_import {
    let name_idx = vm.current_frame.fetch_u16();
    let name_id = StringId::from_index(name_idx);
    handle_call_result!(vm, context, vm.load_attr_import(name_id));
}
StoreAttr => store_attr {
    let name_idx = vm.current_frame.fetch_u16();
    let name_id = StringId::from_index(name_idx);
    try_catch!(vm, context, vm.store_attr(name_id));
}
Jump => jump {
    let offset = vm.current_frame.fetch_i16();
    jump_relative!(vm.current_frame.ip, offset);
}
JumpIfTrue => jump_if_true {
    let offset = vm.current_frame.fetch_i16();
    let cond = vm.pop();
    let result = cond.py_bool(vm);
    cond.drop_with(vm);
    match result {
        Ok(true) => jump_relative!(vm.current_frame.ip, offset),
        Ok(false) => {}
        Err(error) => catch!(vm, context, error),
    }
}
JumpIfFalse => jump_if_false {
    let offset = vm.current_frame.fetch_i16();
    let cond = vm.pop();
    let result = cond.py_bool(vm);
    cond.drop_with(vm);
    match result {
        Ok(false) => jump_relative!(vm.current_frame.ip, offset),
        Ok(true) => {}
        Err(error) => catch!(vm, context, error),
    }
}
JumpIfTrueOrPop => jump_if_true_or_pop {
    let offset = vm.current_frame.fetch_i16();
    let value = vm.pop();
    match value.py_bool(vm) {
        Ok(true) => {
            vm.push(value);
            jump_relative!(vm.current_frame.ip, offset);
        }
        Ok(false) => value.drop_with(vm),
        Err(error) => {
            value.drop_with(vm);
            catch!(vm, context, error);
        }
    }
}
JumpIfFalseOrPop => jump_if_false_or_pop {
    let offset = vm.current_frame.fetch_i16();
    let value = vm.pop();
    match value.py_bool(vm) {
        Ok(true) => value.drop_with(vm),
        Ok(false) => {
            vm.push(value);
            jump_relative!(vm.current_frame.ip, offset);
        }
        Err(error) => {
            value.drop_with(vm);
            catch!(vm, context, error);
        }
    }
}
GetIter => get_iter {
    let value = vm.pop();
    let iterator = value.py_iter(vm);
    value.drop_with(vm);
    match iterator {
        Ok(iterator) => vm.push(iterator),
        Err(e) => catch!(vm, context, e),
    }
}
ForIter => for_iter {
    let offset = vm.current_frame.fetch_i16();
    // Iterator implementations return heap objects from `py_iter`.
    let Value::Ref(heap_id) = *vm.peek() else {
        return context.error(RunError::internal("ForIter: expected iterator ref on stack"));
    };
    if matches!(vm.heap.read(heap_id), HeapReadOutput::Generator(_)) {
        let end_ip = vm.current_frame.ip.wrapping_add_signed(isize::from(offset));
        try_catch!(vm, context, vm.resume_generator(heap_id, GeneratorConsumer::For { end_ip }));
        return Status::Continue;
    }
    let mut iter = vm.heap.read(heap_id);

    match iter.py_next(vm) {
        Ok(Some(value)) => vm.push(value),
        Ok(None) => {
            // Drop the HeapRead before dec_ref to release the reader count
            drop(iter);
            // Iterator exhausted - pop it and jump to end
            let iter = vm.pop();
            iter.drop_with(vm);
            jump_relative!(vm.current_frame.ip, offset);
        }
        Err(e) => {
            // Drop the HeapRead before dec_ref to release the reader count
            drop(iter);
            // Error during iteration (e.g., dict size changed)
            let iter = vm.pop();
            iter.drop_with(vm);
            catch!(vm, context, e);
        }
    }
}
CallFunction => call_function {
    let arg_count=vm.current_frame.fetch_u8() as usize;
    #[cfg(all(monty_native,feature="experimental-aot"))]
    let mut skip_entry=false;
    #[cfg(all(monty_native,feature="experimental-aot"))]
    if context.aot_enabled {
        match vm.try_scalar_aot_call(arg_count,&mut context.fuel,check_limits,poll_print) {
            Ok(ScalarCall::Value(value))=>{vm.push(value);return Status::Continue;}
            Ok(ScalarCall::Miss)=>skip_entry=true,
            Ok(ScalarCall::Unsupported)=>{},
            Err(error)=>{catch!(vm,context,error);return Status::Continue;},
        }
    }
    let result=vm.exec_call_function(arg_count);
    #[cfg(all(monty_native,feature="experimental-aot"))]
    if skip_entry && matches!(&result,Ok(CallResult::FramePushed)){vm.current_frame.aot_attempted=true;}
    handle_call_result!(vm,context,result);
}
CallBuiltinFunction => call_builtin_function {
    let (builtin_id, arg_count) = vm.current_frame.fetch_u8_u8();
    let result = vm.exec_call_builtin_function(builtin_id, arg_count as usize);
    handle_call_result!(vm, context, result);
}
CallBuiltinType => call_builtin_type {
    let (type_id, arg_count) = vm.current_frame.fetch_u8_u8();
    let arg_count = arg_count as usize;

    match vm.exec_call_builtin_type(type_id, arg_count) {
        Ok(result) => vm.push(result),
        Err(err) => catch!(vm, context, err),
    }
}
CallFunctionKw => call_function_kw {
    // Fetch operands: pos_count, kw_count, then kw_count name indices
    let (pos_count, kw_count) = vm.current_frame.fetch_u8_u8();
    let (pos_count, kw_count) = (pos_count as usize, kw_count as usize);

    // Read keyword name StringIds
    let mut kwname_ids = Vec::with_capacity(kw_count);
    for _ in 0..kw_count {
        kwname_ids.push(StringId::from_index(vm.current_frame.fetch_u16()));
    }

    handle_call_result!(vm, context, vm.exec_call_function_kw(pos_count, kwname_ids));
}
CallAttr => call_attr {
    // CallAttr: u16 name_id, u8 arg_count
    // Stack: [obj, arg1, arg2, ..., argN] -> [result]
    let (name_idx, arg_count) = vm.current_frame.fetch_u16_u8();
    let name_id = StringId::from_index(name_idx);
    let arg_count = arg_count as usize;

    handle_call_result!(vm, context, vm.exec_call_attr(name_id, arg_count));
}
CallAttrKw => call_attr_kw {
    // CallAttrKw: u16 name_id, u8 pos_count, u8 kw_count, then kw_count u16 name indices
    // Stack: [obj, pos_args..., kw_values...] -> [result]
    let (name_idx, pos_count, kw_count) = vm.current_frame.fetch_u16_u8_u8();
    let name_id = StringId::from_index(name_idx);
    let (pos_count, kw_count) = (pos_count as usize, kw_count as usize);

    // Read keyword name StringIds
    let mut kwname_ids = Vec::with_capacity(kw_count);
    for _ in 0..kw_count {
        kwname_ids.push(StringId::from_index(vm.current_frame.fetch_u16()));
    }

    handle_call_result!(vm, context, vm.exec_call_attr_kw(name_id, pos_count, kwname_ids));
}
CallFunctionExtended => call_function_extended {
    let flags = vm.current_frame.fetch_u8();
    let has_kwargs = (flags & 0x01) != 0;

    handle_call_result!(vm, context, vm.exec_call_function_extended(has_kwargs));
}
CallAttrExtended => call_attr_extended {
    let (name_idx, flags) = vm.current_frame.fetch_u16_u8();
    let name_id = StringId::from_index(name_idx);
    let has_kwargs = (flags & 0x01) != 0;

    handle_call_result!(vm, context, vm.exec_call_attr_extended(name_id, has_kwargs));
}
MakeFunction => make_function {
    let (func_idx, defaults_count) = vm.current_frame.fetch_u16_u8();
    let func_id = FunctionId::from_index(func_idx);
    let defaults_count = defaults_count as usize;

    // A function defined under an explicit globals dict carries it.
    let globals = vm
        .current_frame
        .namespace
        .as_deref()
        .and_then(FrameNamespace::dict_globals);
    if defaults_count == 0 && globals.is_none() {
        // No defaults - use inline Value::Function (no heap allocation)
        vm.push(Value::DefFunction(func_id));
    } else {
        // Pop default values from stack (drain maintains order: first pushed = first in vec)
        let defaults = vm.pop_n(defaults_count);
        if let Some(globals) = globals {
            vm.heap.inc_ref(globals);
        }

        // Create FunctionDefaults on heap and push reference
        let heap_id = vm.heap.allocate(HeapData::FunctionDefaults(FunctionDefaults {
            func_id,
            defaults,
            globals,
        }));
        vm.push(Value::Ref(heap_id));
    }
}
MakeClosure => make_closure {
    let (func_idx, defaults_count, cell_count) = vm.current_frame.fetch_u16_u8_u8();
    let func_id = FunctionId::from_index(func_idx);
    let (defaults_count, cell_count) = (defaults_count as usize, cell_count as usize);

    // Pop cells from stack (pushed after defaults, so on top)
    // Cells are Value::Ref pointing to HeapData::Cell
    // We use individual pops which reverses order, so we need to reverse back
    let mut cells = Vec::with_capacity(cell_count);
    for _ in 0..cell_count {
        // mut needed for dec_ref_forget when memory-model-checks feature is enabled
        #[cfg_attr(not(feature = "memory-model-checks"), expect(unused_mut))]
        let mut cell_val = vm.pop();
        match &cell_val {
            Value::Ref(heap_id) => {
                // Keep the reference - the Closure will own the HeapId
                cells.push(*heap_id);
                // Mark the Value as dereferenced since Closure takes ownership
                // of the reference count (we don't call drop_with because
                // we're not decrementing the refcount, just transferring it)
                #[cfg(feature = "memory-model-checks")]
                cell_val.dec_ref_forget();
            }
            _ => {
                return context.error(RunError::internal("MakeClosure: expected cell reference on stack"));
            }
        }
    }
    // Reverse to get original order (individual pops reverse the order)
    cells.reverse();

    // Pop default values from stack (drain maintains order: first pushed = first in vec)
    let defaults = vm.pop_n(defaults_count);
    let globals = vm
        .current_frame
        .namespace
        .as_deref()
        .and_then(FrameNamespace::dict_globals);
    if let Some(globals) = globals {
        vm.heap.inc_ref(globals);
    }

    // Create Closure on heap and push reference
    let heap_id = vm.heap.allocate(HeapData::Closure(Closure {
        func_id,
        cells: cells.into_boxed_slice(),
        defaults: defaults.into_boxed_slice(),
        globals,
    }));
    vm.push(Value::Ref(heap_id));
}
Raise => raise {
    let exc = vm.pop();
    let error = vm.make_exception(&exc, true); // is_raise=true, hide caret
    // Re-raise an instance as-is so `raise e` preserves `e`'s
    // identity, like CPython. A bare type or non-exception has
    // nothing to reuse and rebuilds from the error.
    let raised = match &exc {
        Value::Ref(id) if matches!(vm.heap.get(*id), HeapData::Exception(_)) => Some(exc),
        _ => {
            exc.drop_with(vm);
            None
        }
    };
    if let Some(result) = vm.handle_exception_with_value(error, raised) {
        return context.error(result);
    }
    yield_if_parked!(vm, context);
}
Assert => assert {
    match decode_assert_flags(vm.current_frame.fetch_u8()).expect("invalid assert flags in bytecode")
    {
        Some(op) => try_catch!(vm, context, vm.assert_cmp(op)),
        None => try_catch!(vm, context, vm.assert_test()),
    }
}
AssertFailed => assert_failed {
    let cmp_op =
        decode_assert_flags(vm.current_frame.fetch_u8()).expect("invalid assert flags in bytecode");
    let error = vm.assert_failed_msg(cmp_op);
    catch!(vm, context, error);
}
Reraise => reraise {
    // Clone rather than pop: a locally caught bare raise must
    // preserve its enclosing handler's active exception.
    let raised = vm.exception_stack.last().map(|exc| exc.clone_with_heap(vm.heap));
    let error = match &raised {
        Some(exc) => vm.make_exception(exc, true), // is_raise=true for reraise
        // No active exception - create a RuntimeError
        None => {
            SimpleException::new_msg(ExcType::RuntimeError, "No active exception to reraise").into()
        }
    };
    if let Some(result) = vm.handle_exception_with_value(error, raised) {
        return context.error(result);
    }
    yield_if_parked!(vm, context);
}
ClearException => clear_exception {
    // Pop the current exception from the stack
    // This restores the previous exception context (if any)
    if let Some(exc) = vm.exception_stack.pop() {
        exc.drop_with(vm);
    }
}
CheckExcMatch => check_exc_match {
    // Stack: [exception, exc_type] -> [exception, bool]
    let exc_type = vm.pop();
    let exception = vm.peek();
    let result = vm.check_exc_match(exception, &exc_type);
    exc_type.drop_with(vm);
    match result {
        Ok(matched) => vm.push(Value::Bool(matched)),
        // An invalid `except` type (e.g. `except 123:` or a
        // nested tuple) raises a `TypeError`. As in CPython this
        // is an ordinary exception raised while evaluating the
        // clause: it propagates out of the whole `try` and may be
        // caught by an enclosing handler, so route it through the
        // exception machinery rather than aborting the run.
        Err(err) => catch!(vm, context, err),
    }
}
YieldFrom => yield_from {
match vm.yield_from() {
                    Ok(Some(value)) => return context.exit(FrameExit::Return(value)),
                    Ok(None) => {}
                    Err(error) => catch!(vm, context, error),
                }
}
YieldValue => yield_value {
    if let Some(value) = vm.yield_generator() {
        return context.exit(FrameExit::Return(value));
    }
}
ReturnValue => return_value {
    let value = vm.pop();
    if vm.current_frame.is_yield_from {
        vm.pop_frame();
        vm.push(value);
        if let Some(value) = vm.yield_generator() {
            return context.exit(FrameExit::Return(value));
        }
        return Status::Continue;
    }
    if vm.current_frame.generator.is_some() {
        match vm.return_generator(value) {
            GeneratorReturn::Continue => {}
            GeneratorReturn::Stop(value) => return context.exit(FrameExit::Return(value)),
            GeneratorReturn::Raise { exception, stop } => {
                let error = vm.make_exception(&exception, false);
                if stop {
                    exception.drop_with(vm);
                    return context.error(error);
                }
                if let Some(error) = vm.handle_exception_with_value(error, Some(exception)) {
                    return context.error(error);
                }
                yield_if_parked!(vm, context);
            }
        }
        return Status::Continue;
    }
    if vm.suspended_frames.is_empty() {
        // Last frame - check if this is main task or spawned task
        let is_main_task = vm.is_main_task();

        if is_main_task {
            // Module-level return - we're done
            return context.exit(FrameExit::Return(value));
        }

        // Spawned task completed - handle task completion
        let result = vm.handle_task_completion(value);
        match result {
            Ok(AwaitResult::ValueReady(v)) => {
                vm.push(v);
            }
            Ok(AwaitResult::FramePushed) => {}
            Ok(AwaitResult::Yield(pending)) => {
                // All tasks blocked - return to host
                return context.exit(FrameExit::ResolveFutures(pending));
            }
            Err(e) => catch!(vm, context, e),
        }
        return Status::Continue;
    }
    // Read the initializer flag before popping the frame.
    let is_init = vm.current_frame().is_initializer;
    // Pop current frame; `stop` requests returning to the host
    // (e.g. `evaluate_function`).
    let stop = vm.pop_frame();
    if is_init {
        if !matches!(value, Value::None) {
            // CPython raises at the `Foo(...)` call site: the
            // initializer frame is already popped, so the traceback
            // matches (no `__init__` frame).
            let type_name = value.py_type_name(vm);
            value.drop_with(vm);
            let err = ExcType::type_error_init_return(type_name);
            if stop {
                // The initializer was driven by `evaluate_function`
                // and its frame boundary is already popped —
                // propagate directly rather than unwinding into
                // frames that must not observe this error. The
                // pending instance left on the operand stack is
                // reclaimed by the eventual `handle_exception`
                // stack drain (or final teardown).
                return context.error(err);
            }
            catch!(vm, context, err);
            return Status::Continue;
        }
        // `__init__` returned None — discard it. The instance was
        // pushed onto the caller's stack before this frame ran and
        // is the real result of `Foo(...)`.
        value.drop_with(vm);
        if stop {
            let instance = vm.pop();
            return context.exit(FrameExit::Return(instance));
        }
        // Instance already on the caller's stack — push nothing.
    } else if stop {
        // This frame indicated evaluation should stop - return to host with value
        // e.g. `evaluate_function`
        return context.exit(FrameExit::Return(value));
    } else {
        vm.push(value);
    }
}
Await => op_await {
    let result = vm.exec_get_awaitable();
    match result {
        Ok(AwaitResult::ValueReady(value)) => {
            vm.push(value);
        }
        Ok(AwaitResult::FramePushed) => {}
        Ok(AwaitResult::Yield(pending_calls)) => {
            // All tasks are blocked - return control to host
            return context.exit(FrameExit::ResolveFutures(pending_calls));
        }
        Err(e) => {
            catch!(vm, context, e);
        }
    }
}
UnpackSequence => unpack_sequence {
    let count = vm.current_frame.fetch_u8() as usize;
    try_catch!(vm, context, vm.unpack_sequence(count));
}
UnpackEx => unpack_ex {
    let (before, after) = vm.current_frame.fetch_u8_u8();
    try_catch!(vm, context, vm.unpack_ex(before as usize, after as usize));
}
Nop => nop {
    // No operation
}
LoadModule => load_module {
    let module_id = vm.current_frame.fetch_u16();
    try_catch!(vm, context, vm.load_module(module_id));
}
BeforeWith => before_with {
    handle_call_result!(vm, context, vm.exec_before_with());
}
WithExit => with_exit {
    handle_call_result!(vm, context, vm.exec_with_exit());
}
WithExceptStart => with_except_start {
    handle_call_result!(vm, context, vm.exec_with_except_start());
}
}

const HANDLERS: DispatchTable = DispatchTable {
    handlers: {
        let mut handlers = SEMANTICS;
        handlers[Opcode::Nop as usize] = native::<{ Opcode::Nop as u8 }>;
        handlers[Opcode::LoadNone as usize] = native::<{ Opcode::LoadNone as u8 }>;
        handlers[Opcode::LoadTrue as usize] = native::<{ Opcode::LoadTrue as u8 }>;
        handlers[Opcode::LoadFalse as usize] = native::<{ Opcode::LoadFalse as u8 }>;
        handlers[Opcode::LoadSmallInt as usize] = native::<{ Opcode::LoadSmallInt as u8 }>;
        handlers[Opcode::LoadConst as usize] = native::<{ Opcode::LoadConst as u8 }>;
        handlers[Opcode::LoadLocal0 as usize] = native::<{ Opcode::LoadLocal0 as u8 }>;
        handlers[Opcode::LoadLocal1 as usize] = native::<{ Opcode::LoadLocal1 as u8 }>;
        handlers[Opcode::LoadLocal2 as usize] = native::<{ Opcode::LoadLocal2 as u8 }>;
        handlers[Opcode::LoadLocal3 as usize] = native::<{ Opcode::LoadLocal3 as u8 }>;
        handlers[Opcode::LoadLocal as usize] = native::<{ Opcode::LoadLocal as u8 }>;
        handlers[Opcode::LoadLocalW as usize] = native::<{ Opcode::LoadLocalW as u8 }>;
        handlers[Opcode::StoreLocal as usize] = native::<{ Opcode::StoreLocal as u8 }>;
        handlers[Opcode::StoreLocalW as usize] = native::<{ Opcode::StoreLocalW as u8 }>;
        handlers[Opcode::Pop as usize] = native::<{ Opcode::Pop as u8 }>;
        handlers[Opcode::Dup as usize] = native::<{ Opcode::Dup as u8 }>;
        handlers[Opcode::Rot2 as usize] = native::<{ Opcode::Rot2 as u8 }>;
        handlers[Opcode::Dup2 as usize] = native::<{ Opcode::Dup2 as u8 }>;
        handlers[Opcode::Rot3 as usize] = native::<{ Opcode::Rot3 as u8 }>;
        handlers[Opcode::LiftToTop as usize] = native::<{ Opcode::LiftToTop as u8 }>;
        handlers[Opcode::Jump as usize] = native::<{ Opcode::Jump as u8 }>;
        handlers[Opcode::JumpIfTrue as usize] = native::<{ Opcode::JumpIfTrue as u8 }>;
        handlers[Opcode::JumpIfFalse as usize] = native::<{ Opcode::JumpIfFalse as u8 }>;
        handlers[Opcode::JumpIfTrueOrPop as usize] = native::<{ Opcode::JumpIfTrueOrPop as u8 }>;
        handlers[Opcode::JumpIfFalseOrPop as usize] = native::<{ Opcode::JumpIfFalseOrPop as u8 }>;
        handlers
    },
    checkpoints: [checkpoint::<0>, checkpoint::<1>, checkpoint::<2>, checkpoint::<3>],
};

impl VM<'_> {
    #[inline(never)]
    pub(super) fn dispatch_threaded(
        &mut self,
        countdown: &mut u8,
        check_limits: bool,
        poll_print: bool,
    ) -> Result<Option<FrameExit>, RunError> {
        if *countdown == 0 {
            self.dispatch_checkpoint(check_limits, poll_print)?;
            *countdown = 255;
        }
        let fuel = *countdown - 1;
        let chain_remaining = if self.run_reentry_depth < recursion::MAX_RUN_REENTRY_DEPTH {
            14
        } else {
            254
        };
        let state = u32::from(chain_remaining)
            | (u32::from(fuel) << FUEL_SHIFT)
            | if check_limits { CHECK_LIMITS } else { 0 }
            | if poll_print { POLL_PRINT } else { 0 };
        let mut context = DispatchContext {
            chain_remaining,
            fuel,
            #[cfg(all(monty_native, feature = "experimental-aot"))]
            aot_enabled: typed_aot::enabled(),
            #[cfg(all(monty_native, feature = "experimental-aot"))]
            limits_and_print: (check_limits, poll_print),
            outcome: None,
        };
        let status = fetched(
            self,
            self.current_frame.bytecode,
            self.current_frame.ip,
            &mut context,
            state,
            &HANDLERS,
        );
        *countdown = context.fuel;
        match status {
            Status::Continue => Ok(None),
            _ => context
                .outcome
                .take()
                .expect("exit status requires a payload")
                .map(Some),
        }
    }
}

#[expect(clippy::inline_always, reason = "Each handler must end in its own dispatch.")]
#[inline(always)]
fn next<const WIDTH: usize>(
    vm: &mut VM<'_>,
    bytes: &[u8],
    ip: usize,
    context: &mut DispatchContext,
    state: u32,
    table: &DispatchTable,
) -> Status {
    if state.to_le_bytes()[0] == 0 || state.to_le_bytes()[2] == 0 {
        return (table.checkpoints[WIDTH])(vm, bytes, ip, context, state, table);
    }
    fetched(vm, bytes, ip, context, state - 1 - (1 << FUEL_SHIFT), table)
}

#[expect(clippy::inline_always, reason = "Keep instruction fetch in each handler.")]
#[inline(always)]
fn fetched(
    vm: &mut VM<'_>,
    bytes: &[u8],
    ip: usize,
    context: &mut DispatchContext,
    state: u32,
    table: &DispatchTable,
) -> Status {
    let byte = bytes[ip];
    (table.handlers[usize::from(byte)])(vm, bytes, ip + 1, context, state, table)
}

#[cold]
#[inline(never)]
fn checkpoint<const WIDTH: usize>(
    vm: &mut VM<'_>,
    bytes: &[u8],
    ip: usize,
    context: &mut DispatchContext,
    state: u32,
    table: &DispatchTable,
) -> Status {
    vm.current_frame.ip = ip;
    if WIDTH != 0 {
        vm.instruction_ip = ip.checked_sub(WIDTH).expect("opcode start precedes bytecode");
    }
    context.fuel = state.to_le_bytes()[2];
    if context.fuel == 0 {
        if let Err(error) = vm.dispatch_checkpoint(state & CHECK_LIMITS != 0, state & POLL_PRINT != 0) {
            return context.error(error);
        }
        context.fuel = 255;
    }
    // Only the handler chain ends here. Native execution can replenish source
    // fuel without extending this bounded chain or forcing an extra resource poll.
    let _ = (bytes, table);
    Status::Continue
}

fn invalid(_: &mut VM<'_>, _: &[u8], _: usize, _: &mut DispatchContext, _: u32, _: &DispatchTable) -> Status {
    panic!("invalid opcode in bytecode")
}

#[inline(never)]
fn native<const OP: u8>(
    vm: &mut VM<'_>,
    bytes: &[u8],
    mut ip: usize,
    context: &mut DispatchContext,
    state: u32,
    table: &DispatchTable,
) -> Status {
    let base = vm.current_frame.stack_base();
    match OP {
        byte if byte == Opcode::Nop as u8 => {}
        byte if byte == Opcode::LoadNone as u8 => vm.push(Value::None),
        byte if byte == Opcode::LoadTrue as u8 => vm.push(Value::Bool(true)),
        byte if byte == Opcode::LoadFalse as u8 => vm.push(Value::Bool(false)),
        byte if byte == Opcode::LoadSmallInt as u8 => {
            let value = bytes[ip].cast_signed();
            ip += 1;
            vm.push(Value::Int(i64::from(value)));
        }
        byte if byte == Opcode::LoadConst as u8 => {
            let slot = u16::from_le_bytes([bytes[ip], bytes[ip + 1]]);
            let value = vm.current_frame.code.constant(slot);
            if matches!(value, Value::InternLongInt(_)) {
                return (SEMANTICS[usize::from(OP)])(vm, bytes, ip, context, state, table);
            }
            let value = value.clone_with_heap(vm.heap);
            vm.push(value);
            ip += 2;
        }
        byte if byte == Opcode::LoadLocal0 as u8
            || byte == Opcode::LoadLocal1 as u8
            || byte == Opcode::LoadLocal2 as u8
            || byte == Opcode::LoadLocal3 as u8
            || byte == Opcode::LoadLocal as u8
            || byte == Opcode::LoadLocalW as u8 =>
        {
            let (slot, operand) = if OP == Opcode::LoadLocal0 as u8 {
                (0, 0)
            } else if OP == Opcode::LoadLocal1 as u8 {
                (1, 0)
            } else if OP == Opcode::LoadLocal2 as u8 {
                (2, 0)
            } else if OP == Opcode::LoadLocal3 as u8 {
                (3, 0)
            } else if OP == Opcode::LoadLocal as u8 {
                (usize::from(bytes[ip]), 1)
            } else {
                (usize::from(u16::from_le_bytes([bytes[ip], bytes[ip + 1]])), 2)
            };
            if matches!(vm.stack[base + slot], Value::Undefined) {
                return (SEMANTICS[usize::from(OP)])(vm, bytes, ip, context, state, table);
            }
            let value = vm.stack[base + slot].clone_with_heap(vm.heap);
            vm.push(value);
            ip += operand;
        }
        byte if byte == Opcode::StoreLocal as u8 || byte == Opcode::StoreLocalW as u8 => {
            let (slot, operand) = if OP == Opcode::StoreLocal as u8 {
                (usize::from(bytes[ip]), 1)
            } else {
                (usize::from(u16::from_le_bytes([bytes[ip], bytes[ip + 1]])), 2)
            };
            let value = vm.pop();
            let old = mem::replace(&mut vm.stack[base + slot], value);
            old.drop_with(vm);
            ip += operand;
        }
        byte if byte == Opcode::Pop as u8 => vm.pop().drop_with(vm),
        byte if byte == Opcode::Dup as u8 => {
            let value = vm.peek().clone_with_heap(vm.heap);
            vm.push(value);
        }
        byte if byte == Opcode::Rot2 as u8 => {
            let len = vm.stack.len();
            vm.stack.swap(len - 1, len - 2);
        }
        byte if byte == Opcode::Dup2 as u8 => {
            let len = vm.stack.len();
            let first = vm.stack[len - 2].clone_with_heap(vm.heap);
            let second = vm.stack[len - 1].clone_with_heap(vm.heap);
            vm.push(first);
            vm.push(second);
        }
        byte if byte == Opcode::Rot3 as u8 => {
            let len = vm.stack.len();
            vm.stack[len - 3..].rotate_right(1);
        }
        byte if byte == Opcode::LiftToTop as u8 => {
            let count = usize::from(bytes[ip]);
            ip += 1;
            let start = vm.stack.len() - 1 - count;
            vm.stack[start..].rotate_left(1);
        }
        byte if byte == Opcode::Jump as u8
            || byte == Opcode::JumpIfTrue as u8
            || byte == Opcode::JumpIfFalse as u8
            || byte == Opcode::JumpIfTrueOrPop as u8
            || byte == Opcode::JumpIfFalseOrPop as u8 =>
        {
            let start = ip - 1;
            let taken = if OP == Opcode::Jump as u8 {
                true
            } else {
                // Immediate truth values cannot execute Python; other operands use the semantic handler.
                let truth = match vm.peek() {
                    Value::Bool(value) => *value,
                    Value::Int(value) => *value != 0,
                    Value::None => false,
                    _ => return (SEMANTICS[usize::from(OP)])(vm, bytes, ip, context, state, table),
                };
                let taken = truth == (OP == Opcode::JumpIfTrue as u8 || OP == Opcode::JumpIfTrueOrPop as u8);
                if !taken || (OP != Opcode::JumpIfTrueOrPop as u8 && OP != Opcode::JumpIfFalseOrPop as u8) {
                    vm.pop().drop_with(vm);
                }
                taken
            };
            let offset = i16::from_le_bytes([bytes[ip], bytes[ip + 1]]);
            ip += 2;
            if taken {
                jump_relative!(ip, offset);
            }
            // A branch target cannot reconstruct the source instruction's location.
            if state.to_le_bytes()[0] == 0 {
                vm.instruction_ip = start;
            }
            return next::<0>(vm, bytes, ip, context, state, table);
        }
        _ => unreachable!(),
    }
    // The opcode width is constant in each specialized handler; rebuild its start only at a checkpoint.
    if OP == Opcode::LoadConst as u8 || OP == Opcode::LoadLocalW as u8 || OP == Opcode::StoreLocalW as u8 {
        next::<3>(vm, bytes, ip, context, state, table)
    } else if OP == Opcode::LoadSmallInt as u8
        || OP == Opcode::LoadLocal as u8
        || OP == Opcode::StoreLocal as u8
        || OP == Opcode::LiftToTop as u8
    {
        next::<2>(vm, bytes, ip, context, state, table)
    } else {
        next::<1>(vm, bytes, ip, context, state, table)
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use strum::IntoEnumIterator;

    use super::{Handler, Opcode, SEMANTICS, invalid};

    #[test]
    fn every_opcode_has_a_handler() {
        for opcode in Opcode::iter() {
            assert!(
                !ptr::fn_addr_eq(SEMANTICS[opcode as usize], invalid as Handler),
                "{opcode:?}"
            );
        }
    }
}
