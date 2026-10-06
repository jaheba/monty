//! The builtin property, classmethod and staticmethod wrappers.

use std::{
    fmt::{self, Write},
    mem,
};

use super::{LazyHeapSet, PyTrait, Type, instance::bind_method};
use crate::{
    args::{ArgValues, FromArgs},
    bytecode::{CallResult, RunReentryGuard, VM},
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunError, RunResult, SimpleException},
    hash::{HashValue, identity_hash},
    heap::{DropGuard, DropWithContext, HeapData, HeapId, HeapItem, HeapObjectRead},
    modules::ModuleFunctions,
    value::{EitherStr, Value},
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct BuiltinDescriptor {
    pub kind: Type,
    // Property: fget, fset, fdel, doc. Other wrappers use only the first slot.
    pub values: [Value; 4],
}

#[derive(FromArgs)]
#[from_args(name = "property")]
struct PropertyArgs {
    #[from_args(default)]
    fget: Option<Value>,
    #[from_args(default)]
    fset: Option<Value>,
    #[from_args(default)]
    fdel: Option<Value>,
    #[from_args(default)]
    doc: Option<Value>,
}

#[derive(FromArgs)]
#[from_args(name = "classmethod")]
struct ClassMethodArgs {
    #[from_args(pos_only)]
    function: Value,
}

#[derive(FromArgs)]
#[from_args(name = "staticmethod")]
struct StaticMethodArgs {
    #[from_args(pos_only)]
    function: Value,
}

#[derive(FromArgs)]
#[from_args(name = "property accessor")]
struct AccessorArgs {
    #[from_args(pos_only)]
    receiver: Value,
    #[from_args(pos_only)]
    function: Value,
}

pub(crate) fn property_init(vm: &mut VM<'_>, args: ArgValues) -> RunResult<Value> {
    let PropertyArgs { fget, fset, fdel, doc } = PropertyArgs::from_args(args, vm)?;
    Ok(allocate(
        BuiltinDescriptor {
            kind: Type::Property,
            values: [
                fget.unwrap_or(Value::None),
                fset.unwrap_or(Value::None),
                fdel.unwrap_or(Value::None),
                doc.unwrap_or(Value::None),
            ],
        },
        vm,
    ))
}

pub(crate) fn classmethod_init(vm: &mut VM<'_>, args: ArgValues) -> RunResult<Value> {
    let ClassMethodArgs { function } = ClassMethodArgs::from_args(args, vm)?;
    Ok(allocate(
        BuiltinDescriptor {
            kind: Type::ClassMethod,
            values: [function, Value::None, Value::None, Value::None],
        },
        vm,
    ))
}

pub(crate) fn staticmethod_init(vm: &mut VM<'_>, args: ArgValues) -> RunResult<Value> {
    let StaticMethodArgs { function } = StaticMethodArgs::from_args(args, vm)?;
    Ok(allocate(
        BuiltinDescriptor {
            kind: Type::StaticMethod,
            values: [function, Value::None, Value::None, Value::None],
        },
        vm,
    ))
}

fn allocate(descriptor: BuiltinDescriptor, vm: &mut VM<'_>) -> Value {
    Value::Ref(vm.heap.allocate(HeapData::BuiltinDescriptor(Box::new(descriptor))))
}

pub(crate) fn get(id: HeapId, instance: Option<HeapId>, owner: HeapId, vm: &mut VM<'_>) -> RunResult<Value> {
    let HeapData::BuiltinDescriptor(descriptor) = vm.heap.get(id) else {
        unreachable!()
    };
    match descriptor.kind {
        Type::StaticMethod => Ok(descriptor.values[0].clone_with_heap(vm)),
        Type::ClassMethod => Ok(bind_method(&descriptor.values[0], owner, vm)),
        Type::Property => {
            let Some(instance) = instance else {
                vm.heap.inc_ref(id);
                return Ok(Value::Ref(id));
            };
            let function = descriptor.values[0].clone_with_heap(vm);
            defer_drop!(function, vm);
            if matches!(function, Value::None) {
                return Err(missing_accessor("getter"));
            }
            vm.heap.inc_ref(instance);
            vm.evaluate_function("property getter", function, ArgValues::One(Value::Ref(instance)))
        }
        _ => unreachable!("native descriptor constructor selects its kind"),
    }
}

pub(crate) fn set(id: HeapId, instance: HeapId, value: &Value, vm: &mut VM<'_>) -> RunResult<()> {
    let HeapData::BuiltinDescriptor(descriptor) = vm.heap.get(id) else {
        unreachable!()
    };
    let function = descriptor.values[1].clone_with_heap(vm);
    defer_drop!(function, vm);
    if matches!(function, Value::None) {
        return Err(missing_accessor("setter"));
    }
    vm.heap.inc_ref(instance);
    let args = ArgValues::Two(Value::Ref(instance), value.clone_with_heap(vm));
    let result = vm.evaluate_function("property setter", function, args)?;
    defer_drop!(result, vm);
    Ok(())
}

