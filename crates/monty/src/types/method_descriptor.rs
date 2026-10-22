//! Native callable descriptors and their receiver binding.

use crate::{
    args::ArgValues,
    builtins::Builtins,
    bytecode::{CallResult, VM},
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunResult},
    heap::{DropWithContext, HeapData},
    intern::{StaticStrings as S, StringId},
    modules::{ModuleFunctions, itertools::ItertoolsFunctions, random::RandomFunctions},
    os_dispatch::is_path_os_method,
    types::{
        PyTrait, Type,
        builtin_attr::{self, AttrDef},
    },
    value::{EitherStr, Value},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Binding {
    Instance,
    Class,
}

/// Produces the callable and optional receiver without allocating a bound method.
pub(crate) fn resolve(value: &Value, attr: &EitherStr, vm: &VM<'_>) -> Option<(Value, Value)> {
    if matches!(value, Value::Marker(_)) {
        return None;
    }
    if let Value::Ref(id) = value {
        match vm.heap.get(*id) {
            HeapData::Instance(_) | HeapData::Class(_) | HeapData::HostClass(_) | HeapData::HostClassType(_) => {
                return None;
            }
            HeapData::NamedTuple(tuple) => {
                if tuple.get_by_name(attr.as_str(vm.interns), vm.interns).is_some() {
                    return None;
                }
                let name = attr.static_string(vm.interns)?;
                let supported = matches!(name, S::Count | S::Index | S::DunderGetnewargs)
                    || (tuple.class_id().is_some() && matches!(name, S::UnderAsdict | S::UnderReplace | S::UnderMake));
                if supported {
                    return Some((
                        Value::Builtin(Builtins::Method(Type::NamedTuple, vm.interns.intern_static(name))),
                        value.clone_with_heap(vm.heap),
                    ));
                }
            }
            HeapData::NamedTupleClass(_) if attr.static_string(vm.interns) == Some(S::UnderMake) => {
                return Some((
                    Value::Builtin(Builtins::Method(Type::Type, vm.interns.intern_static(S::UnderMake))),
                    value.clone_with_heap(vm.heap),
                ));
            }
            _ => {}
        }
    }
    if matches!(attr.static_string(vm.interns), Some(S::Enter | S::Exit))
        && let Value::Ref(id) = value
        && vm.heap.read(*id).py_is_context_manager(vm)
    {
        let name = vm.interns.intern_static(attr.static_string(vm.interns)?);
        return Some((
            Value::Builtin(Builtins::Method(value.py_type(vm), name)),
            value.clone_with_heap(vm.heap),
        ));
    }
    let owner = match value {
        Value::Builtin(Builtins::Type(owner)) => *owner,
        _ => value.py_type(vm),
    };
    if owner == Type::ItertoolsChain && attr.static_string(vm.interns) == Some(S::FromIterable) {
        // Version-13 dumps can retain this module function as a dictionary key.
        return Some((
            Value::ModuleFunction(ModuleFunctions::Itertools(ItertoolsFunctions::ChainFromIterable)),
            Value::Undefined,
        ));
    }
    let binding = method_kind(owner, attr, vm)?;
    let name = attr.string_id().or_else(|| {
        attr.static_string(vm.interns)
            .map(|name| vm.interns.intern_static(name))
    })?;
    let receiver = if binding == Binding::Class {
        Value::Builtin(Builtins::Type(owner))
    } else if matches!(value, Value::Builtin(Builtins::Type(_))) {
        Value::Undefined
    } else {
        value.clone_with_heap(vm.heap)
    };
    Some((Value::Builtin(Builtins::Method(owner, name)), receiver))
}

