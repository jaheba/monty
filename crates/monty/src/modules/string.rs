//! ASCII character classes and word capitalization from Python's `string` module.

use crate::{
    args::{ArgValues, FromArgs},
    bytecode::{CallResult, VM},
    exception_private::RunResult,
    frozen::FrozenFunction,
    heap::{HeapData, HeapId},
    intern::StaticStrings,
    modules::ModuleFunctions,
    types::{Module, str::allocate_string},
    value::Value,
};

const LOWERCASE: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPERCASE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
const PUNCTUATION: &str = r##"!"#$%&'()*+,-./:;<=>?@[\]^_`{|}~"##;
const WHITESPACE: &str = " \t\n\r\x0b\x0c";

/// Callable identities persisted through the module-function enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::Display, serde::Serialize, serde::Deserialize)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum StringFunctions {
    Capwords,
}

/// Creates the constants in CPython's character order and the capitalization helper.
pub fn create_module(vm: &mut VM<'_>) -> HeapId {
    let mut module = Module::new(StaticStrings::StringAttr, vm.interns);
    for (name, text) in [
        (StaticStrings::AsciiLowercase, LOWERCASE),
        (StaticStrings::AsciiUppercase, UPPERCASE),
        (
            StaticStrings::AsciiLetters,
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ",
        ),
        (StaticStrings::Digits, DIGITS),
        (StaticStrings::Hexdigits, "0123456789abcdefABCDEF"),
        (StaticStrings::Octdigits, "01234567"),
        (StaticStrings::Punctuation, PUNCTUATION),
        (StaticStrings::Whitespace, WHITESPACE),
    ] {
        module.set_attr(name, allocate_string(text, vm.heap), vm);
    }
    let printable = format!("{DIGITS}{LOWERCASE}{UPPERCASE}{PUNCTUATION}{WHITESPACE}");
    module.set_attr(StaticStrings::Printable, allocate_string(printable, vm.heap), vm);
    module.set_attr(
        StaticStrings::Capwords,
        Value::ModuleFunction(ModuleFunctions::String(StringFunctions::Capwords)),
        vm,
    );
    vm.heap.allocate(HeapData::Module(Box::new(module)))
}

/// Binds the public defaults before transferring both values into the frozen frame.
pub(super) fn call(vm: &mut VM<'_>, function: StringFunctions, args: ArgValues) -> RunResult<CallResult> {
    match function {
        StringFunctions::Capwords => {
            let CapwordsArgs { s, sep } = CapwordsArgs::from_args(args, vm)?;
            vm.call_frozen_exact(FrozenFunction::StringCapwords, [s, sep])
        }
    }
}

/// The Python signature accepts custom operands whose string methods may suspend.
#[derive(FromArgs)]
#[from_args(name = "capwords", style = def)]
struct CapwordsArgs {
    s: Value,
    #[from_args(default = Value::None)]
    sep: Value,
}

/// The callback loop runs in ordinary VM frames rather than synchronous native reentry.
pub(crate) const CAPWORDS_SOURCE: &str = r"
def capwords(s, sep):
    return (sep or ' ').join(word.capitalize() for word in s.split(sep))
";
