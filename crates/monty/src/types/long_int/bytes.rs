//! Integer construction from byte strings and integer iterables.

use num_bigint::{BigInt, Sign};
use num_traits::Zero;

use super::LongInt;
use crate::{
    args::{ArgValues, FromArgs},
    bytecode::VM,
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunResult},
    heap::HeapData,
    os_dispatch::value_as_bytes,
    resource_checks::push_checked,
    types::{PyTrait, Type, instance::instance_call_dunder_sync},
    value::Value,
};

/// Implements the inherited classmethod for `int`, `bool`, and their native instances.
pub(crate) fn from_bytes(target: Type, args: ArgValues, vm: &mut VM<'_>) -> RunResult<Value> {
    let FromBytesArgs {
        bytes,
        byteorder,
        signed,
    } = FromBytesArgs::from_args(args, vm)?;
    defer_drop!(bytes, vm);
    defer_drop!(byteorder, vm);
    defer_drop!(signed, vm);
    let order = match byteorder {
        None => Some(false),
        Some(order) => {
            let text = order.as_str(vm).ok_or_else(|| {
                ExcType::type_error(format!(
                    "from_bytes() argument 'byteorder' must be str, not {}",
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
    let number = if let Some(data) = value_as_bytes(bytes, vm.heap, vm.interns) {
        decode_bytes(data, little_endian, signed)
    } else if let Value::Ref(id) = bytes
        && matches!(vm.heap.get(*id), HeapData::Instance(_))
        && let Some(converted) = instance_call_dunder_sync(*id, "__bytes__", None, vm)?
    {
        defer_drop!(converted, vm);
        let data = value_as_bytes(converted, vm.heap, vm.interns).ok_or_else(|| {
            ExcType::type_error(format!(
                "__bytes__ returned non-bytes (type {})",
                converted.py_type_name(vm)
            ))
        })?;
        decode_bytes(data, little_endian, signed)
    } else {
        let data = collect_bytes(bytes, vm)?;
        decode_bytes(&data, little_endian, signed)
    };

    if target == Type::Bool {
        Ok(Value::Bool(!number.is_zero()))
    } else {
        Ok(LongInt::new(number).into_value(vm.heap))
    }
}

/// Keep parameters raw so keyword binding finishes before the Clinic conversion steps.
#[derive(FromArgs)]
#[from_args(name = "from_bytes", style = c_named, at_most_total)]
struct FromBytesArgs {
    #[from_args(static_string = "BytesArg")]
    bytes: Value,
    #[from_args(default)]
    byteorder: Option<Value>,
    #[from_args(kw_only, default = Value::Bool(false))]
    signed: Value,
}

/// Decodes two's complement only for the explicitly signed form.
fn decode_bytes(data: &[u8], little_endian: bool, signed: bool) -> BigInt {
    match (little_endian, signed) {
        (true, true) => BigInt::from_signed_bytes_le(data),
        (false, true) => BigInt::from_signed_bytes_be(data),
        (true, false) => BigInt::from_bytes_le(Sign::Plus, data),
        (false, false) => BigInt::from_bytes_be(Sign::Plus, data),
    }
}

/// Consumes iterable integer bytes without retaining Python values or trusting a length hint.
fn collect_bytes(source: &Value, vm: &mut VM<'_>) -> RunResult<Vec<u8>> {
    if source.as_str(vm).is_some() || !source.py_is_iterable(vm) {
        return Err(ExcType::type_error(format!(
            "cannot convert '{}' object to bytes",
            source.py_type_name(vm)
        )));
    }
    let iterator = source.py_iter(vm)?.require_synchronous(vm)?;
    defer_drop!(iterator, vm);
    let mut iterator = iterator.read(vm);
    let mut data = Vec::new();
    while let Some(item) = iterator.py_next(vm)? {
        defer_drop!(item, vm);
        let byte = u8::try_from(item.saturating_index(vm)?)
            .map_err(|_| ExcType::value_error("bytes must be in range(0, 256)"))?;
        push_checked(&mut data, byte, &vm.heap.tracker)?;
    }
    Ok(data)
}
