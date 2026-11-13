//! Type-informed compilation. Hints select guarded implementations, never trusted layouts.

mod hints;
pub(crate) mod ir;

use std::{cell::Cell, sync::OnceLock};

pub(crate) use hints::{BuiltinHint, FunctionHints, TypeHint};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Untyped code pays for one optional pointer; only hinted functions own caches.
#[derive(Debug)]
pub(crate) struct TypedCode {
    pub hints: FunctionHints,
    pub plan: OnceLock<Option<Box<ir::Plan>>>,
    pub compiled: OnceLock<Option<Box<execute::Compiled>>>,
}

impl TypedCode {
    pub fn new(hints: FunctionHints) -> Self {
        Self {
            hints,
            plan: OnceLock::new(),
            compiled: OnceLock::new(),
        }
    }
}

impl Clone for TypedCode {
    fn clone(&self) -> Self {
        Self::new(self.hints.clone())
    }
}

impl Serialize for TypedCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Preserve Code.H's existing hints payload; compiled instructions are reconstructed when a snapshot is loaded.
        self.hints.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TypedCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        FunctionHints::deserialize(deserializer).map(Self::new)
    }
}

mod compile;
pub(crate) mod execute;

/// Diagnostic coverage of the guarded typed compiler on the current thread.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    pub compilations: u64,
    pub entries: u64,
    pub returns: u64,
    pub fallbacks: u64,
    pub instructions: u64,
}

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static STATS: Cell<Stats> = const { Cell::new(Stats {compilations:0,entries:0,returns:0,fallbacks:0,instructions:0}) };
}

/// Selects guarded typed execution for subsequent runs on this thread.
pub fn set_enabled(enabled: bool) {
    ENABLED.with(|value| value.set(enabled));
}
/// Returns compilation and execution counters without resetting them.
pub fn stats() -> Stats {
    STATS.with(Cell::get)
}
/// Clears counters without discarding compiled instructions.
pub fn reset_stats() {
    STATS.with(|value| value.set(Stats::default()));
}
pub(crate) fn enabled() -> bool {
    ENABLED.with(Cell::get)
}
pub(crate) fn record(f: impl FnOnce(&mut Stats)) {
    STATS.with(|value| {
        let mut stats = value.get();
        f(&mut stats);
        value.set(stats);
    });
}
