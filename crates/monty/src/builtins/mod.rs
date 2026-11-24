//! Python builtin functions, types, and exception constructors.
//!
//! This module provides the interpreter-native implementation of Python builtins.
//! Each builtin function has its own submodule for organization.

mod abs;
mod all;
mod any;
mod bin;
mod chr;
mod divmod;
mod enumerate;
pub(crate) mod filter;
mod format;
mod getattr;
mod hasattr;
mod hash;
mod hex;
mod id;
pub(crate) mod isinstance;
pub(crate) mod issubclass;
pub(crate) mod iter;
mod len;
mod locals;
pub(crate) mod map;
mod min_max; // min and max share implementation
pub(crate) use min_max::candidate_wins;
pub(crate) mod next;
pub(crate) mod object;
pub(crate) mod object_setattr;
mod oct;
pub(crate) mod open;
mod ord;
mod pow;
mod print;
mod repr;
mod reversed;
mod round;
mod setattr;
mod sorted;
mod sum;
pub(crate) mod type_;
mod zip;

use std::{fmt, fmt::Write, str::FromStr};

use crate::{
    args::ArgValues,
    bytecode::{CallResult, VM},
    exception_private::{ExcType, RunResult},
    types::Type,
    value::Value,
};

/// A builtin callable stored inline in a [`Value`]: a function or a native type.
///
/// Dumps name the variants, so `Function` and `Type` are the persistence
/// contract; [`DumpedBuiltins`] reads the shapes older builds wrote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(from = "DumpedBuiltins")]
pub(crate) enum Builtins {
    /// A builtin function like `print` or `len`.
    Function(BuiltinsFunctions),
    /// A native type like `list`, `int` or `ValueError`, which is also its constructor.
    Type(Type),
}

/// Every shape a stored [`Builtins`] has had: exception classes were a separate
/// `ExcType` variant, and `type` was a builtin function, before both joined the
/// native type table.
#[derive(serde::Deserialize)]
enum DumpedBuiltins {
    Function(BuiltinsFunctions),
    ExcType(ExcType),
    Type(Type),
}

impl From<DumpedBuiltins> for Builtins {
    fn from(dumped: DumpedBuiltins) -> Self {
        match dumped {
            DumpedBuiltins::Function(BuiltinsFunctions::Type) => Self::Type(Type::Type),
            DumpedBuiltins::Function(function) => Self::Function(function),
            DumpedBuiltins::ExcType(exc) => Self::Type(Type::from(exc)),
            DumpedBuiltins::Type(ty) => Self::Type(ty),
        }
    }
}

impl Builtins {
    /// Resolves builtin names that evaluate directly to singleton values.
    #[must_use]
    pub fn value_from_name(name: &str) -> Option<Value> {
        match name {
            "Ellipsis" => Some(Value::Ellipsis),
            "NotImplemented" => Some(Value::NotImplemented),
            _ => name.parse::<Self>().ok().map(Value::Builtin),
        }
    }

    /// Calls this builtin, allowing builtins that need host involvement to yield.
    ///
    /// Most builtins produce a [`CallResult::Value`]. `iter()` and `next()` can
    /// push frames for user methods; `open()` yields its host filesystem operation,
    /// and `next()` on an unread file yields the read that loads its buffer.
    pub fn call(self, vm: &mut VM<'_>, args: ArgValues) -> RunResult<CallResult> {
        match self {
            Self::Function(b) => b.call(vm, args),
            Self::Type(t) => t.call(vm, args),
        }
    }

    /// Writes the Python repr() string for this callable to a formatter.
    pub fn py_repr_fmt<W: Write>(self, f: &mut W) -> fmt::Result {
        match self {
            Self::Function(b) => write!(f, "<built-in function {b}>"),
            Self::Type(t) => write!(f, "<class '{t}'>"),
        }
    }

    /// Returns the type of this builtin.
    pub fn py_type(self) -> Type {
        match self {
            Self::Function(_) => Type::BuiltinFunction,
            Self::Type(_) => Type::Type,
        }
    }
}

impl FromStr for Builtins {
    type Err = ();

    /// Resolves a name in the builtins namespace. Types win over functions so
    /// `type` is the type object; `BuiltinsFunctions::Type` survives only as the
    /// bytecode operand class bodies call.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(t) = Type::from_builtin_name(s) {
            Ok(Self::Type(t))
        } else if let Some(b) = BUILTINS.resolve(s) {
            Ok(Self::Function(b))
        } else {
            Err(())
        }
    }
}

pub use monty_types::BuiltinsFunctions;
pub(crate) use type_::type_of_ref;

