//! Exact float conversion and rounding shared by numeric types.

use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::Pow;
use smallvec::smallvec;

use crate::{
    bytecode::VM,
    exception_private::{ExcType, ExcTypeExt, RunResult},
    heap::Heap,
    types::{LongInt, allocate_tuple},
    value::Value,
};

/// The reduced numerator and denominator each fit at most 1,075 binary digits.
pub(crate) fn float_integer_ratio(value: f64, heap: &Heap) -> RunResult<Value> {
    if value.is_nan() {
        return Err(ExcType::value_error_integer_ratio_nan());
    }
    if value.is_infinite() {
        return Err(ExcType::overflow_integer_ratio_infinity());
    }

    let (numerator, power) = positive_float_ratio(value.abs());
    let numerator = if value.is_sign_negative() {
        -numerator
    } else {
        numerator
    };
    let denominator = BigInt::from(1u8) << power;
    Ok(allocate_tuple(
        smallvec![
            LongInt::new(numerator).into_value(heap),
            LongInt::new(denominator).into_value(heap)
        ],
        heap,
    ))
}

/// Decomposes a finite, nonnegative float as `n / 2**k` in lowest terms.
pub(crate) fn positive_float_ratio(value: f64) -> (BigInt, u32) {
    if value == 0.0 {
        return (BigInt::from(0u8), 0);
    }
    let bits = value.to_bits();
    let exponent = i32::try_from((bits >> 52) & 0x7ff).expect("11-bit field");
    let fraction = bits & ((1 << 52) - 1);
    let (mut mantissa, mut exp2) = if exponent == 0 {
        (fraction, -1074)
    } else {
        (fraction | (1 << 52), exponent - 1075)
    };
    // Remove the factors of two shared by numerator and denominator.
    while exp2 < 0 && mantissa & 1 == 0 {
        mantissa >>= 1;
        exp2 += 1;
    }
    if exp2 >= 0 {
        (BigInt::from(mantissa) << exp2.unsigned_abs(), 0)
    } else {
        (BigInt::from(mantissa), exp2.unsigned_abs())
    }
}

/// Formats the IEEE-754 fields directly, preserving signed zero and subnormal exponents.
pub(crate) fn float_hex(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_owned();
    }
    let sign = if value.is_sign_negative() { "-" } else { "" };
    if value.is_infinite() {
        return format!("{sign}inf");
    }
    if value == 0.0 {
        return format!("{sign}0x0.0p+0");
    }

    let bits = value.to_bits();
    let raw_exponent = i32::try_from((bits >> 52) & 0x7ff).expect("11-bit field");
    let fraction = bits & ((1_u64 << 52) - 1);
    // Subnormals have no implicit leading one and retain the minimum normal exponent.
    let (leading, exponent) = if raw_exponent == 0 {
        (0, -1022)
    } else {
        (1, raw_exponent - 1023)
    };
    format!("{sign}0x{leading}.{fraction:013x}p{exponent:+}")
}

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
