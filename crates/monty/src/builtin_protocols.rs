//! Explicit calls to native protocols, independent of operator fallback dispatch.

use crate::{
    args::ArgValues,
    builtins::{BuiltinsFunctions, BuiltinsFunctionsExt, next::builtin_next},
    bytecode::{CallResult, VM},
    defer_drop, defer_drop_mut,
    exception_private::{ExcType, ExcTypeExt, RunResult},
    expressions::CmpOperator,
    heap::{DropGuard, DropWithContext, HeapReadOutput},
    intern::StaticStrings,
    py_attrs::AttrDef,
    py_value::PyValue,
    types::{CmpOrder, PyTrait, Type, bytes::allocate_bytes, dict},
    value::{EitherStr, Value},
};

macro_rules! unary {
    ($($name:ident => $method:ident),* $(,)?) => {$ (
        fn $name<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
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

type BinaryOperation<'h> = fn(&PyValue<'h>, &Value, &mut VM<'h>) -> RunResult<Option<Value>>;

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
    if result.is_none() && name == "ror" && matches!(lhs_type, Type::Dict | Type::DefaultDict | Type::Counter) {
        let value = this.to_owned_value(vm);
        defer_drop!(value, vm);
        if let Some(HeapReadOutput::Dict(left)) = other.read_heap(vm) {
            result = dict::dict_or(&left, value, vm)?;
        }
    }
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

macro_rules! binary {
    ($($name:ident => $method:ident),* $(,)?) => {$ (
        fn $name<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
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

macro_rules! inplace {
    ($($name:ident => $method:ident),* $(,)?) => {$ (
        fn $name<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
            let other = args.get_one_arg(concat!("__", stringify!($name), "__"), vm.heap)?;
            defer_drop!(other, vm);
            let value = this.to_owned_value(vm);
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

fn repr<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__repr__", vm.heap)?;
    this.py_repr(vm).map(CallResult::Value)
}

fn str<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
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

fn bool<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__bool__", vm.heap)?;
    this.py_bool(vm).map(|value| CallResult::Value(Value::Bool(value)))
}

fn float<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__float__", vm.heap)?;
    Ok(CallResult::Value(
        this.py_float_impl(vm)?.map_or(Value::NotImplemented, Value::Float),
    ))
}

fn round<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let digits = args.get_zero_one_arg("__round__", vm.heap)?;
    defer_drop!(digits, vm);
    Ok(CallResult::Value(
        this.py_round_impl(digits.as_ref(), vm)?
            .unwrap_or(Value::NotImplemented),
    ))
}

macro_rules! power {
    ($name:ident, $method:ident) => {
        fn $name<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
            let (other, modulus) = args.get_one_two_args(concat!("__", stringify!($name), "__"), vm.heap)?;
            defer_drop!(other, vm);
            defer_drop!(modulus, vm);
            let lhs_type = this.py_type(vm);
            let rhs_type = other.py_type(vm);
            if matches!(lhs_type, Type::Int | Type::Bool) && !matches!(rhs_type, Type::Int | Type::Bool) {
                return Ok(CallResult::Value(Value::NotImplemented));
            }
            let modulus = modulus.as_ref().filter(|value| !matches!(value, Value::None));
            let mut result = this.$method(other, modulus, vm)?;
            if result.is_none()
                && stringify!($name) == "rpow"
                && matches!(lhs_type, Type::Int | Type::Bool | Type::Float)
                && matches!(rhs_type, Type::Int | Type::Bool | Type::Float)
            {
                let receiver = this.to_owned_value(vm);
                defer_drop!(receiver, vm);
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

fn hash<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__hash__", vm.heap)?;
    let value = this.to_owned_value(vm);
    BuiltinsFunctions::Hash.call(vm, ArgValues::One(value))
}

fn format<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let spec = args.get_one_arg("__format__", vm.heap)?;
    let value = this.to_owned_value(vm);
    BuiltinsFunctions::Format.call(vm, ArgValues::Two(value, spec))
}

fn bytes<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__bytes__", vm.heap)?;
    let value = match this {
        PyValue::Path(path) => allocate_bytes(path.get(vm.heap).as_str().as_bytes().to_vec(), vm.heap),
        PyValue::Bytes(_) => this.to_owned_value(vm),
        _ => unreachable!("registered bytes conversion"),
    };
    Ok(CallResult::Value(value))
}

fn iter<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__iter__", vm.heap)?;
    let value = this.to_owned_value(vm);
    defer_drop!(value, vm);
    value.py_iter(vm)
}

fn next<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__next__", vm.heap)?;
    let value = this.to_owned_value(vm);
    builtin_next(vm, ArgValues::One(value))
}

fn reversed<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__reversed__", vm.heap)?;
    let value = this.to_owned_value(vm);
    let reversed = BuiltinsFunctions::Reversed
        .call(vm, ArgValues::One(value))?
        .require_synchronous(vm)?;
    reversed.into_py_iter(vm)
}

fn setitem<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let (key, item) = args.get_two_args("__setitem__", vm.heap)?;
    let value = this.to_owned_value(vm);
    defer_drop_mut!(value, vm);
    // py_setitem consumes both operands on success and error.
    value.py_setitem(key, item, vm)?;
    Ok(CallResult::Value(Value::None))
}

fn delitem<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
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

fn dict_repr<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__repr__", vm.heap)?;
    let PyValue::Dict(dict) = this else { unreachable!() };
    dict.plain_repr(vm).map(CallResult::Value)
}

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

