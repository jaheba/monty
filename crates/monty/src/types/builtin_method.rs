//! Callable handles for the existing native attribute-call dispatcher.

use std::fmt;

use crate::{
    args::ArgValues,
    bytecode::{CallResult, VM},
    exception_private::{ExcType, ExcTypeExt, RunResult},
    heap::DropWithContext,
    intern::StringId,
};

/// A builtin method's interned name; its receiver is held by `BoundMethod`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) struct BuiltinMethod {
    #[serde(rename = "N")]
    name: StringId,
}

impl BuiltinMethod {
    pub(crate) fn new(name: StringId) -> Self {
        Self { name }
    }

    /// Removes the bound receiver and delegates the remaining arguments unchanged.
    pub(crate) fn call(self, vm: &mut VM<'_>, args: ArgValues) -> RunResult<CallResult> {
        let (receiver, args) = match args {
            ArgValues::One(receiver) => (receiver, ArgValues::Empty),
            ArgValues::Two(receiver, arg) => (receiver, ArgValues::One(arg)),
            ArgValues::ArgsKargs { mut args, kwargs } if !args.is_empty() => {
                let receiver = args.remove(0);
                (receiver, ArgValues::from_parts(args, kwargs))
            }
            args => {
                args.drop_with(vm);
                return Err(ExcType::type_error("native method requires a receiver"));
            }
        };
        vm.call_native_method(receiver, self.name, args)
    }
}

impl fmt::Display for BuiltinMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("native method")
    }
}
