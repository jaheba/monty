//! Descriptor access and binding for members of sandbox classes.

use super::instance::{bind_method, class_defines, class_member, is_method_value};
use crate::{
    args::{ArgValues, KwargsValues},
    bytecode::VM,
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunResult},
    heap::{HeapData, HeapId},
    value::Value,
};

fn descriptor_class(value: &Value, vm: &VM<'_>) -> Option<HeapId> {
    match value {
        Value::Ref(id) => match vm.heap.get(*id) {
            HeapData::Instance(instance) => Some(instance.class()),
            _ => None,
        },
        _ => None,
    }
}

/// A getter with a setter or deleter takes precedence over instance storage.
pub(crate) fn has_data_getter(value: &Value, vm: &VM<'_>) -> bool {
    if let Value::Ref(id) = value
        && let HeapData::BuiltinDescriptor(descriptor) = vm.heap.get(*id)
    {
        return descriptor.kind == super::Type::Property;
    }
    descriptor_class(value, vm).is_some_and(|class| {
        class_defines(class, "__get__", vm)
            && (class_defines(class, "__set__", vm) || class_defines(class, "__delete__", vm))
    })
}

/// Resolves a class member for instance or class access, returning an owned value.
pub(crate) fn get(value: &Value, instance: Option<HeapId>, owner: HeapId, vm: &mut VM<'_>) -> RunResult<Value> {
    if let Value::Ref(id) = value
        && matches!(vm.heap.get(*id), HeapData::BuiltinDescriptor(_))
    {
        return super::builtin_descriptor::get(*id, instance, owner, vm);
    }
    if let Some(class) = descriptor_class(value, vm)
        && let Some(getter) = class_member(class, "__get__", vm)
    {
        defer_drop!(getter, vm);
        let receiver = instance.map_or(Value::None, |id| {
            vm.heap.inc_ref(id);
            Value::Ref(id)
        });
        vm.heap.inc_ref(owner);
        let args = ArgValues::ArgsKargs {
            args: vec![value.clone_with_heap(vm), receiver, Value::Ref(owner)],
            kwargs: KwargsValues::Empty,
        };
        // CPython calls the raw type-level __get__ with all three arguments.
        return vm.evaluate_function("__get__", getter, args);
    }
    if let Some(instance) = instance
        && is_method_value(value, vm)
    {
        return Ok(bind_method(value, instance, vm));
    }
    Ok(value.clone_with_heap(vm))
}

/// Calls a descriptor setter when present, including rejection of delete-only descriptors.
pub(crate) fn set(descriptor: &Value, instance: HeapId, value: &Value, vm: &mut VM<'_>) -> RunResult<bool> {
    if let Value::Ref(id) = descriptor
        && let HeapData::BuiltinDescriptor(builtin) = vm.heap.get(*id)
        && builtin.kind == super::Type::Property
    {
        super::builtin_descriptor::set(*id, instance, value, vm)?;
        return Ok(true);
    }
    let Some(class) = descriptor_class(descriptor, vm) else {
        return Ok(false);
    };
    let setter = class_member(class, "__set__", vm);
    if setter.is_none() && !class_defines(class, "__delete__", vm) {
        return Ok(false);
    }
    let Some(setter) = setter else {
        return Err(ExcType::attribute_error_type("descriptor", "__set__"));
    };
    defer_drop!(setter, vm);
    let Value::Ref(id) = descriptor else { unreachable!() };
    let callable = get(setter, Some(*id), class, vm)?;
    defer_drop!(callable, vm);
    vm.heap.inc_ref(instance);
    let args = ArgValues::Two(Value::Ref(instance), value.clone_with_heap(vm));
    let result = vm.evaluate_function("__set__", callable, args)?;
    defer_drop!(result, vm);
    Ok(true)
}
