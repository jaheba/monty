//! Standard-stream methods backed by the existing host print writer.

use monty_types::PrintStream;

use crate::{
    args::ArgValues,
    bytecode::{CallResult, VM},
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunResult},
    heap::DropWithContext,
    intern::StaticStrings,
    types::{PyTrait, Type},
    value::{EitherStr, Value},
};

/// Dispatches methods on stdout/stderr markers without opening a host file.
pub(crate) fn call(stream: PrintStream, attr: &EitherStr, args: ArgValues, vm: &mut VM<'_>) -> RunResult<CallResult> {
    match attr.static_string(vm.interns) {
        Some(StaticStrings::Write) => write(stream, args, vm).map(CallResult::Value),
        Some(StaticStrings::Flush) => {
            args.check_zero_args("TextIOWrapper.flush", vm.heap)?;
            // Buffering and delivery belong to the host callback, as for print(..., flush=True).
            Ok(CallResult::Value(Value::None))
        }
        _ => {
            args.drop_with(vm);
            Err(ExcType::attribute_error(Type::TextIOWrapper, attr.as_str(vm.interns)))
        }
    }
}

/// Returns characters written, while the print writer retains its byte-based output cap.
fn write(stream: PrintStream, args: ArgValues, vm: &mut VM<'_>) -> RunResult<Value> {
    let text = args
        .reject_kwargs("TextIOWrapper.write", vm.heap)?
        .get_one_arg("TextIOWrapper.write", vm.heap)?;
    defer_drop!(text, vm);
    let output = text.as_str_heap(vm.heap, vm.interns).ok_or_else(|| {
        ExcType::type_error(format!(
            "write() argument must be str, not {}",
            text.py_type(vm).cpython_arg_name(vm.heap, vm.interns)
        ))
    })?;
    let length = i64::try_from(output.chars().count()).map_err(|_| ExcType::overflow_c_ssize_t())?;
    vm.print_writer.write(stream, output.into())?;
    Ok(Value::Int(length))
}
