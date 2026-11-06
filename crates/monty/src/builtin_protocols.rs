//! Python-visible adapters for explicit calls to native special methods.
//!
//! Each type registers these shared adapters in its own `PyAttrs` implementation.
//! Lookup and enumeration use that single generated table.
//! The same adapter serves `value.__add__(other)` and `int.__add__(value, other)`;
//! `py_attrs::call_unbound` validates and removes the latter's explicit receiver.
//!
//! Operators still use `PyTrait` directly. An explicit method call must preserve
//! its one-sided result: `int.__add__(1, 2.0)` returns `NotImplemented`, while
//! `1 + 2.0` can try the float implementation. Sequence descriptors instead raise
//! their operand errors. The adapters below supply those calling rules without
//! changing native operator dispatch or allocating Python bound-method objects.
//!
//! Keep registration lists explicit: a `PyTrait` default does not establish that
//! CPython exposes that method on the corresponding type. Initialization and
//! standalone method objects are outside these tables.

use crate::{
    args::ArgValues,
    builtins::{BuiltinsFunctions, BuiltinsFunctionsExt, next::builtin_next},
    bytecode::{CallResult, VM},
    defer_drop, defer_drop_mut,
    exception_private::{ExcType, ExcTypeExt, RunResult},
    expressions::CmpOperator,
    heap::{DropGuard, DropWithContext, HeapReadOutput},
    py_attrs::AttrDef,
    py_value::PyValue,
    types::{CmpOrder, PyTrait, Type, bytes::allocate_bytes, dict},
    value::{EitherStr, Value},
};

/// Adapts zero-argument native hooks whose optional result represents a declined operation.
/// Registration determines availability; the macro does not add a hook to every native type.
macro_rules! unary {
    ($($name:ident => $method:ident),* $(,)?) => {$ (
        pub(crate) fn $name<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
            args.check_zero_args(concat!("__", stringify!($name), "__"), vm.heap)?;
            let result = this.$method(vm)?.unwrap_or(Value::NotImplemented);
            Ok(CallResult::Value(result))
        }
    )*};
}
unary! {
    neg => py_neg_impl, pos => py_pos_impl, abs => py_abs_impl, invert => py_invert_impl,
    index => py_index_impl, int => py_int_impl, floor => py_floor_impl,
    ceil => py_ceil_impl, trunc => py_trunc_impl,
}

/// One native implementation, before reflected dispatch or an operator's final TypeError.
type BinaryOperation<'h> = fn(&PyValue<'h>, &Value, &mut VM<'h>) -> RunResult<Option<Value>>;

