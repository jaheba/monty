//! Pure path matching using the directory scanner's component matcher.

use monty_types::scan::SegmentPattern;

use super::Path;
use crate::{
    args::{ArgValues, FromArgs},
    bytecode::VM,
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunResult},
    os_dispatch::value_as_path_str,
    types::PyTrait,
    value::Value,
};

/// Matches relative patterns from the right; absolute patterns must cover the whole path.
pub(super) fn path_match(path: &Path, args: ArgValues, vm: &mut VM<'_>) -> RunResult<Value> {
    let PathMatchArgs {
        receiver,
        path_pattern,
        case_sensitive,
    } = PathMatchArgs::from_args(args.prepend(Value::None), vm)?;
    receiver.drop_with(vm);
    defer_drop!(path_pattern, vm);
    defer_drop!(case_sensitive, vm);

    let pattern = value_as_path_str(path_pattern, vm.heap, vm.interns)
        .map(|text| Path::new(text.to_owned()))
        .ok_or_else(|| {
            ExcType::type_error(format!(
                "argument should be a str or an os.PathLike object where __fspath__ returns a str, not '{}'",
                path_pattern.py_type_name(vm)
            ))
        })?;
    let parts = path.parts();
    let pattern_parts = pattern.parts();
    if pattern_parts.is_empty() {
        return Err(ExcType::value_error("empty pattern"));
    }
    if parts.len() < pattern_parts.len() || pattern.is_absolute() && parts.len() > pattern_parts.len() {
        return Ok(Value::Bool(false));
    }

    let case_sensitive = if matches!(case_sensitive, Value::None) {
        true
    } else {
        // CPython hashes this argument for its compiled-pattern cache before testing its truth value.
        case_sensitive
            .py_hash(vm)?
            .ok_or_else(|| ExcType::type_error_unhashable(&case_sensitive.py_type_name(vm)))?;
        case_sensitive.py_bool(vm)?
    };

    for (part, pattern_part) in parts.iter().rev().zip(pattern_parts.iter().rev()) {
        // A wildcard cannot consume the root separator, even though it is a pathlib component.
        if *part == "/" || *pattern_part == "/" {
            if part != pattern_part {
                return Ok(Value::Bool(false));
            }
        } else if !SegmentPattern::new(pattern_part, case_sensitive)
            .try_matches(part, |step| vm.heap.tracker.check_time_every(step))?
        {
            return Ok(Value::Bool(false));
        }
    }
    Ok(Value::Bool(true))
}

/// The bound receiver remains in the signature to match pure-Python arity errors.
#[derive(FromArgs)]
#[from_args(name = "PurePath.match", style = def)]
struct PathMatchArgs {
    #[from_args(pos_only)]
    receiver: Value,
    path_pattern: Value,
    #[from_args(kw_only, default = Value::None)]
    case_sensitive: Value,
}
