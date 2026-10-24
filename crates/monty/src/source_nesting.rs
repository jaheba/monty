//! Pre-parse resource bounds for source passed to Ruff.
//!
//! Spam uses an independent lexer, so malformed or adversarial input is
//! rejected before Ruff can allocate parser-stack segments outside Monty's
//! sandbox allocator.

use ruff_python_parser::Mode;
use ruff_text_size::{TextRange, TextSize};
use spam::{Limits, MAX_SOURCE_BYTES, MAX_TOKENS, MAX_WORK_UNITS, check};

use crate::parse::MAX_NESTING_DEPTH;

/// Whether `source` can be handed to Ruff or the type checker without
/// exceeding Monty's parser-resource budget.
#[must_use]
pub fn source_within_nesting_bound(source: &str, source_scan_threshold: usize) -> bool {
    source.len() <= source_scan_threshold || nesting_bound_exceeded(source, Mode::Module, MAX_NESTING_DEPTH).is_none()
}

/// Returns the source range where Spam first exceeds the parser budget.
pub(crate) fn nesting_bound_exceeded(source: &str, _mode: Mode, limit: u16) -> Option<TextRange> {
    let limits = Limits {
        max_source_bytes: MAX_SOURCE_BYTES,
        // Existing bytecode limits report more specific errors for large flat
        // modules, so the parser preflight uses Spam's implementation ceiling.
        max_tokens: MAX_TOKENS,
        max_work_units: MAX_WORK_UNITS,
        max_nesting_depth: limit,
        max_indentation_depth: limit,
        // Monty's converter applies its own AST-depth budget. This preflight
        // is limited to source shapes that can recurse in Ruff itself.
        max_ast_depth: None,
        ..Limits::default()
    };
    let rejection = check(source, &limits).err()?;
    let start = TextSize::new(u32::try_from(rejection.range.start).unwrap_or(u32::MAX));
    let end = TextSize::new(u32::try_from(rejection.range.end).unwrap_or(u32::MAX));
    Some(TextRange::new(start, end))
}
