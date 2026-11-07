//! Translation-table operations with Python callbacks in ordinary VM frames.

use monty_types::BuiltinsFunctions;

use super::PyStr;
use crate::{
    args::{ArgValues, FromArgs},
    builtins::Builtins,
    bytecode::{CallResult, VM},
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunResult},
    frozen::{FrozenFunction, FrozenFunctions},
    heap::HeapReadOutput,
    intern::StringId,
    modules::ModuleFunctions,
    types::{BuiltinType, PyTrait},
    value::Value,
};

/// Keeps the source alive across suspendable mapping lookups without copying its text.
pub(super) fn translate<'h>(source: &PyStr<'h>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<CallResult> {
    let table = args
        .reject_kwargs("str.translate", vm.heap)?
        .get_one_arg("str.translate", vm.heap)?;
    let source = match source {
        PyStr::Interned(id) => Value::InternString(*id),
        PyStr::Inline(text) => Value::InlineString(*text),
        PyStr::Heap(value) => value.clone_value(vm.heap),
    };
    vm.call_frozen_exact(
        FrozenFunction::StrTranslate,
        [
            source,
            table,
            Value::ModuleFunction(ModuleFunctions::Frozen(FrozenFunctions::TranslateLookup)),
            Value::Builtin(Builtins::Function(BuiltinsFunctions::Ord)),
            Value::Builtin(Builtins::Function(BuiltinsFunctions::Chr)),
            Value::Builtin(Builtins::Function(BuiltinsFunctions::Isinstance)),
            Value::Builtin(Builtins::Type(BuiltinType::Int)),
            Value::Builtin(Builtins::Type(BuiltinType::Str)),
            Value::Builtin(Builtins::ExcType(ExcType::LookupError)),
            Value::Builtin(Builtins::ExcType(ExcType::TypeError)),
            Value::Builtin(Builtins::ExcType(ExcType::ValueError)),
        ],
    )
}

/// Preserves special-method lookup rules, including ignoring an instance's own __getitem__ attribute.
pub(crate) fn call_lookup(args: ArgValues, vm: &mut VM<'_>) -> RunResult<CallResult> {
    let (table, key) = args.get_two_args("translate_lookup", vm.heap)?;
    defer_drop!(table, vm);
    defer_drop!(key, vm);
    if let Some(HeapReadOutput::Instance(instance)) = table.read_heap(vm) {
        instance.call_getitem(key.clone_with_heap(vm.heap), vm)
    } else {
        table.py_getitem(key, vm).map(CallResult::Value)
    }
}

/// The one-dict and two/three-string forms share binding, but validate different operands.
pub(crate) fn maketrans(args: ArgValues, vm: &mut VM<'_>) -> RunResult<CallResult> {
    let MaketransArgs { x, y, z } = MaketransArgs::from_args(args, vm)?;
    defer_drop!(x, vm);
    defer_drop!(y, vm);
    defer_drop!(z, vm);
    // CPython validates the optional strings before testing the first argument's form.
    for (argument, position) in [(y, 2), (z, 3)] {
        if let Some(value) = argument
            && !value.is_str(vm.heap)
        {
            return Err(ExcType::type_error(format!(
                "maketrans() argument {position} must be str, not {}",
                value.py_type(vm).cpython_arg_name(vm.heap, vm.interns)
            )));
        }
    }
    let has_y = y.is_some();
    let x = x.clone_with_heap(vm.heap);
    let y = y.as_ref().map_or(Value::None, |value| value.clone_with_heap(vm.heap));
    let z = z.as_ref().map_or(Value::InternString(StringId::EMPTY), |value| {
        value.clone_with_heap(vm.heap)
    });
    vm.call_frozen_exact(
        FrozenFunction::StrMaketrans,
        [
            x,
            y,
            z,
            Value::Bool(has_y),
            Value::Builtin(Builtins::Function(BuiltinsFunctions::Len)),
            Value::Builtin(Builtins::Function(BuiltinsFunctions::Ord)),
            Value::Builtin(Builtins::Type(BuiltinType::Iterator)),
            Value::Builtin(Builtins::Function(BuiltinsFunctions::Next)),
            Value::Builtin(Builtins::Function(BuiltinsFunctions::Isinstance)),
            Value::Builtin(Builtins::Type(BuiltinType::Int)),
            Value::Builtin(Builtins::Type(BuiltinType::Str)),
            Value::Builtin(Builtins::Type(BuiltinType::Dict)),
            Value::Builtin(Builtins::ExcType(ExcType::TypeError)),
            Value::Builtin(Builtins::ExcType(ExcType::ValueError)),
        ],
    )
}

/// Absence differs from explicit None in both optional string positions.
#[derive(FromArgs)]
#[from_args(name = "maketrans", style = unpack, kwarg_error_name = "str.maketrans")]
struct MaketransArgs {
    #[from_args(pos_only)]
    x: Value,
    #[from_args(pos_only, default)]
    y: Option<Value>,
    #[from_args(pos_only, default)]
    z: Option<Value>,
}

/// A missing mapping entry preserves its character; other exceptions propagate.
pub(crate) const TRANSLATE_SOURCE: &str = r"
def translate(text, table, lookup, ord_, chr_, isinstance_, int_, str_, lookup_error, type_error, value_error):
    pieces = []
    for character in text:
        try:
            replacement = lookup(table, ord_(character))
        except lookup_error:
            replacement = character
        if replacement is None:
            continue
        if isinstance_(replacement, int_):
            if not 0 <= replacement <= 0x10ffff:
                raise value_error('character mapping must be in range(0x110000)')
            replacement = chr_(replacement)
        elif not isinstance_(replacement, str_):
            raise type_error('character mapping must return integer, None or str')
        pieces.append(replacement)
    return ''.join(pieces)
";

/// Iterating the target avoids repeated Unicode indexing while constructing paired mappings.
pub(crate) const MAKETRANS_SOURCE: &str = r"
def maketrans(x, y, z, has_y, len_, ord_, iter_, next_, isinstance_, int_, str_, dict_, type_error, value_error):
    result = {}
    if has_y:
        if not isinstance_(x, str_):
            raise type_error('first maketrans argument must be a string if there is a second argument')
        if len_(x) != len_(y):
            raise value_error('the first two maketrans arguments must have equal length')
        target = iter_(y)
        for character in x:
            result[ord_(character)] = ord_(next_(target))
        for character in z:
            result[ord_(character)] = None
    else:
        if not isinstance_(x, dict_):
            raise type_error('if you give only one argument to maketrans it must be a dict')
        for key, value in x.items():
            if isinstance_(key, str_):
                if len_(key) != 1:
                    raise value_error('string keys in translate table must be of length 1')
                key = ord_(key)
            elif not isinstance_(key, int_):
                raise type_error('keys in translate table must be strings or integers')
            result[key] = value
    return result
";
