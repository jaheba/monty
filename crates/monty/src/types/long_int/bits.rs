//! Bit queries use magnitudes, including negative values and i64::MIN.

use num_bigint::BigInt;

use super::{LongInt, PyInt, i64_bits};
use crate::{
    args::ArgValues,
    bytecode::VM,
    exception_private::{RunError, RunResult},
    intern::StaticStrings,
    value::Value,
};

/// Small integers stay allocation-free; large population counts poll between digits.
pub(super) fn call<'h>(
    number: &PyInt<'h>,
    method: StaticStrings,
    args: ArgValues,
    vm: &mut VM<'h>,
) -> RunResult<Value> {
    let count = match method {
        StaticStrings::BitLength => {
            args.check_zero_args("int.bit_length", vm.heap)?;
            number.with_value(vm, i64_bits, BigInt::bits)
        }
        StaticStrings::BitCount => {
            args.check_zero_args("int.bit_count", vm.heap)?;
            number.with_value(
                vm,
                |number| Ok(u64::from(number.unsigned_abs().count_ones())),
                |number| {
                    let mut count = 0;
                    for (step, digit) in number.iter_u64_digits().enumerate() {
                        vm.heap.tracker.check_time_every(step)?;
                        count += u64::from(digit.count_ones());
                    }
                    Ok::<_, RunError>(count)
                },
            )?
        }
        _ => unreachable!("integer dispatch only passes bit query methods"),
    };
    Ok(i64::try_from(count).map_or_else(|_| LongInt::new(BigInt::from(count)).into_value(vm.heap), Value::Int))
}
