//! Construction from replacement segments through the existing path constructor.

use super::Path;
use crate::{
    args::{ArgValues, FromArgs, KwargsValues},
    bytecode::VM,
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunResult},
    heap::DropGuard,
    value::Value,
};

/// The bound receiver is omitted from construction but still conflicts with a `self` keyword.
pub(super) fn call(args: ArgValues, vm: &mut VM<'_>) -> RunResult<Value> {
    let WithSegmentsArgs { segments, keywords } = WithSegmentsArgs::from_args(args, vm)?;
    let mut segments = DropGuard::new(segments, vm);
    {
        let (_, vm) = segments.as_parts_mut();
        defer_drop!(keywords, vm);
        if let Some(key) = keywords.first_key(vm.heap, vm.interns)? {
            return Err(if key == "self" {
                ExcType::type_error("PurePath.with_segments() got multiple values for argument 'self'")
            } else {
                ExcType::type_error_unexpected_keyword("PurePath.with_segments", &key)
            });
        }
    }

    let (segments, vm) = segments.into_parts();
    Path::init(vm, ArgValues::from_parts(segments, KwargsValues::Empty))
}

/// Collect keywords before construction so binding errors precede invalid segment types.
#[derive(FromArgs)]
#[from_args(name = "PurePath.with_segments", style = def)]
struct WithSegmentsArgs {
    #[from_args(varargs)]
    segments: Vec<Value>,
    #[from_args(varkwargs)]
    keywords: KwargsValues,
}
