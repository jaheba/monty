//! Home expansion uses only the HOME value supplied by the sandbox's OS handler.

use monty_types::{
    GetenvArgs, MontyObject, OsFunctionCall,
    unstable::{self, MontyNode},
};

use super::Path;
use crate::{
    args::ArgValues,
    bytecode::{CallResult, VM},
    exception_private::{ExcType, ExcTypeExt, RunError, RunResult, SimpleException},
    heap::{DropWithContext, HeapObjectRead},
    modules::os_path::posix,
    os_dispatch::{PreConversionEffect, invalid_reply},
};

/// Expands `~` for the classmethod, also callable through a Path instance.
pub(crate) fn class_home(vm: &mut VM<'_>, args: ArgValues) -> RunResult<CallResult> {
    check_no_args(args, "Path.home", "cls", vm)?;
    Ok(home_call(String::new(), true))
}

/// Keeps paths without a leading tilde unchanged and rejects named-user database lookups.
pub(super) fn expanduser<'h>(
    path: &HeapObjectRead<'h, Path>,
    vm: &mut VM<'h>,
    args: ArgValues,
) -> RunResult<CallResult> {
    check_no_args(args, "Path.expanduser", "self", vm)?;
    let text = path.get(vm.heap).as_str();
    if !text.starts_with('~') {
        return Ok(CallResult::Value(path.clone_value(vm.heap)));
    }
    if text == "~" || text.starts_with("~/") {
        Ok(home_call(text[1..].to_owned(), false))
    } else {
        Err(no_home())
    }
}

/// Python's zero-argument methods count the bound receiver and report keyword conflicts first.
fn check_no_args(args: ArgValues, name: &str, receiver: &str, vm: &mut VM<'_>) -> RunResult<()> {
    if matches!(args, ArgValues::Empty) {
        return Ok(());
    }

    let (pos, kwargs) = args.into_parts();
    let given = pos.len() + 1;
    let first_kwarg = kwargs.first_key(vm.heap, vm.interns);
    pos.drop_with(vm);
    kwargs.drop_with(vm);
    Err(match first_kwarg? {
        Some(key) if key == receiver => {
            ExcType::type_error(format!("{name}() got multiple values for argument '{receiver}'"))
        }
        Some(key) => ExcType::type_error_unexpected_keyword(name, &key),
        None => ExcType::type_error(format!("{name}() takes 1 positional argument but {given} were given")),
    })
}

/// Requests the same virtual HOME used by os.path, never the process environment directly.
fn home_call(tail: String, is_home: bool) -> CallResult {
    CallResult::OsCallWithEffect {
        call: OsFunctionCall::Getenv(GetenvArgs {
            key: "HOME".to_owned(),
            default: MontyObject::none(),
        }),
        effect: PreConversionEffect::PathExpandUser { tail, is_home }.into(),
    }
}

/// Turns the host-controlled HOME into a virtual Path; an unresolved tilde is an error.
pub(crate) fn reply(value: &MontyObject, tail: &str, is_home: bool) -> Result<MontyObject, RunError> {
    match unstable::root_node(value) {
        MontyNode::None => Err(no_home()),
        MontyNode::String(home) => {
            let expanded = posix::expand_home(home.as_bytes(), tail.as_bytes());
            let expanded = String::from_utf8(expanded).expect("HOME and path tail are valid UTF-8");
            if expanded.starts_with('~') {
                Err(no_home())
            } else {
                Ok(MontyObject::path(Path::new(expanded).path))
            }
        }
        _ => Err(invalid_reply(
            if is_home { "Path.home" } else { "Path.expanduser" },
            "str or None",
            value,
        )),
    }
}

/// Named users and missing HOME cannot fall back to a host user database.
fn no_home() -> RunError {
    SimpleException::new_msg(ExcType::RuntimeError, "Could not determine home directory.").into()
}