fn dict_eq<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    dict_equality(this, args, false, vm)
}

fn dict_ne<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    dict_equality(this, args, true, vm)
}

fn dict_order<'h>(_: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let other = args.get_one_arg("comparison", vm.heap)?;
    other.drop_with(vm);
    Ok(CallResult::Value(Value::NotImplemented))
}

fn dict_or<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let other = args.get_one_arg("__or__", vm.heap)?;
    defer_drop!(other, vm);
    let PyValue::Dict(dict) = this else { unreachable!() };
    Ok(CallResult::Value(
        dict::dict_or_plain(dict, other, vm)?.unwrap_or(Value::NotImplemented),
    ))
}

fn dict_ior<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let other = args.get_one_arg("__ior__", vm.heap)?;
    let name = EitherStr::Heap("update".to_owned());
    let Some(AttrDef::Method(update)) = Type::Dict.lookup_attr(&name, vm.interns) else {
        unreachable!()
    };
    let result = update(this, ArgValues::One(other), vm)?.require_synchronous(vm)?;
    result.drop_with(vm);
    Ok(CallResult::Value(this.to_owned_value(vm)))
}

fn dict_ror<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
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

fn counter_delitem<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let key = args.get_one_arg("__delitem__", vm.heap)?;
    defer_drop!(key, vm);
    let PyValue::Dict(dict) = this else { unreachable!() };
    dict.pop(key, vm)?.drop_with(vm);
    Ok(CallResult::Value(Value::None))
}

fn complex<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__complex__", vm.heap)?;
    Ok(CallResult::Value(this.to_owned_value(vm)))
}

fn len<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    args.check_zero_args("__len__", vm.heap)?;
    let len = this.py_len(vm).expect("registered length");
    Ok(CallResult::Value(Value::Int(
        i64::try_from(len).map_err(|_| ExcType::overflow_c_ssize_t())?,
    )))
}

fn contains<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let item = args.get_one_arg("__contains__", vm.heap)?;
    defer_drop!(item, vm);
    Ok(CallResult::Value(
        this.py_contains_impl(item, vm)?
            .map_or(Value::NotImplemented, Value::Bool),
    ))
}

fn getitem<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let key = args.get_one_arg("__getitem__", vm.heap)?;
    defer_drop!(key, vm);
    this.py_getitem(key, vm).map(CallResult::Value)
}

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
macro_rules! comparison {
    ($($name:ident => $op:ident),* $(,)?) => {$ (
        fn $name<'h>(this: &mut PyValue<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
            compare(this, args, CmpOperator::$op, vm)
        }
    )*};
}
comparison! { eq => Eq, ne => NotEq, lt => Lt, le => LtE, gt => Gt, ge => GtE }