fn missing_accessor(accessor: &str) -> RunError {
    SimpleException::new_msg(ExcType::AttributeError, format!("property has no {accessor}")).into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub(crate) enum DescriptorMethod {
    Getter,
    Setter,
    Deleter,
}

impl fmt::Display for DescriptorMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Getter => "property.getter",
            Self::Setter => "property.setter",
            Self::Deleter => "property.deleter",
        })
    }
}

impl DescriptorMethod {
    pub(crate) fn call(self, vm: &mut VM<'_>, args: ArgValues) -> RunResult<Value> {
        let AccessorArgs { receiver, function } = AccessorArgs::from_args(args, vm)?;
        let mut guard = DropGuard::new((receiver, function), vm);
        let ((receiver, _), vm) = guard.as_parts_mut();
        let mut values = if let Value::Ref(id) = receiver
            && let HeapData::BuiltinDescriptor(descriptor) = vm.heap.get(*id)
            && descriptor.kind == Type::Property
        {
            descriptor.values.each_ref().map(|v| v.clone_with_heap(vm))
        } else {
            return Err(ExcType::type_error("property accessor requires a property"));
        };
        let ((receiver, function), vm) = guard.into_parts();
        receiver.drop_with(vm);
        let index = match self {
            Self::Getter => 0,
            Self::Setter => 1,
            Self::Deleter => 2,
        };
        mem::replace(&mut values[index], function).drop_with(vm);
        Ok(Value::Ref(vm.heap.allocate(HeapData::BuiltinDescriptor(Box::new(
            BuiltinDescriptor {
                kind: Type::Property,
                values,
            },
        )))))
    }
}

impl HeapItem for BuiltinDescriptor {
    fn py_dec_ref_ids(&mut self, stack: &mut Vec<HeapId>) {
        for value in &mut self.values {
            value.py_dec_ref_ids(stack);
        }
    }
}

impl<'h> PyTrait<'h> for HeapObjectRead<'h, BuiltinDescriptor> {
    fn py_type(&self, vm: &VM<'h>) -> Type {
        self.get(vm.heap).kind
    }
    fn py_len(&self, _: &VM<'h>) -> Option<usize> {
        None
    }
    fn py_eq_impl(&self, _: &Value, _: &mut VM<'h>) -> RunResult<Option<bool>> {
        Ok(None)
    }
    fn py_hash(&self, _: &mut VM<'h>) -> RunResult<Option<HashValue>> {
        Ok(Some(identity_hash(self.id())))
    }
    fn py_repr_fmt(&self, f: &mut impl Write, vm: &mut VM<'h>, _: &mut LazyHeapSet) -> RunResult<()> {
        Ok(write!(f, "<{} object>", self.get(vm.heap).kind)?)
    }
    fn py_getattr(&self, attr: &EitherStr, vm: &mut VM<'h>) -> RunResult<Option<CallResult>> {
        let descriptor = self.get(vm.heap);
        let name = attr.as_str(vm.interns);
        let value = if descriptor.kind == Type::Property {
            let index = match name {
                "fget" => Some(0),
                "fset" => Some(1),
                "fdel" => Some(2),
                "__doc__" => Some(3),
                _ => None,
            };
            if let Some(index) = index {
                descriptor.values[index].clone_with_heap(vm)
            } else {
                let method = match name {
                    "getter" => DescriptorMethod::Getter,
                    "setter" => DescriptorMethod::Setter,
                    "deleter" => DescriptorMethod::Deleter,
                    _ => return Ok(None),
                };
                bind_method(
                    &Value::ModuleFunction(ModuleFunctions::Descriptor(method)),
                    self.id(),
                    vm,
                )
            }
        } else if matches!(name, "__func__" | "__wrapped__") {
            descriptor.values[0].clone_with_heap(vm)
        } else {
            return Ok(None);
        };
        Ok(Some(CallResult::Value(value)))
    }
    fn py_call_attr(&mut self, vm: &mut VM<'h>, attr: &EitherStr, args: ArgValues) -> RunResult<CallResult> {
        // Attribute resolution can fail before the argument bundle is consumed.
        let mut guard = DropGuard::new(args, vm);
        let (_, vm) = guard.as_parts_mut();
        let Some(CallResult::Value(callable)) = self.py_getattr(attr, vm)? else {
            return Err(ExcType::attribute_error(self.py_type(vm), attr.as_str(vm.interns)));
        };
        let (args, vm) = guard.into_parts();
        defer_drop!(callable, vm);
        vm.call_function(callable, args)
    }
    fn py_call(&mut self, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
        let descriptor = self.get(vm.heap);
        if descriptor.kind != Type::StaticMethod {
            args.drop_with(vm);
            return Err(ExcType::type_error_not_callable_object(&self.py_type_name(vm)));
        }
        let function = descriptor.values[0].clone_with_heap(vm);
        let mut guard = DropGuard::new((function, args), vm);
        let (_, vm) = guard.as_parts_mut();
        vm.enter_run_reentry()?;
        let ((function, args), vm) = guard.into_parts();
        let mut reentry = RunReentryGuard::new(vm);
        let vm = &mut *reentry;
        defer_drop!(function, vm);
        vm.call_function(function, args)
    }
}
