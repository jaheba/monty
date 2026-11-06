//! Rounding helpers shared by immediate and heap-allocated numbers.

use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::Pow;

use crate::{
    builtin_protocols,
    bytecode::VM,
    exception_private::RunResult,
    heap::Heap,
    py_attrs::{MethodCall, PyAttrs},
    py_value::PyValue,
    types::LongInt,
    value::Value,
};

/// Resolves numeric rounding precision through `__index__`, clamping huge integers.
/// User `__round__` methods receive the original argument instead of using this helper.
pub(crate) fn round_digits(ndigits: Option<&Value>, vm: &mut VM<'_>) -> RunResult<Option<i64>> {
    match ndigits {
        None | Some(Value::None) => Ok(None),
        Some(value) => value.saturating_index(vm).map(Some),
    }
}

/// Rounds a small integer without losing precision, promoting an overflowing result.
pub(crate) fn round_int(n: i64, digits: Option<i64>, heap: &Heap) -> Value {
    if let Some(d) = digits.filter(|d| *d < 0) {
        // Any divisor >= 10**20 rounds an i64 to zero; smaller intermediates fit i128.
        let result = match u32::try_from(d.unsigned_abs()) {
            Ok(exp @ ..=19) => {
                let factor = 10_i128.pow(exp);
                let n = i128::from(n);
                let mut q = n / factor;
                let r2 = (n % factor).abs() * 2;
                if r2 > factor || (r2 == factor && q % 2 != 0) {
                    q += if n < 0 { -1 } else { 1 };
                }
                q * factor
            }
            _ => 0,
        };
        match i64::try_from(result) {
            Ok(i) => Value::Int(i),
            Err(_) => LongInt::new(BigInt::from(result)).into_value(heap),
        }
    } else {
        Value::Int(n)
    }
}

/// Rounds a wide integer to a multiple of `10**k`, with ties to even.
pub(crate) fn round_bigint(n: &BigInt, k: u64, heap: &Heap) -> Value {
    let digit_bound = n.bits().saturating_mul(30_103) / 100_000 + 1;
    if k > digit_bound {
        return Value::Int(0);
    }
    let factor = Pow::pow(BigInt::from(10u8), k);
    let (quotient, remainder) = n.div_mod_floor(&factor);
    let twice_remainder = remainder << 1u32;
    let quotient = if twice_remainder > factor || (twice_remainder == factor && quotient.is_odd()) {
        quotient + 1
    } else {
        quotient
    };
    LongInt::new(quotient * factor).into_value(heap)
}

/// Rounds a float, returning an integer only when precision is omitted or `None`.
pub(crate) fn round_float(value: f64, digits: Option<i64>, heap: &Heap) -> RunResult<Value> {
    if let Some(digits) = digits {
        Ok(Value::Float(round_float_to_digits(value, digits)))
    } else {
        LongInt::value_from_f64(bankers_round(value), heap)
    }
}

/// Rounds to the nearest integer, breaking ties toward an even integer.
fn bankers_round(value: f64) -> f64 {
    let floor = value.floor();
    let frac = value - floor;
    if frac < 0.5 {
        floor
    } else if frac > 0.5 {
        floor + 1.0
    } else if floor % 2.0 == 0.0 {
        floor
    } else {
        floor + 1.0
    }
}

/// Rounds decimal places, saturating extreme precisions and preserving signed zero.
fn round_float_to_digits(value: f64, digits: i64) -> f64 {
    if !value.is_finite() {
        return value;
    }
    let rounded = if digits >= 0 {
        let Ok(exp) = i32::try_from(digits) else {
            return value;
        };
        let multiplier = 10_f64.powi(exp);
        if !multiplier.is_finite() {
            return value;
        }
        let scaled = value * multiplier;
        if !scaled.is_finite() {
            return value;
        }
        bankers_round(scaled) / multiplier
    } else {
        let Ok(exp) = i32::try_from(digits) else {
            return 0.0_f64.copysign(value);
        };
        let multiplier = 10_f64.powi(exp);
        if multiplier == 0.0 {
            return 0.0_f64.copysign(value);
        }
        bankers_round(value * multiplier) / multiplier
    };
    if rounded == 0.0 {
        0.0_f64.copysign(value)
    } else {
        rounded
    }
}

