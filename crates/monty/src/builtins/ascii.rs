//! ASCII-only representations using the existing Python repr protocol.

use std::fmt::Write;

use crate::{
    args::ArgValues,
    bytecode::VM,
    defer_drop,
    exception_private::RunResult,
    types::{PyTrait, str::allocate_string},
    value::Value,
};

/// Escapes non-ASCII repr characters with Python's code-point-dependent escape widths.
pub(super) fn builtin_ascii(vm: &mut VM<'_>, args: ArgValues) -> RunResult<Value> {
    let value = args.reject_kwargs("ascii", vm.heap)?.get_one_arg("ascii", vm.heap)?;
    defer_drop!(value, vm);
    let repr = value.py_repr(vm)?;
    defer_drop!(repr, vm);
    let text = repr.as_str(vm).expect("py_repr returns a string");
    if text.is_ascii() {
        return Ok(repr.clone_with_heap(vm.heap));
    }

    // Each input character expands to at most ten ASCII bytes, so growth is bounded by the tracked repr.
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if c.is_ascii() {
            escaped.push(c);
        } else {
            let codepoint = u32::from(c);
            if codepoint <= 0xff {
                write!(escaped, "\\x{codepoint:02x}")?;
            } else if codepoint <= 0xffff {
                write!(escaped, "\\u{codepoint:04x}")?;
            } else {
                write!(escaped, "\\U{codepoint:08x}")?;
            }
        }
    }
    Ok(allocate_string(escaped, vm.heap))
}