/// Joins native methods and protocol entries at compile time, without allocating during lookup.
pub(crate) const fn join<const N: usize>(
    a: &[(StaticStrings, AttrDef)],
    b: &[(StaticStrings, AttrDef)],
) -> [(StaticStrings, AttrDef); N] {
    let mut result = [(StaticStrings::DunderRepr, AttrDef::Method(repr)); N];
    let mut i = 0;
    while i < a.len() {
        result[i] = a[i];
        i += 1;
    }
    let mut j = 0;
    while j < b.len() {
        result[i + j] = b[j];
        j += 1;
    }
    result
}

pub(crate) fn lookup(owner: Type, name: StaticStrings) -> Option<AttrDef> {
    attrs(owner)
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .map(|(_, definition)| *definition)
}

macro_rules! entries {
    (@entry abs) => { (StaticStrings::DunderAbs, AttrDef::Method(abs)) };
    (@entry add) => { (StaticStrings::DunderAdd, AttrDef::Method(add)) };
    (@entry and) => { (StaticStrings::DunderAnd, AttrDef::Method(and)) };
    (@entry bool) => { (StaticStrings::DunderBool, AttrDef::Method(bool)) };
    (@entry bytes) => { (StaticStrings::DunderBytes, AttrDef::Method(bytes)) };
    (@entry ceil) => { (StaticStrings::DunderCeil, AttrDef::Method(ceil)) };
    (@entry complex) => { (StaticStrings::DunderComplex, AttrDef::Method(complex)) };
    (@entry contains) => { (StaticStrings::DunderContains, AttrDef::Method(contains)) };
    (@entry delitem) => { (StaticStrings::DunderDelitem, AttrDef::Method(delitem)) };
    (@entry divmod) => { (StaticStrings::DunderDivmod, AttrDef::Method(divmod)) };
    (@entry eq) => { (StaticStrings::DunderEq, AttrDef::Method(eq)) };
    (@entry float) => { (StaticStrings::DunderFloat, AttrDef::Method(float)) };
    (@entry floor) => { (StaticStrings::DunderFloor, AttrDef::Method(floor)) };
    (@entry floordiv) => { (StaticStrings::DunderFloordiv, AttrDef::Method(floordiv)) };
    (@entry format) => { (StaticStrings::DunderFormat, AttrDef::Method(format)) };
    (@entry ge) => { (StaticStrings::DunderGe, AttrDef::Method(ge)) };
    (@entry getitem) => { (StaticStrings::DunderGetitem, AttrDef::Method(getitem)) };
    (@entry gt) => { (StaticStrings::DunderGt, AttrDef::Method(gt)) };
    (@entry hash) => { (StaticStrings::DunderHash, AttrDef::Method(hash)) };
    (@entry iadd) => { (StaticStrings::DunderIadd, AttrDef::Method(iadd)) };
    (@entry iand) => { (StaticStrings::DunderIand, AttrDef::Method(iand)) };
    (@entry imul) => { (StaticStrings::DunderImul, AttrDef::Method(imul)) };
    (@entry index) => { (StaticStrings::DunderIndex, AttrDef::Method(index)) };
    (@entry int) => { (StaticStrings::DunderInt, AttrDef::Method(int)) };
    (@entry invert) => { (StaticStrings::DunderInvert, AttrDef::Method(invert)) };
    (@entry ior) => { (StaticStrings::DunderIor, AttrDef::Method(ior)) };
    (@entry isub) => { (StaticStrings::DunderIsub, AttrDef::Method(isub)) };
    (@entry iter) => { (StaticStrings::DunderIter, AttrDef::Method(iter)) };
    (@entry ixor) => { (StaticStrings::DunderIxor, AttrDef::Method(ixor)) };
    (@entry le) => { (StaticStrings::DunderLe, AttrDef::Method(le)) };
    (@entry len) => { (StaticStrings::DunderLen, AttrDef::Method(len)) };
    (@entry lshift) => { (StaticStrings::DunderLshift, AttrDef::Method(lshift)) };
    (@entry lt) => { (StaticStrings::DunderLt, AttrDef::Method(lt)) };
    (@entry mod) => { (StaticStrings::DunderMod, AttrDef::Method(modulo)) };
    (@entry mul) => { (StaticStrings::DunderMul, AttrDef::Method(mul)) };
    (@entry ne) => { (StaticStrings::DunderNe, AttrDef::Method(ne)) };
    (@entry neg) => { (StaticStrings::DunderNeg, AttrDef::Method(neg)) };
    (@entry next) => { (StaticStrings::DunderNext, AttrDef::Method(next)) };
    (@entry or) => { (StaticStrings::DunderOr, AttrDef::Method(or)) };
    (@entry pos) => { (StaticStrings::DunderPos, AttrDef::Method(pos)) };
    (@entry pow) => { (StaticStrings::DunderPow, AttrDef::Method(pow)) };
    (@entry radd) => { (StaticStrings::DunderRadd, AttrDef::Method(radd)) };
    (@entry rand) => { (StaticStrings::DunderRand, AttrDef::Method(rand)) };
    (@entry rdivmod) => { (StaticStrings::DunderRdivmod, AttrDef::Method(rdivmod)) };
    (@entry repr) => { (StaticStrings::DunderRepr, AttrDef::Method(repr)) };
    (@entry reversed) => { (StaticStrings::DunderReversed, AttrDef::Method(reversed)) };
    (@entry rfloordiv) => { (StaticStrings::DunderRfloordiv, AttrDef::Method(rfloordiv)) };
    (@entry rlshift) => { (StaticStrings::DunderRlshift, AttrDef::Method(rlshift)) };
    (@entry rmod) => { (StaticStrings::DunderRmod, AttrDef::Method(rmod)) };
    (@entry rmul) => { (StaticStrings::DunderRmul, AttrDef::Method(rmul)) };
    (@entry ror) => { (StaticStrings::DunderRor, AttrDef::Method(ror)) };
    (@entry round) => { (StaticStrings::DunderRound, AttrDef::Method(round)) };
    (@entry rpow) => { (StaticStrings::DunderRpow, AttrDef::Method(rpow)) };
    (@entry rrshift) => { (StaticStrings::DunderRrshift, AttrDef::Method(rrshift)) };
    (@entry rshift) => { (StaticStrings::DunderRshift, AttrDef::Method(rshift)) };
    (@entry rsub) => { (StaticStrings::DunderRsub, AttrDef::Method(rsub)) };
    (@entry rtruediv) => { (StaticStrings::DunderRtruediv, AttrDef::Method(rtruediv)) };
    (@entry rxor) => { (StaticStrings::DunderRxor, AttrDef::Method(rxor)) };
    (@entry setitem) => { (StaticStrings::DunderSetitem, AttrDef::Method(setitem)) };
    (@entry str) => { (StaticStrings::DunderStr, AttrDef::Method(str)) };
    (@entry sub) => { (StaticStrings::DunderSub, AttrDef::Method(sub)) };
    (@entry truediv) => { (StaticStrings::DunderTruediv, AttrDef::Method(truediv)) };
    (@entry trunc) => { (StaticStrings::DunderTrunc, AttrDef::Method(trunc)) };
    (@entry xor) => { (StaticStrings::DunderXor, AttrDef::Method(xor)) };
    (@entry dict_repr) => { (StaticStrings::DunderRepr, AttrDef::Method(dict_repr)) };
    (@entry dict_eq) => { (StaticStrings::DunderEq, AttrDef::Method(dict_eq)) };
    (@entry dict_ne) => { (StaticStrings::DunderNe, AttrDef::Method(dict_ne)) };
    (@entry dict_lt) => { (StaticStrings::DunderLt, AttrDef::Method(dict_order)) };
    (@entry dict_le) => { (StaticStrings::DunderLe, AttrDef::Method(dict_order)) };
    (@entry dict_gt) => { (StaticStrings::DunderGt, AttrDef::Method(dict_order)) };
    (@entry dict_ge) => { (StaticStrings::DunderGe, AttrDef::Method(dict_order)) };
    (@entry dict_or) => { (StaticStrings::DunderOr, AttrDef::Method(dict_or)) };
    (@entry dict_ior) => { (StaticStrings::DunderIor, AttrDef::Method(dict_ior)) };
    (@entry dict_ror) => { (StaticStrings::DunderRor, AttrDef::Method(dict_ror)) };
    (@entry counter_delitem) => { (StaticStrings::DunderDelitem, AttrDef::Method(counter_delitem)) };
    ($($name:ident),* $(,)?) => { &[$(entries!(@entry $name)),*] };
}