/// Applies descriptor operand rules, then calls one native binary implementation.
/// The additional paths cover accepted native representations and reflected methods;
/// returning `NotImplemented` remains observable to the Python caller.
fn binary_call<'h>(
    this: &mut PyValue<'h>,
    args: ArgValues,
    vm: &mut VM<'h>,
    name: &str,
    operation: BinaryOperation<'h>,
) -> RunResult<CallResult> {
    let other = args.get_one_arg(&format!("__{name}__"), vm.heap)?;
    defer_drop!(other, vm);
    let lhs_type = this.py_type(vm);
    let rhs_type = other.py_type(vm);
    // Integer descriptors accept integers and bools; operators can try a float's reflected method.
    if matches!(lhs_type, Type::Int | Type::Bool) && !matches!(rhs_type, Type::Int | Type::Bool) {
        return Ok(CallResult::Value(Value::NotImplemented));
    }
    // Float helpers need a numeric value even when the operand is an interned or heap big integer.
    // Conversion can raise OverflowError before the arithmetic helper runs.
    let converted = if lhs_type == Type::Float && matches!(rhs_type, Type::Int | Type::Bool) {
        Some(Value::Float(
            other.py_value(vm).py_float_impl(vm)?.expect("integer float conversion"),
        ))
    } else {
        None
    };
    let sequence = matches!(
        lhs_type,
        Type::List | Type::Tuple | Type::NamedTuple | Type::Str | Type::Bytes | Type::Deque
    );
    let repeat = if sequence && matches!(name, "mul" | "rmul") {
        let count = other.as_int_with_overflow(vm, || ExcType::overflow_index_sized_int().into())?;
        // Validate first, then avoid native repetition loops over an empty sequence.
        Some(Value::Int(if this.py_len(vm) == Some(0) { 0 } else { count }))
    } else {
        None
    };
    let operand = repeat.as_ref().or(converted.as_ref()).unwrap_or(other);
    // These reflected operations are symmetric within their accepted native types.
    // Calling a reflected method explicitly cannot rely on the VM's operator loop to supply them.
    let mut result = match name {
        "rand" if matches!(lhs_type, Type::Int | Type::Bool) => this.py_and_impl(operand, vm)?,
        "ror" if matches!(lhs_type, Type::Int | Type::Bool) => this.py_or_impl(operand, vm)?,
        "rxor" if matches!(lhs_type, Type::Int | Type::Bool) => this.py_xor_impl(operand, vm)?,
        "radd"
            if matches!(
                lhs_type,
                Type::Int | Type::Bool | Type::Float | Type::Date | Type::DateTime | Type::TimeDelta
            ) =>
        {
            this.py_add_impl(operand, vm)?
        }
        "rmul" if matches!(lhs_type, Type::Int | Type::Bool | Type::Float) => this.py_mul_impl(operand, vm)?,
        _ => operation(this, operand, vm)?,
    };
    // Small-integer helpers defer some big-integer operands to the other storage representation.
    if result.is_none() && matches!(lhs_type, Type::Int | Type::Bool) && matches!(name, "add" | "radd" | "mul" | "rmul")
    {
        let value = this.to_owned_value(vm);
        defer_drop!(value, vm);
        result = if matches!(name, "add" | "radd") {
            operand.py_value(vm).py_add_impl(value, vm)?
        } else {
            operand.py_value(vm).py_mul_impl(value, vm)?
        };
    }
    // Timedelta's native helpers implement the left-hand forms. Swapping two timedeltas
    // supplies the reflected forms without broadening their accepted operand types.
    if result.is_none()
        && matches!(name, "rsub" | "rtruediv" | "rfloordiv" | "rmod" | "rdivmod")
        && lhs_type == Type::TimeDelta
        && rhs_type == Type::TimeDelta
    {
        let value = this.to_owned_value(vm);
        defer_drop!(value, vm);
        result = match name {
            "rsub" => other.py_value(vm).py_sub_impl(value, vm)?,
            "rtruediv" => other.py_value(vm).py_truediv_impl(value, vm)?,
            "rfloordiv" => other.py_value(vm).py_floordiv_impl(value, vm)?,
            "rmod" => other.py_value(vm).py_mod_impl(value, vm)?,
            "rdivmod" => other.py_value(vm).py_divmod_impl(value, vm)?,
            _ => unreachable!(),
        };
    }
    // The left set operand determines whether a reflected result is mutable or frozen.
    if result.is_none() && matches!(lhs_type, Type::Set | Type::FrozenSet) {
        let value = this.to_owned_value(vm);
        defer_drop!(value, vm);
        result = match name {
            "rsub" => other.py_value(vm).py_sub_impl(value, vm)?,
            "rand" => other.py_value(vm).py_and_impl(value, vm)?,
            "ror" => other.py_value(vm).py_or_impl(value, vm)?,
            "rxor" => other.py_value(vm).py_xor_impl(value, vm)?,
            _ => None,
        };
    }
    // Defaultdict's reflected union preserves factory metadata through the native dict union helper.
    // Plain dict and Counter's inherited descriptor have separate adapters below.
    if result.is_none() && name == "ror" && matches!(lhs_type, Type::Dict | Type::DefaultDict | Type::Counter) {
        let value = this.to_owned_value(vm);
        defer_drop!(value, vm);
        if let Some(HeapReadOutput::Dict(left)) = other.read_heap(vm) {
            result = dict::dict_or(&left, value, vm)?;
        }
    }
    // A sequence's __add__ raises here; its descriptor does not return NotImplemented
    // and leave the caller to reproduce the operator's final error.
    if result.is_none() && sequence && name == "add" {
        return Err(ExcType::binary_type_error(
            "+",
            lhs_type,
            lhs_type.name(vm.heap, vm.interns),
            other.py_type_name(vm),
        ));
    }
    Ok(CallResult::Value(result.unwrap_or(Value::NotImplemented)))
}