/// Python-visible methods for the native Int value.
pub(crate) struct PyIntAttrs;

#[monty_macros::pyattrs]
impl<'h> PyAttrs<'h> for PyIntAttrs {
    type Receiver = PyValue<'h>;

    #[pymethod]
    const __repr__: MethodCall = builtin_protocols::repr;

    #[pymethod]
    const __str__: MethodCall = builtin_protocols::str;

    #[pymethod]
    const __bool__: MethodCall = builtin_protocols::bool;

    #[pymethod]
    const __float__: MethodCall = builtin_protocols::float;

    #[pymethod]
    const __round__: MethodCall = builtin_protocols::round;

    #[pymethod]
    const __hash__: MethodCall = builtin_protocols::hash;

    #[pymethod]
    const __format__: MethodCall = builtin_protocols::format;

    #[pymethod]
    const __neg__: MethodCall = builtin_protocols::neg;

    #[pymethod]
    const __pos__: MethodCall = builtin_protocols::pos;

    #[pymethod]
    const __abs__: MethodCall = builtin_protocols::abs;

    #[pymethod]
    const __invert__: MethodCall = builtin_protocols::invert;

    #[pymethod]
    const __index__: MethodCall = builtin_protocols::index;

    #[pymethod]
    const __int__: MethodCall = builtin_protocols::int;

    #[pymethod]
    const __floor__: MethodCall = builtin_protocols::floor;

    #[pymethod]
    const __ceil__: MethodCall = builtin_protocols::ceil;

    #[pymethod]
    const __trunc__: MethodCall = builtin_protocols::trunc;

    #[pymethod]
    const __add__: MethodCall = builtin_protocols::add;

    #[pymethod]
    const __radd__: MethodCall = builtin_protocols::radd;

    #[pymethod]
    const __sub__: MethodCall = builtin_protocols::sub;

    #[pymethod]
    const __rsub__: MethodCall = builtin_protocols::rsub;

    #[pymethod]
    const __mul__: MethodCall = builtin_protocols::mul;

    #[pymethod]
    const __rmul__: MethodCall = builtin_protocols::rmul;

    #[pymethod]
    const __truediv__: MethodCall = builtin_protocols::truediv;

    #[pymethod]
    const __rtruediv__: MethodCall = builtin_protocols::rtruediv;

    #[pymethod]
    const __floordiv__: MethodCall = builtin_protocols::floordiv;

    #[pymethod]
    const __rfloordiv__: MethodCall = builtin_protocols::rfloordiv;

    #[pymethod]
    const __mod__: MethodCall = builtin_protocols::modulo;

    #[pymethod]
    const __rmod__: MethodCall = builtin_protocols::rmod;

    #[pymethod]
    const __divmod__: MethodCall = builtin_protocols::divmod;

    #[pymethod]
    const __rdivmod__: MethodCall = builtin_protocols::rdivmod;

    #[pymethod]
    const __and__: MethodCall = builtin_protocols::and;

    #[pymethod]
    const __rand__: MethodCall = builtin_protocols::rand;

    #[pymethod]
    const __or__: MethodCall = builtin_protocols::or;

    #[pymethod]
    const __ror__: MethodCall = builtin_protocols::ror;

    #[pymethod]
    const __xor__: MethodCall = builtin_protocols::xor;

    #[pymethod]
    const __rxor__: MethodCall = builtin_protocols::rxor;

    #[pymethod]
    const __lshift__: MethodCall = builtin_protocols::lshift;

    #[pymethod]
    const __rlshift__: MethodCall = builtin_protocols::rlshift;

    #[pymethod]
    const __rshift__: MethodCall = builtin_protocols::rshift;

    #[pymethod]
    const __rrshift__: MethodCall = builtin_protocols::rrshift;

    #[pymethod]
    const __pow__: MethodCall = builtin_protocols::pow;

    #[pymethod]
    const __rpow__: MethodCall = builtin_protocols::rpow;

    #[pymethod]
    const __eq__: MethodCall = builtin_protocols::eq;