pub(crate) trait BuiltinsFunctionsExt: Sized {
    fn call(self, vm: &mut VM<'_>, args: ArgValues) -> RunResult<CallResult>;
}

type BuiltinHandler = fn(&mut VM<'_>, ArgValues) -> RunResult<CallResult>;

/// Indexed by the u8 builtin operand stored in bytecode.
pub(crate) struct BuiltinRegistry {
    handlers: [Option<BuiltinHandler>; u8::MAX as usize + 1],
}

impl BuiltinRegistry {
    pub(crate) const fn builtin(&mut self, id: BuiltinsFunctions, handler: BuiltinHandler) {
        let slot = &mut self.handlers[id as usize];
        assert!(slot.is_none(), "duplicate builtin registration");
        *slot = Some(handler);
    }

    fn resolve(&self, name: &str) -> Option<BuiltinsFunctions> {
        let id = BuiltinsFunctions::from_str(name).ok()?;
        self.handlers[id as usize].map(|_| id)
    }
}

static BUILTINS: BuiltinRegistry = {
    use BuiltinsFunctions as B;

    macro_rules! value {
        ($handler:path) => {
            |vm, args| $handler(vm, args).map(CallResult::Value)
        };
    }

    let mut registry = BuiltinRegistry {
        handlers: [None; u8::MAX as usize + 1],
    };
    registry.builtin(B::Abs, value!(abs::builtin_abs));
    registry.builtin(B::All, value!(all::builtin_all));
    registry.builtin(B::Any, value!(any::builtin_any));
    registry.builtin(B::Bin, value!(bin::builtin_bin));
    registry.builtin(B::Chr, value!(chr::builtin_chr));
    registry.builtin(B::Divmod, value!(divmod::builtin_divmod));
    registry.builtin(B::Enumerate, value!(enumerate::builtin_enumerate));
    registry.builtin(B::Filter, filter::builtin_filter);
    registry.builtin(B::Map, map::builtin_map);
    registry.builtin(B::Format, value!(format::builtin_format));
    registry.builtin(B::Getattr, getattr::builtin_getattr);
    registry.builtin(B::Hasattr, hasattr::builtin_hasattr);
    registry.builtin(B::Hash, value!(hash::builtin_hash));
    registry.builtin(B::Hex, value!(hex::builtin_hex));
    registry.builtin(B::Id, value!(id::builtin_id));
    registry.builtin(B::Isinstance, value!(isinstance::builtin_isinstance));
    registry.builtin(B::Issubclass, value!(issubclass::builtin_issubclass));
    registry.builtin(B::Len, value!(len::builtin_len));
    registry.builtin(B::Max, value!(min_max::builtin_max));
    registry.builtin(B::Min, value!(min_max::builtin_min));
    registry.builtin(B::Next, next::builtin_next);
    registry.builtin(B::Oct, value!(oct::builtin_oct));
    registry.builtin(B::Open, open::builtin_open);
    registry.builtin(B::Ord, value!(ord::builtin_ord));
    registry.builtin(B::Pow, value!(pow::builtin_pow));
    registry.builtin(B::Print, value!(print::builtin_print));
    registry.builtin(B::Repr, value!(repr::builtin_repr));
    registry.builtin(B::Reversed, value!(reversed::builtin_reversed));
    registry.builtin(B::Round, value!(round::builtin_round));
    registry.builtin(B::Setattr, value!(setattr::builtin_setattr));
    registry.builtin(B::ObjectSetattr, value!(object_setattr::builtin_object_setattr));
    registry.builtin(B::ObjectNew, value!(object::builtin_object_new));
    registry.builtin(B::ObjectInit, value!(object::builtin_object_init));
    registry.builtin(B::ObjectInitSubclass, value!(object::builtin_object_init_subclass));
    registry.builtin(B::TypeMro, value!(type_::builtin_type_mro));
    registry.builtin(B::Sorted, value!(sorted::builtin_sorted));
    registry.builtin(B::Sum, value!(sum::builtin_sum));
    registry.builtin(B::Type, value!(type_::builtin_type));
    registry.builtin(B::Zip, value!(zip::builtin_zip));
    registry.builtin(B::Locals, value!(locals::builtin_locals));

    crate::shards::register(&mut registry);

    // Host values and restored bytecode can refer to a builtin directly by ID.
    let mut next_id = Some(0u8);
    while let Some(id) = next_id {
        if B::from_repr(id).is_some() {
            assert!(registry.handlers[id as usize].is_some(), "missing builtin registration");
        }
        next_id = id.checked_add(1);
    }
    registry
};

impl BuiltinsFunctionsExt for BuiltinsFunctions {
    fn call(self, vm: &mut VM<'_>, args: ArgValues) -> RunResult<CallResult> {
        let handler = BUILTINS.handlers[self as usize].expect("builtin has a registered handler");
        handler(vm, args)
    }
}
