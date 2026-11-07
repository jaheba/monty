//! Line indentation helpers implemented in ordinary, resource-limited VM frames.

use monty_types::{BuiltinsFunctions, StringRepr};

use crate::{
    args::{ArgValues, FromArgs},
    builtins::Builtins,
    bytecode::{CallResult, VM},
    exception_private::{ExcType, RunResult},
    frozen::FrozenFunction,
    heap::{HeapData, HeapId},
    intern::StaticStrings,
    modules::ModuleFunctions,
    types::{Module, PyTrait, str::allocate_string},
    value::Value,
};

/// Identities retained by imported helpers and snapshots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, serde::Serialize, serde::Deserialize)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum TextwrapFunctions {
    Dedent,
    Indent,
}

/// Exposes indentation helpers without the paragraph-wrapping API.
pub fn create_module(vm: &mut VM<'_>) -> HeapId {
    let mut module = Module::new(StaticStrings::Textwrap, vm.interns);
    for (name, function) in [
        (StaticStrings::Dedent, TextwrapFunctions::Dedent),
        (StaticStrings::Indent, TextwrapFunctions::Indent),
    ] {
        module.set_attr(name, Value::ModuleFunction(ModuleFunctions::Textwrap(function)), vm);
    }
    vm.heap.allocate(HeapData::Module(Box::new(module)))
}

/// Binds public signatures before handing owned values to suspendable implementations.
pub(super) fn call(vm: &mut VM<'_>, function: TextwrapFunctions, args: ArgValues) -> RunResult<CallResult> {
    match function {
        TextwrapFunctions::Dedent => {
            let DedentArgs { text } = DedentArgs::from_args(args, vm)?;
            let type_name = text.py_type(vm).dunder_name(vm.heap, vm.interns);
            let type_name = allocate_string(format!("{}", StringRepr(&type_name)), vm.heap);
            vm.call_frozen_exact(
                FrozenFunction::TextwrapDedent,
                [
                    text,
                    Value::Builtin(Builtins::Function(BuiltinsFunctions::Len)),
                    Value::Builtin(Builtins::ExcType(ExcType::AttributeError)),
                    Value::Builtin(Builtins::ExcType(ExcType::TypeError)),
                    type_name,
                ],
            )
        }
        TextwrapFunctions::Indent => {
            let IndentArgs {
                text,
                prefix,
                predicate,
            } = IndentArgs::from_args(args, vm)?;
            vm.call_frozen_exact(FrozenFunction::TextwrapIndent, [text, prefix, predicate])
        }
    }
}

/// `dedent` accepts its text argument positionally or by name.
#[derive(FromArgs)]
#[from_args(name = "dedent", style = def)]
struct DedentArgs {
    text: Value,
}

/// A predicate can suspend and is only invoked for lines actually present.
#[derive(FromArgs)]
#[from_args(name = "indent", style = def)]
struct IndentArgs {
    text: Value,
    prefix: Value,
    #[from_args(default = Value::None)]
    predicate: Value,
}

/// Whitespace-only lines use Python's Unicode definition; indentation uses literal spaces and tabs.
pub(crate) const DEDENT_SOURCE: &str = r"
def dedent(text, len_, attribute_error, type_error, type_name):
    try:
        lines = text.split('\n')
    except (attribute_error, type_error):
        raise type_error('expected str object, not ' + type_name) from None

    margin = None
    for line in lines:
        if line and not line.isspace():
            prefix = line[:len_(line) - len_(line.lstrip(' \t'))]
            if margin is None:
                margin = prefix
            else:
                limit = len_(margin)
                shared = 0
                for character in prefix:
                    if shared == limit or margin[shared] != character:
                        break
                    shared += 1
                margin = margin[:shared]

    width = 0 if margin is None else len_(margin)
    return '\n'.join('' if line.isspace() else line[width:] for line in lines)
";

/// Prefixes stay separate until join, preserving delayed type errors and callback order.
pub(crate) const INDENT_SOURCE: &str = r"
def indent(text, prefix, predicate):
    result = []
    for line in text.splitlines(True):
        selected = not line.isspace() if predicate is None else predicate(line)
        if selected:
            result.append(prefix)
        result.append(line)
    return ''.join(result)
";