    #[pymethod]
    const __ne__: MethodCall = builtin_protocols::ne;

    #[pymethod]
    const __lt__: MethodCall = builtin_protocols::lt;

    #[pymethod]
    const __le__: MethodCall = builtin_protocols::le;

    #[pymethod]
    const __gt__: MethodCall = builtin_protocols::gt;

    #[pymethod]
    const __ge__: MethodCall = builtin_protocols::ge;
}

/// Python-visible methods for the native Bool value.
pub(crate) struct PyBoolAttrs;

#[monty_macros::pyattrs]
impl<'h> PyAttrs<'h> for PyBoolAttrs {
    type Receiver = PyValue<'h>;

    #[pymethod]
    const __repr__: MethodCall = builtin_protocols::repr;

    #[pymethod]
    const __str__: MethodCall = builtin_protocols::str;

    #[pymethod]
    const __bool__: MethodCall = builtin_protocols::bool;

    #[pymethod]
    const __float__: MethodCall = builtin_protocols::float;

    #[pymethod]
    const __round__: MethodCall = builtin_protocols::round;

    #[pymethod]
    const __hash__: MethodCall = builtin_protocols::hash;

    #[pymethod]
    const __format__: MethodCall = builtin_protocols::format;

    #[pymethod]
    const __neg__: MethodCall = builtin_protocols::neg;

    #[pymethod]
    const __pos__: MethodCall = builtin_protocols::pos;

    #[pymethod]
    const __abs__: MethodCall = builtin_protocols::abs;

    #[pymethod]
    const __invert__: MethodCall = builtin_protocols::invert;

    #[pymethod]
    const __index__: MethodCall = builtin_protocols::index;

    #[pymethod]
    const __int__: MethodCall = builtin_protocols::int;

    #[pymethod]
    const __floor__: MethodCall = builtin_protocols::floor;

    #[pymethod]
    const __ceil__: MethodCall = builtin_protocols::ceil;

    #[pymethod]
    const __trunc__: MethodCall = builtin_protocols::trunc;

    #[pymethod]
    const __add__: MethodCall = builtin_protocols::add;

    #[pymethod]
    const __radd__: MethodCall = builtin_protocols::radd;

    #[pymethod]
    const __sub__: MethodCall = builtin_protocols::sub;

    #[pymethod]
    const __rsub__: MethodCall = builtin_protocols::rsub;

    #[pymethod]
    const __mul__: MethodCall = builtin_protocols::mul;

    #[pymethod]
    const __rmul__: MethodCall = builtin_protocols::rmul;

    #[pymethod]
    const __truediv__: MethodCall = builtin_protocols::truediv;

    #[pymethod]
    const __rtruediv__: MethodCall = builtin_protocols::rtruediv;

    #[pymethod]
    const __floordiv__: MethodCall = builtin_protocols::floordiv;

    #[pymethod]
    const __rfloordiv__: MethodCall = builtin_protocols::rfloordiv;

    #[pymethod]
    const __mod__: MethodCall = builtin_protocols::modulo;

    #[pymethod]
    const __rmod__: MethodCall = builtin_protocols::rmod;

    #[pymethod]
    const __divmod__: MethodCall = builtin_protocols::divmod;

    #[pymethod]
    const __rdivmod__: MethodCall = builtin_protocols::rdivmod;

    #[pymethod]
    const __and__: MethodCall = builtin_protocols::and;

    #[pymethod]
    const __rand__: MethodCall = builtin_protocols::rand;

    #[pymethod]
    const __or__: MethodCall = builtin_protocols::or;

    #[pymethod]
    const __ror__: MethodCall = builtin_protocols::ror;

    #[pymethod]
    const __xor__: MethodCall = builtin_protocols::xor;

    #[pymethod]
    const __rxor__: MethodCall = builtin_protocols::rxor;

    #[pymethod]
    const __lshift__: MethodCall = builtin_protocols::lshift;

    #[pymethod]
    const __rlshift__: MethodCall = builtin_protocols::rlshift;

    #[pymethod]
    const __rshift__: MethodCall = builtin_protocols::rshift;

