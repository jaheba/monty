//! Shards compiled into the interpreter.

pub(crate) mod dynamic_eval;

use crate::builtins::BuiltinRegistry;

pub(crate) const fn register(registry: &mut BuiltinRegistry) {
    dynamic_eval::register(registry);
}
