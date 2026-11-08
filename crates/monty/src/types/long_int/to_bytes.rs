//! Fixed-width integer byte encoding with bounded output allocation.

use monty_types::ResourceTracker;
use num_bigint::BigInt;
use num_traits::{Signed, Zero};

use super::PyInt;
use crate::{
    args::{ArgValues, FromArgs},
    bytecode::VM,
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunResult},
    resource_checks::{check_repeat_size, try_vec_with_capacity},
    string_builder::BytesBuilder,
    types::{PyTrait, bytes::allocate_bytes},
    value::Value,
};

/// Converts Clinic arguments in declaration order before validating byte order and length.
pub(super) fn call<'h>(number: &PyInt<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<Value> {
    let ToBytesArgs {
        length,
        byteorder,
        signed,
    } = ToBytesArgs::from_args(args, vm)?;
    defer_drop!(length, vm);
    defer_drop!(byteorder, vm);
    defer_drop!(signed, vm);
    let length = isize::try_from(length.as_int(vm)?).map_err(|_| ExcType::overflow_c_ssize_t())?;
    let order = match byteorder {
        None => Some(false),
        Some(order) => {
            let text = order.as_str(vm).ok_or_else(|| {
                ExcType::type_error(format!(
                    "to_bytes() argument 'byteorder' must be str, not {}",
                    order.py_type(vm).cpython_arg_name(vm.heap, vm.interns)
                ))
            })?;
            match text {
                "little" => Some(true),
                "big" => Some(false),
                _ => None,
            }
        }
    };
    let signed = signed.py_bool(vm)?;
    let little_endian = order.ok_or_else(|| ExcType::value_error("byteorder must be either 'little' or 'big'"))?;
    let length = usize::try_from(length).map_err(|_| ExcType::value_error("length argument must be non-negative"))?;
    let bytes = number.with_value(
        vm,
        |number| encode(&BigInt::from(number), length, little_endian, signed, &vm.heap.tracker),
        |number| encode(number, length, little_endian, signed, &vm.heap.tracker),
    )?;
    Ok(allocate_bytes(bytes, vm.heap))
}

/// Raw values preserve keyword binding and the ordering of user index/truth callbacks.
#[derive(FromArgs)]
#[from_args(name = "to_bytes", style = c_named, at_most_total)]
struct ToBytesArgs {
    #[from_args(default = Value::Int(1))]
    length: Value,
    #[from_args(default)]
    byteorder: Option<Value>,
    #[from_args(kw_only, default = Value::Bool(false))]
    signed: Value,
}

/// The minimal representation is bounded by the source; padding is bounded by the preflighted length.
fn encode(
    number: &BigInt,
    length: usize,
    little_endian: bool,
    signed: bool,
    tracker: &ResourceTracker,
) -> RunResult<Vec<u8>> {
    let negative = number.is_negative();
    if negative && !signed {
        return Err(ExcType::overflow_error("can't convert negative int to unsigned"));
    }
    if length == 0 {
        return if number.is_zero() {
            Ok(Vec::new())
        } else {
            Err(ExcType::overflow_error("int too big to convert"))
        };
    }

    // Choose byte order before padding to avoid reversing an attacker-sized result.
    let minimal = match (little_endian, signed) {
        (true, true) => number.to_signed_bytes_le(),
        (false, true) => number.to_signed_bytes_be(),
        (true, false) => number.to_bytes_le().1,
        (false, false) => number.to_bytes_be().1,
    };
    if minimal.len() > length {
        return Err(ExcType::overflow_error("int too big to convert"));
    }
    check_repeat_size(length, 1, tracker)?;
    let buffer = try_vec_with_capacity(length)?;
    let mut bytes = BytesBuilder::from_existing(buffer, tracker);
    let padding = length - minimal.len();
    let fill = if negative { 0xff } else { 0 };
    if little_endian {
        bytes.push_slice(&minimal)?;
        bytes.push_repeated(fill, padding)?;
    } else {
        bytes.push_repeated(fill, padding)?;
        bytes.push_slice(&minimal)?;
    }
    Ok(bytes.finish())
}