    #[pymethod]
    const __rrshift__: MethodCall = builtin_protocols::rrshift;

    #[pymethod]
    const __pow__: MethodCall = builtin_protocols::pow;

    #[pymethod]
    const __rpow__: MethodCall = builtin_protocols::rpow;

    #[pymethod]
    const __eq__: MethodCall = builtin_protocols::eq;

    #[pymethod]
    const __ne__: MethodCall = builtin_protocols::ne;

    #[pymethod]
    const __lt__: MethodCall = builtin_protocols::lt;

    #[pymethod]
    const __le__: MethodCall = builtin_protocols::le;

    #[pymethod]
    const __gt__: MethodCall = builtin_protocols::gt;

    #[pymethod]
    const __ge__: MethodCall = builtin_protocols::ge;
}

/// Python-visible methods for the native Float value.
pub(crate) struct PyFloatAttrs;

#[monty_macros::pyattrs]
impl<'h> PyAttrs<'h> for PyFloatAttrs {
    type Receiver = PyValue<'h>;

    #[pymethod]
    const __repr__: MethodCall = builtin_protocols::repr;

    #[pymethod]
    const __str__: MethodCall = builtin_protocols::str;

    #[pymethod]
    const __bool__: MethodCall = builtin_protocols::bool;

    #[pymethod]
    const __float__: MethodCall = builtin_protocols::float;

    #[pymethod]
    const __round__: MethodCall = builtin_protocols::round;

    #[pymethod]
    const __hash__: MethodCall = builtin_protocols::hash;

    #[pymethod]
    const __format__: MethodCall = builtin_protocols::format;

    #[pymethod]
    const __neg__: MethodCall = builtin_protocols::neg;

    #[pymethod]
    const __pos__: MethodCall = builtin_protocols::pos;

    #[pymethod]
    const __abs__: MethodCall = builtin_protocols::abs;

    #[pymethod]
    const __int__: MethodCall = builtin_protocols::int;

    #[pymethod]
    const __floor__: MethodCall = builtin_protocols::floor;

    #[pymethod]
    const __ceil__: MethodCall = builtin_protocols::ceil;

    #[pymethod]
    const __trunc__: MethodCall = builtin_protocols::trunc;

    #[pymethod]
    const __add__: MethodCall = builtin_protocols::add;

    #[pymethod]
    const __radd__: MethodCall = builtin_protocols::radd;

    #[pymethod]
    const __sub__: MethodCall = builtin_protocols::sub;

    #[pymethod]
    const __rsub__: MethodCall = builtin_protocols::rsub;

    #[pymethod]
    const __mul__: MethodCall = builtin_protocols::mul;

    #[pymethod]
    const __rmul__: MethodCall = builtin_protocols::rmul;

    #[pymethod]
    const __truediv__: MethodCall = builtin_protocols::truediv;

    #[pymethod]
    const __rtruediv__: MethodCall = builtin_protocols::rtruediv;

    #[pymethod]
    const __floordiv__: MethodCall = builtin_protocols::floordiv;

    #[pymethod]
    const __rfloordiv__: MethodCall = builtin_protocols::rfloordiv;

    #[pymethod]
    const __mod__: MethodCall = builtin_protocols::modulo;

    #[pymethod]
    const __rmod__: MethodCall = builtin_protocols::rmod;

    #[pymethod]
    const __divmod__: MethodCall = builtin_protocols::divmod;

    #[pymethod]
    const __rdivmod__: MethodCall = builtin_protocols::rdivmod;

    #[pymethod]
    const __pow__: MethodCall = builtin_protocols::pow;

    #[pymethod]
    const __rpow__: MethodCall = builtin_protocols::rpow;

    #[pymethod]
    const __eq__: MethodCall = builtin_protocols::eq;

    #[pymethod]
    const __ne__: MethodCall = builtin_protocols::ne;

    #[pymethod]
    const __lt__: MethodCall = builtin_protocols::lt;

    #[pymethod]
    const __le__: MethodCall = builtin_protocols::le;

    #[pymethod]
    const __gt__: MethodCall = builtin_protocols::gt;

    #[pymethod]
    const __ge__: MethodCall = builtin_protocols::ge;
}