/// Gives each registered binary name an adapter while sharing operand validation.
/// `modulo` is a Rust helper name only; Python sees `__mod__`.
macro_rules! binary {
    ($($name:ident => $method:ident),* $(,)?) => {$ (
        pub(crate) fn $name<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
            let name = if stringify!($name) == "modulo" { "mod" } else { stringify!($name) };
            binary_call(this, args, vm, name, |this, other, vm| this.$method(other, vm))
        }
    )*};
}
binary! {
    add => py_add_impl, radd => py_radd_impl, sub => py_sub_impl, rsub => py_rsub_impl,
    mul => py_mul_impl, rmul => py_rmul_impl, truediv => py_truediv_impl, rtruediv => py_rtruediv_impl,
    floordiv => py_floordiv_impl, rfloordiv => py_rfloordiv_impl, modulo => py_mod_impl, rmod => py_rmod_impl,
    divmod => py_divmod_impl, rdivmod => py_rdivmod_impl,
    and => py_and_impl, rand => py_rand_impl, or => py_or_impl, ror => py_ror_impl,
    xor => py_xor_impl, rxor => py_rxor_impl, lshift => py_lshift_impl, rlshift => py_rlshift_impl,
    rshift => py_rshift_impl, rrshift => py_rrshift_impl,
}

/// Reuses native in-place mutation and returns the original receiver on success.
/// A false result becomes NotImplemented; an explicit call does not try binary fallback.
macro_rules! inplace {
    ($($name:ident => $method:ident),* $(,)?) => {$ (
        pub(crate) fn $name<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
            let other = args.get_one_arg(concat!("__", stringify!($name), "__"), vm.heap)?;
            defer_drop!(other, vm);
            let value = this.to_owned_value(vm);
            // Mutation may raise after changing the receiver. Retain its owned reference
            // on success and release it on every error or declined-operation path.
            let mut pending = DropGuard::new(value, vm);
            let (value, vm) = pending.as_parts_mut();
            if value.$method(other, vm)? {
                Ok(CallResult::Value(pending.into_inner()))
            } else {
                Ok(CallResult::Value(Value::NotImplemented))
            }
        }
    )*};
}
inplace! {
    iadd => py_iadd_impl, imul => py_imul_impl, isub => py_isub_impl,
    iand => py_iand_impl, ior => py_ior_impl, ixor => py_ixor_impl,
}

/// Runs repr through PyValue so the root container participates in cycle detection.
pub(crate) fn repr<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__repr__", vm.heap)?;
    this.py_repr(vm).map(CallResult::Value)
}

/// Preserves each type's string conversion, including `object.__str__`'s repr fallback.
/// Container fallback starts at PyValue to seed the cycle set before formatting children.
pub(crate) fn str<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__str__", vm.heap)?;
    match this {
        PyValue::List(_)
        | PyValue::Tuple(_)
        | PyValue::NamedTuple(_)
        | PyValue::Dict(_)
        | PyValue::Set(_)
        | PyValue::FrozenSet(_)
        | PyValue::Deque(_) => this.py_repr(vm),
        _ => this.py_str(vm),
    }
    .map(CallResult::Value)
}

/// Uses native truth testing, including Python 3.14's error for NotImplemented.
/// Register only types exposing `__bool__`; a length-based truth test alone is insufficient.
pub(crate) fn bool<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__bool__", vm.heap)?;
    this.py_bool(vm).map(|value| CallResult::Value(Value::Bool(value)))
}

/// Returns the native floating-point conversion without int/string constructor fallbacks.
pub(crate) fn float<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__float__", vm.heap)?;
    Ok(CallResult::Value(
        this.py_float_impl(vm)?.map_or(Value::NotImplemented, Value::Float),
    ))
}

/// Passes the optional precision value to the native hook without coercing it here.
/// Each numeric type retains its own validation and return type.
pub(crate) fn round<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let digits = args.get_zero_one_arg("__round__", vm.heap)?;
    defer_drop!(digits, vm);
    Ok(CallResult::Value(
        this.py_round_impl(digits.as_ref(), vm)?
            .unwrap_or(Value::NotImplemented),
    ))
}

