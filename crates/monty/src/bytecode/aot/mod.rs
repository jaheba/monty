//! Type-informed compilation. Hints select guarded implementations, never trusted layouts.

mod hints;
#[cfg(monty_native)]
pub(crate) mod ir;

#[cfg(monty_native)]
use std::sync::OnceLock;

pub(crate) use hints::{BuiltinHint, FunctionHints, TypeHint};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Untyped code pays for one optional pointer; only hinted functions own caches.
#[derive(Debug)]
pub(crate) struct TypedCode {
    pub hints: FunctionHints,
    #[cfg(monty_native)]
    pub plan: OnceLock<Option<Box<ir::Plan>>>,
    #[cfg(monty_native)]
    pub artifact: OnceLock<Option<Box<backend::Artifact>>>,
}

impl TypedCode {
    pub fn new(hints: FunctionHints) -> Self {
        Self {
            hints,
            #[cfg(monty_native)]
            plan: OnceLock::new(),
            #[cfg(monty_native)]
            artifact: OnceLock::new(),
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
        // Preserve Code.H's existing hints payload; caches contain process-local addresses.
        self.hints.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for TypedCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        FunctionHints::deserialize(deserializer).map(Self::new)
    }
}

#[cfg(monty_native)]
pub(crate) mod abi;
#[cfg(monty_native)]
pub(crate) mod backend;

use std::cell::Cell;

/// Selects how future code objects retain their offline-compiled templates.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Copied,
    #[default]
    Shared,
}

/// Diagnostic coverage of the guarded typed compiler on the current thread.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    pub compilations: u64,
    pub code_bytes: u64,
    pub entries: u64,
    pub returns: u64,
    pub fallbacks: u64,
    pub instructions: u64,
    pub fast_calls: u64,
    pub shared_compilations: u64,
}

thread_local! {
    static BACKEND: Cell<Backend> = const { Cell::new(Backend::Shared) };
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static STATS: Cell<Stats> = const { Cell::new(Stats {compilations:0,code_bytes:0,entries:0,returns:0,fallbacks:0,instructions:0,fast_calls:0,shared_compilations:0}) };
}

/// Selects guarded typed execution for subsequent runs on this thread.
pub fn set_enabled(enabled: bool) {
    ENABLED.with(|value| value.set(enabled));
}
/// Selects templates for code objects compiled after this call. Existing caches remain valid.
pub fn set_backend(backend: Backend) {
    BACKEND.with(|value| value.set(backend));
}
#[cfg(monty_native)]
pub(crate) fn backend() -> Backend {
    BACKEND.with(Cell::get)
}
/// Returns compilation and execution counters without resetting them.
pub fn stats() -> Stats {
    STATS.with(Cell::get)
}
/// Clears counters without discarding compiled artifacts.
pub fn reset_stats() {
    STATS.with(|value| value.set(Stats::default()));
}
#[cfg(monty_native)]
pub(crate) fn enabled() -> bool {
    ENABLED.with(Cell::get)
}
#[cfg(monty_native)]
pub(crate) fn record(f: impl FnOnce(&mut Stats)) {
    STATS.with(|value| {
        let mut stats = value.get();
        f(&mut stats);
        value.set(stats);
    });
}