pub(crate) fn call(owner: Type, name: StringId, args: ArgValues, vm: &mut VM<'_>) -> RunResult<CallResult> {
    let attr = EitherStr::Interned(name);
    if let Some(AttrDef::Method(call)) = owner.get_attr(&attr, vm.interns) {
        return builtin_attr::invoke_unbound(owner, &attr, call, args, vm);
    }
    let (receiver, args) = match args {
        ArgValues::One(receiver) => (receiver, ArgValues::Empty),
        ArgValues::Two(receiver, arg) => (receiver, ArgValues::One(arg)),
        ArgValues::ArgsKargs { mut args, kwargs } if !args.is_empty() => {
            let receiver = args.remove(0);
            (receiver, ArgValues::from_parts(args, kwargs))
        }
        args => {
            args.drop_with(vm);
            return Err(ExcType::type_error(format!(
                "unbound method {}.{}() needs an argument",
                owner,
                attr.as_str(vm.interns)
            )));
        }
    };
    call_bound(owner, name, receiver, args, vm)
}

/// Invokes a resolved native receiver without packing it into positional arguments.
pub(crate) fn call_bound(
    owner: Type,
    name: StringId,
    receiver: Value,
    args: ArgValues,
    vm: &mut VM<'_>,
) -> RunResult<CallResult> {
    let attr = EitherStr::Interned(name);
    defer_drop!(receiver, vm);
    if let Some(AttrDef::Method(call)) = owner.get_attr(&attr, vm.interns) {
        if receiver.py_type(vm) == owner {
            return call(receiver, args, vm).map(CallResult::Value);
        }
        return builtin_attr::invoke_unbound(owner, &attr, call, args.prepend(receiver.clone_with_heap(vm.heap)), vm);
    }
    if method_kind(owner, &attr, vm) == Some(Binding::Class) {
        return owner.call_class_method(name, args, vm);
    }
    if owner == Type::HostClass {
        if let Value::Ref(id) = receiver {
            let object_id = match vm.heap.get(*id) {
                HeapData::HostClass(instance) => Some(instance.instance_id()),
                HeapData::HostClassType(class) => Some(class.type_id()),
                _ => None,
            };
            if let Some(object_id) = object_id {
                return Ok(CallResult::MethodCall {
                    name: attr,
                    args,
                    object_id,
                });
            }
        }
    } else if receiver.py_type(vm).is_instance_of(owner) {
        return vm.call_attr(receiver.clone_with_heap(vm.heap), name, args);
    }
    args.drop_with(vm);
    Err(ExcType::type_error(format!(
        "descriptor '{}' for '{}' objects doesn't apply to a '{}' object",
        attr.as_str(vm.interns),
        owner,
        receiver.py_type_name(vm)
    )))
}

