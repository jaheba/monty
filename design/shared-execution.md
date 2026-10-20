# Shared execution audit

Starting point: PR #1088, `f7ddd917acbd5893b9cabdbd5025ab14fdc4fc02`.

## Current implementations

| Mechanism          | Python                                                                         | MontyScript                                                                         |
| ------------------ | ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------- |
| Operand storage    | `bytecode/vm/mod.rs`: `Vec<Value>`                                             | `vm/mod.rs`: `Vec<JsValue>`, interleaved locals and operands                        |
| Frames             | Borrowed `Code`, cached instruction slice, current frame separate from callers | `Vec<Frame>`, `Arc<Function>`, constructor and promise completion state             |
| Instruction offset | Body-relative `usize` in `CallFrame`                                           | Body-relative `usize` in `Frame`                                                    |
| Calls              | `bytecode/vm/call.rs`, Python binding and recursion tokens                     | `vm/call.rs`, constructors, receivers and closure cells                             |
| Exceptions         | Exception tables, Python exception stack and traceback construction            | `handle_throw`, pending thrown value, completion delivery; compiler inlines finally |
| Suspension         | VM snapshot plus Python scheduler state                                        | `vm/host.rs`: `ParkedVm`, serializable frame completions and queued jobs            |
| Limits             | Dispatch countdown and builtin checks                                          | Persistent dispatch ticks, `checkpoint`, native/reentrancy depth and builtin checks |
| Heap               | Python payloads over `monty-heap`                                              | `JsHeapData` over `monty-heap`                                                      |
| Dumps              | `MONTY` header, version 13                                                     | `MONTYJS` header, version 2; idle and suspended variants                            |

Python's countdown currently checks every 256 dispatch iterations and restarts at `run` entry.
JavaScript checks every 255 steps and parks its tick count across host suspension.
Unifying those counters would change accounting behavior; this extraction leaves both intact.

JavaScript frame completions include async settlement, thenables and finally callbacks.
Python coroutine scheduling cannot replace these completions.
`vm/feed.rs` drains JavaScript jobs separately from execution of the module body.
`vm/host.rs` explicitly blocks host suspension from callbacks whose continuation is on the Rust stack.
Sharing a call loop before converting those callbacks to explicit pending operations would retain this limitation.

## Shared operand fetching

`monty-execution::fetch` owns fixed-width operand fetching and offset advancement.
Both interpreters call the same monomorphized implementation without a language selector.
The frame still owns its offset and code lifetime; opcode decoding and little-endian operand interpretation remain local.
A failed read does not advance the offset.
Compiled operands are trusted; both callers treat a truncated operand as an internal invariant violation.
No heap reference, resource token or pending operation is transferred by this helper.

Operand fetching and stack cleanup change neither bytecode encoding nor serialized state.
The separate string change below bumps the JavaScript dump version.
These helpers do not alter host frames or require cross-language value conversion.

## Shared stack cleanup

`monty-execution::release_stack` releases one frame or exception-handler region while retaining its caller prefix
and allocation capacity.
The language supplies cleanup for each owned value; a const parameter preserves top-first or bottom-first order.
Python uses it for completed frames and handler unwinding; JavaScript uses it for returns, throws and aborted frames.
The shared mechanism cannot suspend or call interpreted code during cleanup.
Frame namespaces, constructor state, promise completions and recursion tokens remain language-owned.

## Frame and operation continuations

Retain separate dispatch loops and value representations.
Before extracting frame lifecycle, describe each frame exit as return, throw, yield or host suspension, with language-owned
completion state and explicit ownership of the returned value.
An operation that calls interpreted code needs a saved continuation containing its operands and next action.
On return, take that continuation before executing the next action, so host resume cannot repeat it.
On throw or abort, release its operands through the language's cleanup implementation.
Serialize pending continuations alongside frames; do not serialize Rust stack continuations.

A shared driver requires both implementations to report these outcomes without nested unbounded `run` calls.
Do not introduce a trait requiring identical argument binding, exception values or constructor behavior.

## MontyScript semantics

The UTF-8 string change is separate from the execution extraction.
`string.rs` stores valid UTF-8 and exposes scalar indexing; parser literals, compiler keys and templates, JSON escapes,
string methods, iterators, JSX, concatenation and regex capture offsets follow that representation.
JavaScript dump version 2 rejects version 1 because persisted strings and their hash semantics changed.
Python dumps and host protocol messages have no format changes.

Classes, generators, timers, object descriptors and TSX were present in the PR and remain available.
TypeScript syntax is erased through oxc; this does not provide type checking.
TSX rendering remains a builtin facility separate from execution components.
The supported subset and promise scheduling contract are in `docs/limitations/javascript.md`.

## Remaining boundary

Calls and semantic-operation continuations are still interpreter-specific.
Host suspension from JavaScript builtins that recursively invoke interpreted callbacks remains unsupported and raises
an explicit error; the extraction does not claim to make these operations resumable.
No unsafe heap code was changed.
Scalar indexing scans UTF-8, and regex execution converts to UTF-16 for the existing executor; these have different
costs from the PR's direct code-unit indexing and need string-heavy profiling before further optimization.

Python's host `abort` is a forced stop: it bypasses guest handlers and `finally` to enforce host limits.
An exception delivered with ordinary host resumption runs `finally`.
MontyScript's existing `abort` injects an exception and runs `finally`; its resource-error termination can discard frames.
These behaviors remain language-specific rather than being hidden behind a shared cancellation operation.

See [the benchmark results](benchmarks/README.md) for measured costs and the reproducible harness.

## Validation

- `make lint-rs`: workspace Clippy, all features and import checks pass.
- Python interpreter tests with `memory-model-checks` pass; the additional abort/finally regression passes.
- Python worker: 100 subprocess tests pass; filesystem: 264 tests pass.
- MontyScript worker: 21 subprocess tests pass, including graceful scalar-string memory rejection.
- Node 25 differential suite: 27 cases pass with unreachable-heap checks.
- Focused GC-every-allocation tests: six continuation/cleanup tests and five Unicode/snapshot contract tests pass.
- Native host binding regression selection: 176 tests pass; the expanded Unicode selection passes ten tests.
- Native addon build, TypeScript checks, changed-file TypeScript lint, Rust formatting and Markdown checks pass.

The 100,000-deep structure stress test passes with normal GC.
Running that stress case with GC forced on every allocation was stopped because it makes construction quadratic;
GC-every-allocation validation is confined to the focused tests above.
Browser conversion unit tests run, but a full browser/WASM component end-to-end suite was not run.

The Python crate README retains its existing two-space list continuation indentation: mdformat’s MkDocs plugin
rewrites it to four spaces, which Rust 1.99 Clippy rejects when the README is included as crate documentation.