/// Adapts pow/rpow with an optional modulus; an explicit None means no modulus.
/// Integer descriptors accept integer operands, while float reflected power first
/// converts its numeric base. Native implementations retain modulus validation.
macro_rules! power {
    ($name:ident, $method:ident) => {
        pub(crate) fn $name<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
            let (other, modulus) = args.get_one_two_args(concat!("__", stringify!($name), "__"), vm.heap)?;
            defer_drop!(other, vm);
            defer_drop!(modulus, vm);
            let lhs_type = this.py_type(vm);
            let rhs_type = other.py_type(vm);
            if matches!(lhs_type, Type::Int | Type::Bool) && !matches!(rhs_type, Type::Int | Type::Bool) {
                return Ok(CallResult::Value(Value::NotImplemented));
            }
            // None is accepted as the omitted third argument, including for float power.
            let modulus = modulus.as_ref().filter(|value| !matches!(value, Value::None));
            let mut result = this.$method(other, modulus, vm)?;
            if result.is_none()
                && stringify!($name) == "rpow"
                && matches!(lhs_type, Type::Int | Type::Bool | Type::Float)
                && matches!(rhs_type, Type::Int | Type::Bool | Type::Float)
            {
                let receiver = this.to_owned_value(vm);
                defer_drop!(receiver, vm);
                // Native reflected hooks may defer same-kind arithmetic to operator dispatch.
                // Supply that arithmetic for this explicit call, preserving float base conversion.
                let base = if lhs_type == Type::Float {
                    Value::Float(other.py_value(vm).py_float_impl(vm)?.expect("numeric conversion"))
                } else {
                    other.clone_with_heap(vm.heap)
                };
                defer_drop!(base, vm);
                result = base.py_value(vm).py_pow_impl(receiver, modulus, vm)?;
            }
            Ok(CallResult::Value(result.unwrap_or(Value::NotImplemented)))
        }
    };
}
power!(pow, py_pow_impl);
power!(rpow, py_rpow_impl);

/// Shares the builtin hash conversion and signed hash representation.
/// These are Monty's native hash values, which need not equal CPython's hashes.
pub(crate) fn hash<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__hash__", vm.heap)?;
    let value = this.to_owned_value(vm);
    BuiltinsFunctions::Hash.call(vm, ArgValues::One(value))
}

/// Uses the formatter shared by format(), f-strings, and str.format().
/// The special method requires its spec argument even though format(value) can omit it.
pub(crate) fn format<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let spec = args.get_one_arg("__format__", vm.heap)?;
    let value = this.to_owned_value(vm);
    BuiltinsFunctions::Format.call(vm, ArgValues::Two(value, spec))
}

/// Returns bytes unchanged or encodes a sandbox Path's POSIX spelling as UTF-8.
/// Path conversion reads no host filesystem or host locale state.
pub(crate) fn bytes<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__bytes__", vm.heap)?;
    let value = match this {
        PyValue::Path(path) => allocate_bytes(path.get(vm.heap).as_str().as_bytes().to_vec(), vm.heap),
        PyValue::Bytes(_) => this.to_owned_value(vm),
        _ => unreachable!("registered bytes conversion"),
    };
    Ok(CallResult::Value(value))
}

/// Obtains a native iterator while keeping the receiver alive across iterator creation.
/// The temporary owned value bridges helpers that still accept Value rather than PyValue.
pub(crate) fn iter<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__iter__", vm.heap)?;
    let value = this.to_owned_value(vm);
    defer_drop!(value, vm);
    value.py_iter(vm)
}

/// Preserves builtin next()'s exhaustion and suspension behavior.
/// An unread file may suspend for a host buffer load; directly calling py_next would miss that path.
pub(crate) fn next<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__next__", vm.heap)?;
    let value = this.to_owned_value(vm);
    builtin_next(vm, ArgValues::One(value))
}

/// Returns an iterator over the existing reversed() builtin's eager snapshot.
/// Iterator wrapping changes the return shape, not the snapshot's timing or mutation behavior.
pub(crate) fn reversed<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__reversed__", vm.heap)?;
    let value = this.to_owned_value(vm);
    let reversed = BuiltinsFunctions::Reversed
        .call(vm, ArgValues::One(value))?
        .require_synchronous(vm)?;
    reversed.into_py_iter(vm)
}

/// Transfers the key and replacement into native assignment, returning None on success.
/// Native assignment owns their cleanup, including when index conversion or mutation fails.
pub(crate) fn setitem<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let (key, item) = args.get_two_args("__setitem__", vm.heap)?;
    let value = this.to_owned_value(vm);
    defer_drop_mut!(value, vm);
    // py_setitem consumes both operands on success and error.
    value.py_setitem(key, item, vm)?;
    Ok(CallResult::Value(Value::None))
}

