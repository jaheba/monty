//! The `callable()` predicate, without invoking the value or its attribute hooks.

use crate::{
    args::ArgValues, bytecode::VM, defer_drop, exception_private::RunResult, heap::HeapData, types::Type, value::Value,
};

/// Tests for a call slot, including host proxies whose eventual call is host-policy dependent.
pub(super) fn builtin_callable(vm: &mut VM<'_>, args: ArgValues) -> RunResult<Value> {
    let value = args
        .reject_kwargs("callable", vm.heap)?
        .get_one_arg("callable", vm.heap)?;
    defer_drop!(value, vm);
    // Internal callback checks intentionally accept fewer types than Python's call-slot predicate.
    let callable = value.is_callable(vm)
        // Typing placeholders represent CPython class/special-form objects, which carry a call slot.
        || matches!(value, Value::Marker(marker) if marker.py_type() == Type::SpecialForm)
        || matches!(value, Value::Ref(id) if matches!(vm.heap.get(*id),
            HeapData::NamedTupleClass(_) | HeapData::HostClass(_) | HeapData::HostClassType(_)));
    Ok(Value::Bool(callable))
}
