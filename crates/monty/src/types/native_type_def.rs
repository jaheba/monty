//! Local definitions of native Python types, registered by the `Type` enum.

use crate::{
    args::ArgValues,
    bytecode::{CallResult, VM},
    exception_private::{ExcType, ExcTypeExt, RunResult},
    heap::DropWithContext,
    types::Type,
    value::{EitherStr, Value},
};

/// Type-level metadata and behavior, independent of an instance's heap representation.
/// Shared payloads can use marker types to define each Python type separately.
pub(crate) trait NativeTypeDef {
    /// CPython's `tp_name`, also used to hash native types in dumps.
    const NAME: &'static str;

    /// Export in the builtins namespace; module types leave this absent.
    const BUILTIN_NAME: Option<&'static str> = None;

    /// Direct native bases; `object` overrides this with an empty slice.
    const BASES: &'static [Type] = &[Type::Object];

    /// May return a value, push a Python frame, or suspend in a host call.
    fn construct(vm: &mut VM<'_>, args: ArgValues) -> RunResult<CallResult> {
        args.drop_with(vm);
        Err(ExcType::type_error_not_callable(Self::NAME))
    }

    /// Returns `None` so lookup can continue through native ancestors.
    fn class_attr(_attr: &EitherStr, _vm: &mut VM<'_>) -> Option<Value> {
        None
    }

    /// Returns unconsumed arguments when generic attribute lookup should handle the call.
    fn class_method(_attr: &EitherStr, args: ArgValues, _vm: &mut VM<'_>) -> Result<RunResult<CallResult>, ArgValues> {
        Err(args)
    }
}