/// Deletes a list index/slice, a deque index, or a mapping key using native storage.
/// Missing dict keys raise KeyError; Counter's missing-key exception is registered separately.
pub(crate) fn delitem<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let key = args.get_one_arg("__delitem__", vm.heap)?;
    defer_drop!(key, vm);
    match this {
        PyValue::List(list) => list.delete_item(key, vm)?,
        PyValue::Deque(deque) => deque.delete_item(key, vm)?,
        PyValue::Dict(dict) => {
            let pair = dict.pop(key, vm)?.ok_or_else(|| ExcType::key_error(key, vm))?;
            pair.drop_with(vm);
        }
        _ => unreachable!("registered item deletion"),
    }
    Ok(CallResult::Value(Value::None))
}

/// Applies dict's representation to the original mapping, including native subtypes.
/// Using the live payload lets element `__repr__` callbacks change the entries still to be printed.
pub(crate) fn dict_repr<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__repr__", vm.heap)?;
    let PyValue::Dict(dict) = this else { unreachable!() };
    dict.plain_repr(vm).map(CallResult::Value)
}

/// Compares real mapping entries, bypassing Counter's multiset equality.
/// Zero counts remain entries when `dict.__eq__` or `dict.__ne__` is called on a Counter.
fn dict_equality<'h>(this: &mut PyValue<'h>, args: ArgValues, negate: bool, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let other = args.get_one_arg(if negate { "__ne__" } else { "__eq__" }, vm.heap)?;
    defer_drop!(other, vm);
    let PyValue::Dict(dict) = this else { unreachable!() };
    let result = match other.read_heap(vm) {
        Some(HeapReadOutput::Dict(other)) => Value::Bool(dict.eq_dict(&other, vm)? ^ negate),
        _ => Value::NotImplemented,
    };
    Ok(CallResult::Value(result))
}

/// Selects plain mapping equality for the defining dict descriptor.
pub(crate) fn dict_eq<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    dict_equality(this, args, false, vm)
}

/// Negates a handled mapping comparison while preserving NotImplemented for other types.
pub(crate) fn dict_ne<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    dict_equality(this, args, true, vm)
}

/// Keeps dict ordering unsupported even when the receiver is a Counter.
/// The inherited dict descriptor must not dispatch to Counter's ordering override.
pub(crate) fn dict_order<'h>(_: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let other = args.get_one_arg("comparison", vm.heap)?;
    other.drop_with(vm);
    Ok(CallResult::Value(Value::NotImplemented))
}

/// Builds a plain dict result for `dict.__or__`, regardless of native receiver subtype.
/// Counter and defaultdict register their own union implementations.
pub(crate) fn dict_or<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let other = args.get_one_arg("__or__", vm.heap)?;
    defer_drop!(other, vm);
    let PyValue::Dict(dict) = this else { unreachable!() };
    Ok(CallResult::Value(
        dict::dict_or_plain(dict, other, vm)?.unwrap_or(Value::NotImplemented),
    ))
}

/// Uses dict.update semantics and returns the same receiver.
/// Calling `dict.__ior__` on a Counter must replace counts rather than apply multiset union.
pub(crate) fn dict_ior<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let other = args.get_one_arg("__ior__", vm.heap)?;
    let name = EitherStr::Heap("update".to_owned());
    let Some(AttrDef::Method(update)) = Type::Dict.lookup_attr(&name, vm.interns) else {
        unreachable!()
    };
    let result = update(this, ArgValues::One(other), vm)?.require_synchronous(vm)?;
    result.drop_with(vm);
    Ok(CallResult::Value(this.to_owned_value(vm)))
}

/// Applies the inherited dict reflected-union descriptor with reversed mapping order.
/// The result is plain dict, including when Counter inherits this method.
pub(crate) fn dict_ror<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let other = args.get_one_arg("__ror__", vm.heap)?;
    defer_drop!(other, vm);
    let value = this.to_owned_value(vm);
    defer_drop!(value, vm);
    let result = match other.read_heap(vm) {
        Some(HeapReadOutput::Dict(left)) => dict::dict_or_plain(&left, value, vm)?,
        _ => None,
    };
    Ok(CallResult::Value(result.unwrap_or(Value::NotImplemented)))
}