pub(crate) const fn attrs(owner: Type) -> &'static [(StaticStrings, AttrDef)] {
    match owner {
        Type::Int => entries!(
            repr, str, bool, float, round, hash, format, neg, pos, abs, invert, index, int, floor, ceil, trunc, add,
            radd, sub, rsub, mul, rmul, truediv, rtruediv, floordiv, rfloordiv, mod, rmod, divmod, rdivmod, and, rand,
            or, ror, xor, rxor, lshift, rlshift, rshift, rrshift, pow, rpow, eq, ne, lt, le, gt, ge
        ),
        Type::Bool => entries!(
            repr, str, bool, float, round, hash, format, neg, pos, abs, invert, index, int, floor, ceil, trunc, add,
            radd, sub, rsub, mul, rmul, truediv, rtruediv, floordiv, rfloordiv, mod, rmod, divmod, rdivmod, and, rand,
            or, ror, xor, rxor, lshift, rlshift, rshift, rrshift, pow, rpow, eq, ne, lt, le, gt, ge
        ),
        Type::Float => entries!(
            repr, str, bool, float, round, hash, format, neg, pos, abs, int, floor, ceil, trunc, add, radd, sub, rsub,
            mul, rmul, truediv, rtruediv, floordiv, rfloordiv, mod, rmod, divmod, rdivmod, pow, rpow, eq, ne, lt, le,
            gt, ge
        ),
        Type::Complex => entries!(
            repr, str, bool, complex, hash, format, neg, pos, abs, add, radd, sub, rsub, mul, rmul, truediv, rtruediv,
            pow, rpow, eq, ne, lt, le, gt, ge
        ),
        Type::Str => entries!(
            repr, str, hash, format, iter, add, mul, rmul, mod, rmod, eq, ne, lt, le, gt, ge
        ),
        Type::Bytes => entries!(
            repr, str, hash, format, bytes, iter, add, mul, rmul, mod, rmod, eq, ne, lt, le, gt, ge
        ),
        Type::List => entries!(
            repr, str, format, iter, reversed, setitem, delitem, imul, add, mul, rmul, iadd, eq, ne, lt, le, gt, ge
        ),
        Type::Tuple => entries!(repr, str, hash, format, iter, add, mul, rmul, eq, ne, lt, le, gt, ge),
        Type::NamedTuple => entries!(repr, str, hash, format, iter, add, mul, rmul, eq, ne, lt, le, gt, ge),
        Type::Dict => entries!(
            dict_repr, str, format, iter, reversed, setitem, delitem, dict_or, dict_ror, dict_ior, dict_eq, dict_ne,
            dict_lt, dict_le, dict_gt, dict_ge
        ),
        Type::DefaultDict => entries!(
            repr, str, format, iter, reversed, setitem, delitem, or, ror, ior, eq, ne, lt, le, gt, ge
        ),
        Type::Counter => entries!(
            repr,
            str,
            format,
            iter,
            reversed,
            setitem,
            counter_delitem,
            neg,
            pos,
            add,
            sub,
            and,
            or,
            dict_ror,
            iadd,
            isub,
            iand,
            ior,
            eq,
            ne,
            lt,
            le,
            gt,
            ge
        ),
        Type::Set => entries!(
            repr, str, format, iter, ixor, sub, rsub, and, rand, or, ror, xor, rxor, isub, iand, ior, eq, ne, lt, le,
            gt, ge
        ),
        Type::FrozenSet => entries!(
            repr, str, hash, format, iter, sub, rsub, and, rand, or, ror, xor, rxor, eq, ne, lt, le, gt, ge
        ),
        Type::Deque => entries!(
            repr, str, format, iter, reversed, setitem, delitem, imul, add, mul, rmul, iadd, eq, ne, lt, le, gt, ge
        ),
        Type::DictKeys => entries!(
            repr, str, format, iter, reversed, sub, rsub, and, rand, or, ror, xor, rxor, eq, ne, lt, le, gt, ge
        ),
        Type::DictItems => entries!(
            repr, str, format, iter, reversed, sub, rsub, and, rand, or, ror, xor, rxor, eq, ne, lt, le, gt, ge
        ),
        Type::DictValues => entries!(repr, str, hash, format, iter, reversed, len, eq, ne, lt, le, gt, ge),
        Type::Range => entries!(
            repr, str, bool, hash, format, iter, reversed, len, contains, getitem, eq, ne, lt, le, gt, ge
        ),
        Type::Date => entries!(repr, str, hash, format, add, radd, sub, rsub, eq, ne, lt, le, gt, ge),
        Type::DateTime => entries!(repr, str, hash, format, add, radd, sub, rsub, eq, ne, lt, le, gt, ge),
        Type::Time => entries!(repr, str, hash, format, eq, ne, lt, le, gt, ge),
        Type::TimeDelta => entries!(
            repr, str, bool, hash, format, neg, pos, abs, add, radd, sub, rsub, mul, rmul, truediv, rtruediv, floordiv,
            rfloordiv, mod, rmod, divmod, rdivmod, eq, ne, lt, le, gt, ge
        ),
        Type::TimeZone => entries!(repr, str, hash, format, eq, ne, lt, le, gt, ge),
        Type::Decimal => entries!(
            repr, str, bool, hash, neg, pos, abs, add, radd, sub, rsub, mul, rmul, truediv, rtruediv, floordiv,
            rfloordiv, mod, rmod, divmod, rdivmod, pow, rpow, eq, ne, lt, le, gt, ge
        ),
        Type::DecimalContext => entries!(repr, str, hash, format, eq, ne, lt, le, gt, ge),
        Type::Path => entries!(
            repr, str, hash, format, bytes, truediv, rtruediv, eq, ne, lt, le, gt, ge
        ),
        Type::Random => entries!(repr, str, hash, format, eq, ne, lt, le, gt, ge),
        Type::RePattern => entries!(repr, str, hash, format, eq, ne, lt, le, gt, ge),
        Type::ReMatch => entries!(repr, str, hash, format, getitem, eq, ne, lt, le, gt, ge),
        Type::Iterator => entries!(repr, str, hash, format, iter, next, eq, ne, lt, le, gt, ge),
        Type::ListIterator => entries!(repr, str, hash, format, iter, next, eq, ne, lt, le, gt, ge),
        Type::CallableIterator => entries!(repr, str, hash, format, iter, next, eq, ne, lt, le, gt, ge),
        Type::NoneType => entries!(repr, str, bool, hash, format, eq, ne, lt, le, gt, ge),
        Type::Ellipsis => entries!(repr, str, hash, format, eq, ne, lt, le, gt, ge),
        Type::NotImplementedType => entries!(repr, str, bool, hash, format, eq, ne, lt, le, gt, ge),
        Type::TextIOWrapper => entries!(repr, str, hash, format, iter, next, eq, ne, lt, le, gt, ge),
        Type::BufferedReader => entries!(repr, str, hash, format, iter, next, eq, ne, lt, le, gt, ge),
        Type::BufferedWriter => entries!(repr, str, hash, format, iter, next, eq, ne, lt, le, gt, ge),
        Type::BufferedRandom => entries!(repr, str, hash, format, iter, next, eq, ne, lt, le, gt, ge),
        Type::HashlibHash => entries!(repr, str, hash, format, eq, ne, lt, le, gt, ge),
        Type::HashlibHashXof => entries!(repr, str, hash, format, eq, ne, lt, le, gt, ge),
        Type::Blake2b => entries!(repr, str, hash, format, eq, ne, lt, le, gt, ge),
        Type::Blake2s => entries!(repr, str, hash, format, eq, ne, lt, le, gt, ge),
        Type::LruCacheWrapper => entries!(repr, str, hash, format, eq, ne, lt, le, gt, ge),
        _ if owner.is_iterator() => attrs(Type::Iterator),
        _ => &[],
    }
}