/// Whether a native attribute binds the class rather than the instance.
fn method_kind(owner: Type, attr: &EitherStr, vm: &VM<'_>) -> Option<Binding> {
    if let Some(def) = owner.get_attr(attr, vm.interns) {
        return match def {
            AttrDef::Method(_) => Some(Binding::Instance),
            AttrDef::ClassMethod(_) => Some(Binding::Class),
            AttrDef::Value(_) => None,
        };
    }
    let name = attr.static_string(vm.interns)?;

    let class_method = match owner {
        Type::Dict | Type::DefaultDict | Type::Counter => name == S::Fromkeys,
        Type::Bytes => name == S::Fromhex,
        Type::Path => name == S::Cwd,
        Type::DateTime => matches!(name, S::Now | S::Strptime | S::Fromisoformat | S::Combine),
        Type::ItertoolsChain => name == S::FromIterable,
        _ => false,
    } || (name == S::ClassGetitem && owner.has_class_getitem());
    if class_method {
        return Some(Binding::Class);
    }

    // Types not yet using #[pyattrs] still dispatch through their native methods.
    let regular = match owner {
        Type::Random => RandomFunctions::from_static_string(name).is_some(),
        Type::Counter if matches!(name, S::MostCommon | S::Elements | S::Total | S::Subtract) => true,
        Type::DefaultDict if name == S::DunderMissing => true,
        Type::Path if is_path_os_method(name) => true,
        Type::Bytes => matches!(
            name,
            S::Capitalize
                | S::Center
                | S::Count
                | S::Decode
                | S::Endswith
                | S::Find
                | S::Fromhex
                | S::Hex
                | S::Index
                | S::Isalnum
                | S::Isalpha
                | S::Isascii
                | S::Isdigit
                | S::Islower
                | S::Isspace
                | S::Istitle
                | S::Isupper
                | S::Join
                | S::Ljust
                | S::Lower
                | S::Lstrip
                | S::Partition
                | S::Removeprefix
                | S::Removesuffix
                | S::Replace
                | S::Rfind
                | S::Rindex
                | S::Rjust
                | S::Rpartition
                | S::Rsplit
                | S::Rstrip
                | S::Split
                | S::Splitlines
                | S::Startswith
                | S::Strip
                | S::Swapcase
                | S::Title
                | S::Upper
                | S::Zfill
        ),
        Type::Deque => matches!(
            name,
            S::Append
                | S::Appendleft
                | S::Clear
                | S::Copy
                | S::Count
                | S::Extend
                | S::Extendleft
                | S::Index
                | S::Insert
                | S::Pop
                | S::Popleft
                | S::Remove
                | S::Reverse
                | S::Rotate
        ),
        Type::DateTime => matches!(
            name,
            S::Astimezone
                | S::Date
                | S::Dst
                | S::Isoformat
                | S::Isoweekday
                | S::Replace
                | S::Strftime
                | S::Time
                | S::Timestamp
                | S::Timetz
                | S::Tzname
                | S::Utcoffset
                | S::Weekday
        ),
        Type::TextIOWrapper | Type::BufferedReader | Type::BufferedWriter | Type::BufferedRandom => matches!(
            name,
            S::Close
                | S::Flush
                | S::Read
                | S::Readable
                | S::Readline
                | S::Readlines
                | S::Seek
                | S::Seekable
                | S::Tell
                | S::Writable
                | S::Write
        ),
        Type::HashlibHash | Type::HashlibHashXof | Type::Blake2b | Type::Blake2s => {
            matches!(name, S::Copy | S::Digest | S::Hexdigest | S::Update)
        }
        Type::LruCacheWrapper => matches!(name, S::CacheClear | S::CacheInfo | S::CacheParameters),
        Type::Set => matches!(
            name,
            S::Add
                | S::Clear
                | S::Copy
                | S::Difference
                | S::Discard
                | S::Intersection
                | S::Isdisjoint
                | S::Issubset
                | S::Issuperset
                | S::Pop
                | S::Remove
                | S::SymmetricDifference
                | S::Union
                | S::Update
        ),
        Type::FrozenSet => matches!(
            name,
            S::Copy
                | S::Difference
                | S::Intersection
                | S::Isdisjoint
                | S::Issubset
                | S::Issuperset
                | S::SymmetricDifference
                | S::Union
        ),
        Type::Dict | Type::DefaultDict | Type::Counter => matches!(
            name,
            S::Clear
                | S::Copy
                | S::Fromkeys
                | S::Get
                | S::Items
                | S::Keys
                | S::Pop
                | S::Popitem
                | S::Setdefault
                | S::Update
                | S::Values
        ),
        Type::Path => matches!(
            name,
            S::AsPosix
                | S::Fspath
                | S::Glob
                | S::IsAbsolute
                | S::Joinpath
                | S::Open
                | S::Rglob
                | S::Walk
                | S::WithName
                | S::WithStem
                | S::WithSuffix
        ),
        Type::DirEntry => matches!(
            name,
            S::Fspath | S::IsDir | S::IsFile | S::IsJunction | S::IsSymlink | S::StatMethod
        ),
        Type::ScandirIterator => matches!(name, S::Close),
        Type::DictKeys | Type::DictItems => matches!(name, S::Isdisjoint),
        Type::CsvWriter => matches!(name, S::Writerow | S::Writerows),
        Type::CsvDictWriter => matches!(name, S::Writeheader | S::Writerow | S::Writerows),
        _ => false,
    };
    regular.then_some(Binding::Instance)
}