/// Implements Counter's missing-key deletion rule: an absent key is a no-op.
/// Calling `dict.__delitem__` on that same Counter still raises KeyError.
pub(crate) fn counter_delitem<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let key = args.get_one_arg("__delitem__", vm.heap)?;
    defer_drop!(key, vm);
    let PyValue::Dict(dict) = this else { unreachable!() };
    dict.pop(key, vm)?.drop_with(vm);
    Ok(CallResult::Value(Value::None))
}

/// Returns the native complex receiver with a counted reference.
/// Decimal's conversion has its own generated registration and does not use this identity adapter.
pub(crate) fn complex<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__complex__", vm.heap)?;
    Ok(CallResult::Value(this.to_owned_value(vm)))
}

/// Adapts native length for types without a type-specific wrapper, such as range and dict_values.
/// A registered length must exist and fit the signed Python index range.
pub(crate) fn len<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__len__", vm.heap)?;
    let len = this.py_len(vm).expect("registered length");
    Ok(CallResult::Value(Value::Int(
        i64::try_from(len).map_err(|_| ExcType::overflow_c_ssize_t())?,
    )))
}

/// Uses range's native membership implementation without the operator's iterator fallback.
pub(crate) fn contains<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let item = args.get_one_arg("__contains__", vm.heap)?;
    defer_drop!(item, vm);
    Ok(CallResult::Value(
        this.py_contains_impl(item, vm)?
            .map_or(Value::NotImplemented, Value::Bool),
    ))
}

/// Preserves integer/slice range keys and integer/name regex group keys.
/// Other collection types register their own wrappers, including defaultdict's missing-key hook.
pub(crate) fn getitem<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let key = args.get_one_arg("__getitem__", vm.heap)?;
    defer_drop!(key, vm);
    this.py_getitem(key, vm).map(CallResult::Value)
}

/// Preserves one-sided comparison results rather than applying reflected comparison.
/// Equality can fall back to object identity; ordering distinguishes an unordered NaN
/// from an unsupported operand pair, returning False and NotImplemented respectively.
fn compare<'h>(this: &mut PyValue<'h>, args: ArgValues, op: CmpOperator, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let other = args.get_one_arg("comparison", vm.heap)?;
    defer_drop!(other, vm);
    let result = if matches!(op, CmpOperator::Eq | CmpOperator::NotEq) {
        if matches!(this.py_type(vm), Type::Int | Type::Bool) && !matches!(other.py_type(vm), Type::Int | Type::Bool) {
            None
        } else {
            this.py_eq_impl(other, vm)?
                .or_else(|| {
                    use crate::types::py_trait::PyObjectIdentity;
                    (this.py_identity() == other.py_identity()).then_some(true)
                })
                .map(|equal| if op == CmpOperator::Eq { equal } else { !equal })
        }
    } else {
        let value = this.to_owned_value(vm);
        defer_drop!(value, vm);
        // Set inclusion and Counter ordering cannot be represented by a total Ordering.
        if let Some(result) = value.py_cmp_op(other, op, vm)? {
            Some(result)
        } else {
            // Integer/float descriptors don't ask a Decimal on the right to compare.
            let lhs = value.py_type(vm);
            let rhs = other.py_type(vm);
            if (matches!(lhs, Type::Int | Type::Bool) && !matches!(rhs, Type::Int | Type::Bool))
                || (lhs == Type::Float && !matches!(rhs, Type::Int | Type::Float | Type::Bool))
            {
                None
            } else {
                match value.py_cmp(other, vm)? {
                    CmpOrder::Incomparable => None,
                    CmpOrder::Unordered => Some(false),
                    CmpOrder::Ordered(order) => Some(match op {
                        CmpOperator::Lt => order.is_lt(),
                        CmpOperator::LtE => order.is_le(),
                        CmpOperator::Gt => order.is_gt(),
                        CmpOperator::GtE => order.is_ge(),
                        _ => unreachable!(),
                    }),
                }
            }
        }
    };
    Ok(CallResult::Value(result.map_or(Value::NotImplemented, Value::Bool)))
}
/// Binds Python's six comparison names to the shared one-sided comparison adapter.
macro_rules! comparison {
    ($($name:ident => $op:ident),* $(,)?) => {$ (
        pub(crate) fn $name<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
            compare(this, args, CmpOperator::$op, vm)
        }
    )*};
}
comparison! { eq => Eq, ne => NotEq, lt => Lt, le => LtE, gt => Gt, ge => GtE }
