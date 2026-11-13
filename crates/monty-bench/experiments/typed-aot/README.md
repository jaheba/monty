# Typed native execution prototype

Experimental guarded compilation for typed Python, prepared on branch `typed-aot-study`. The eight-hour session is recorded in
[WORKLOG.md](WORKLOG.md), with its start and deadline in [session.json](session.json).

Annotations select implementations; runtime guards establish their preconditions. A disagreement uses ordinary
Python execution. The prototype does not turn annotations into new runtime type errors.

## Execution model

The optional `experimental-aot` feature preserves bounded annotation hints and lowers eligible function bodies
into typed values and explicit control flow from their normal bytecode. Local loads and stores become value bindings within each block;
parallel copies connect blocks. Arithmetic operates on unboxed values with overflow and domain checks.

Type lowering happens before the function executes. Native linking is currently lazy at its first eligible
entry, without profiling or a warmup threshold. Code and type-analysis caches are shared by subsequent calls.
This is not an offline native object format or a persistent compilation cache.
The shared backend executes a typed operation graph through Rust templates compiled with Monty.
Its speedups come from unboxing and removing VM stack/local traffic. Whole-function machine-code generation
and cross-operation native register allocation remain future work. The copied backend copies template bodies
and links their operand records; it does not patch machine operands into per-program instructions.

The ordinary VM frame retains its original inputs throughout a speculative invocation. Eligible bodies execute
no Python-visible effects. Successful execution returns through Monty's existing return handler. A failed guard
can therefore restart ordinary execution from the unchanged function entry. Spent execution time is not refunded.

Regular resource checkpoints keep unboxed values in the kernel. Original arguments remain VM roots; native
sequence buffers contain copied scalars, with no borrowed heap pointers held across GC. Executable mappings
carry an allocator-accounting lease and a bounded process-wide mapping reservation.

Suspending and effectful functions currently execute in the interpreter. Their typed pure callees can run before
and after suspension. Snapshots retain hints and bytecode, while native addresses and caches are rebuilt locally.

## Current coverage

- Exact primitive integers, floats, booleans and `None`, with scalar branches and loops.
- Checked integer arithmetic, guarded mixed numeric conversions and ordinary exception fallback.
- Builtin range loops, including private cursor state and immutable range aliases.
- Read-only homogeneous lists/tuples, scalar indexing and unpacking, and sequence length.
- String-keyed dictionary reads returning scalars; custom equality and missing keys fall back.
- Possibly unbound scalar locals, with guards preserving `UnboundLocalError`.
- Homogeneous scalar tuples constructed at return or immediately unpacked for parallel assignment.
- Read-only list/tuple iteration, including nested loops, break/continue and ordinary empty-loop behavior.

This list describes optimization eligibility. Unsupported programs keep the interpreter's existing behavior.
Union support is currently limited to matching list/tuple element types. General calls, mutation, attributes,
exception handlers and arbitrary nested containers remain outside this tier. Unicode optimization is excluded
from this session because the user has separate work on it.

Inputs up to 4096 elements use a lazily validated scalar buffer. Larger inputs use checked reads of only the
accessed elements. Direct sequence loops use the same views. Finite scalar leaf calls can skip a physical VM
frame after exact argument and recursion checks. Tiny cold functions still need a lower setup cost.

## Controls

A build of `monty-runtime` with `experimental-aot` accepts `--aot`. Standalone mode runs the existing type checker
before execution and rejects diagnostics. Protocol workers retain the host's `Configure.type_check` setting, and explicitly propagate
the execution mode to their worker thread. `--native-stats` prints separate counters on stderr.

The interpreter crate exposes the experimental thread-local controls as `monty::experimental_aot` when the
feature is enabled. Embedders arrange validation separately. These controls are experimental and are not exposed
by the published Python/JavaScript bindings.

## Combined execution

The combined prototype uses threaded dispatch for ordinary Python and shared typed templates for eligible
functions. It keeps source-instruction fuel separate from the bounded Rust handler chain. Native checkpoints
replenish fuel without extending that chain. The match-dispatch prototype remains available for comparison.
Current source selects threaded dispatch with the independent `experimental-threaded` Cargo feature;
`experimental-aot` alone retains match dispatch. Earlier frozen comparisons used separate source worktrees.

`Backend::Shared` is the default: program-specific patch records call statically linked Rust templates, without
per-plan executable mappings. `--aot-copied` retains copied code as an explicit comparison. Both backends use
the same typed IR and runtime guards. Changing the backend affects newly linked artifacts; cached artifacts
retain their backend.

Typed hints and boxed caches occupy one optional metadata pointer in `Code`. Its serialized `H` payload
remains hints alone. Unsupported bodies are rejected before CFG analysis; wholly untyped functions do not
allocate typed metadata.

## Final candidate

The final source supports both dispatchers in one tree. `experimental-aot` uses match dispatch;
`experimental-aot,experimental-threaded` selects the combined experiment. Both remain opt-in.
The default AOT backend uses shared templates. The final source is based on commit
`9a9f8784bf6cb36b09f183b53d7d829bcd807bdf`, without changes from the user's dirty checkout.

[guarded-confirmation/report.html](guarded-confirmation/report.html) has fifteen samples, serial execution,
three warmups and fresh application compilation per request. Validation is disabled in this comparison.

| Repeated request          | No-feature interpreter | Final match AOT | Final combined AOT | CPython 3.15 |
| ------------------------- | ---------------------: | --------------: | -----------------: | -----------: |
| Spectral norm             |                 305 ms |          253 ms |             240 ms |      66.8 ms |
| Sequence unpack           |                 312 ms |         6.73 ms |            6.78 ms |      17.4 ms |
| Policy scoring, 200 rows  |                0.54 ms |         0.50 ms |            0.51 ms |      0.55 ms |
| Policy scoring, 5000 rows |                7.66 ms |         5.98 ms |            5.95 ms |      3.97 ms |
| Portfolio exposure        |                4.35 ms |         1.33 ms |            1.34 ms |      1.57 ms |
| Long dictionary keys      |                0.38 ms |         0.24 ms |            0.25 ms |      0.29 ms |

Policy, exposure and long-key scenarios are newly written probes, not stock corpus entries. Long keys expose
an intermediate prototype defect: repeated reads rehashed a 36 KiB string and took 25.0 ms. The final helper
uses Monty's existing cached hash and identity shortcut; string/Unicode implementation files were not changed.

The [targeted fallback recheck](fallback-confirmation/report.html) still finds roughly 3–10% overhead on seven
previously regressing untyped workloads. For example, portfolio risk is 56.8/58.5 ms and Pystone is 283/298 ms
for no-feature/final match AOT. An entry-check reorder improved some cases but did not eliminate this cost.
The prior full-corpus match run found 15 regression classifications and no improvements on mostly untyped
sources, with unchanged compatibility. That full run predates the final hash/entry fixes; the targeted recheck
uses the final binary. There is no claim of an overall speedup or of a zero-cost fallback.

[guarded-timings.csv](guarded-timings.csv) records fifteen alternating 25 ms micro samples. Fresh tiny code is
5.83/6.99 µs interpreted/shared AOT; a fresh 1000-iteration loop is 119.6/31.4 µs. Retained execution of that
loop is 111.6/20.8 µs. Fresh 20-iteration loops regress from 9.75 to 11.55 µs; 100 iterations improve from
18.92 to 13.28 µs. These exclude validation. Cold checked submissions (15 samples, fresh workers and varied filename/source) took policy_scoring_short 13.68/13.53 ms; policy_scoring_batch 21.13/18.93 ms; portfolio_exposure 17.39/14.11 ms; dictionary_long_keys 11.79/11.88 ms. These are interpreter/shared Feed medians; startup/readiness/reset are excluded. Validation dominates these short cold requests. Full provenance is in `guarded-cold-validation/results.json`.

Final executables and exact source patches are frozen in `guarded-match-artifacts/` and
`guarded-combined-artifacts/`. Match SHA256 is `e57f05511b42b70a1d18967ac3c75c5b96e4c78f86c315c47ef77071a5044761`;
combined is `9adfb37553b3233adbd8f82c7a3e2147eb64b4f86515f607a4647ac77a1cd882`.
The match build adds 166320 bytes to the full CLI; the combined build adds 265936 bytes in this release profile.
The compiled executables stay in the isolated study; the persistent bundle contains source patches and metadata.

Whole-function machine-code emission, broader calls, nested containers and redundant guarded-read elimination
remain follow-up work. The present result is a measured typed compilation tier with guarded replay and normal
suspension boundaries, not a production-ready general Python compiler.

## Earlier application measurements

[budget-confirmation/report.html](budget-confirmation/report.html) contains fifteen timing samples, three
separate RSS samples and serial execution. Frozen binaries and source are in `combined-budget-artifacts/`
and `compact-artifacts/`. Repeated requests reuse a worker but compile a fresh application session each time.
Validation is disabled in this Flying Circus run.

| Repeated request          | Threaded interpreter | Combined shared AOT | Match shared AOT | CPython 3.15 |
| ------------------------- | -------------------: | ------------------: | ---------------: | -----------: |
| Spectral norm             |               318 ms |              239 ms |           251 ms |      66.7 ms |
| Sequence unpack           |               315 ms |             6.91 ms |          6.88 ms |      17.5 ms |
| Policy scoring, 200 rows  |              0.56 ms |             0.52 ms |          0.52 ms |      0.56 ms |
| Policy scoring, 5000 rows |              8.06 ms |             6.54 ms |          6.62 ms |      4.18 ms |
| Portfolio exposure        |              4.10 ms |             1.41 ms |          1.43 ms |      1.58 ms |
| Capacity planning         |              28.9 ms |             28.8 ms |          29.1 ms |      8.75 ms |
| Ticket search             |              26.7 ms |             26.9 ms |          26.5 ms |      6.97 ms |
| Float object workload     |               216 ms |              216 ms |           214 ms |      49.8 ms |

Policy scoring and portfolio exposure are newly written application-style workloads. The first five sources
pass Monty's checker. Capacity, ticket and float are fallback controls; the float workload fails checking.
No overall speedup follows from this selected cohort. The earlier combined version took 271 ms on spectral
norm before its source fuel was separated from handler depth.

[shared-threaded-confirmation/report.html](shared-threaded-confirmation/report.html) compares copied/shared
backends with both dispatchers. Shared unpacking took 6.94 ms versus 12.8 ms copied; spectral norm and portfolio
were similar. This is the reason shared templates became the default.

The full [Flying Circus screen](full-screen/report.html) found no compatibility changes: 121 inconclusive
workloads and the same three baseline failures in both modes (generator iteration in pidigits/nqueens and
function attributes in regex_effbot). Three samples make it screening evidence, not proof of no regression.
The subsequent [full comparison](full-confirmation/report.html) used fifteen samples. Threaded-only had
23 regression and 7 improvement classifications; threaded with AOT had 25 and 6, across scenarios/metrics.
Examples: standard portfolio risk went from 57.0 to 64.5 ms, Mandelbrot from 15.4 to 18.6 ms, while medium
CSV cleanup improved from 662 to 624 ms. Compatibility remained unchanged. Threaded dispatch therefore
remains an independent opt-in feature. The separate full match-only AOT comparison is in `full-match-confirmation/`; it predates the final hash/entry fixes.
Annotated workload generation verifies unchanged executable AST after erasing annotations;
[workloads-manifest.json](workloads-manifest.json) records source hashes.

## Checked submissions

`cold-varied-validation/results.json` and `warm-varied-validation/results.json` each contain fifteen samples
per mode, with validation enabled/disabled and interpreter/copied/shared backends. Each sample uses a distinct
filename and source comment. Times include Configure and Feed, including compilation and requested checking;
process startup, readiness and Reset are excluded. Cold uses a new worker per sample, warm reuses workers.

| Validation enabled, Feed median | Cold interpreter | Cold shared AOT | Warm interpreter | Warm shared AOT |
| ------------------------------- | ---------------: | --------------: | ---------------: | --------------: |
| Policy scoring, 200 rows        |         13.55 ms |        13.24 ms |          1.31 ms |         1.22 ms |
| Policy scoring, 5000 rows       |         21.09 ms |        19.46 ms |          9.05 ms |         7.34 ms |
| Portfolio exposure              |         16.98 ms |        14.25 ms |          5.15 ms |         2.27 ms |
| Spectral norm                   |        329.40 ms |       250.94 ms |        318.10 ms |       238.24 ms |
| Sequence unpack                 |        336.49 ms |        30.59 ms |        324.28 ms |        18.57 ms |

Cold validation dominates short requests. Varying source also costs more than repeatedly checking identical
source: the earlier `validated-confirmation/` run had warm checker state and repeated the same filename/code.
These boundaries must remain distinct.

## Setup and retained execution

[compact-timings.csv](compact-timings.csv) has fifteen alternating 25 ms samples per mode and case. Fresh
includes parsing, compilation, execution and destruction; retained reuses compiled code. Type checking is
excluded. The executable and hashes are in `compact-example-artifacts/`.

| Case                          | Fresh interpreter | Fresh shared AOT | Retained interpreter | Retained shared AOT |
| ----------------------------- | ----------------: | ---------------: | -------------------: | ------------------: |
| Tiny expression               |           5.82 µs |          6.90 µs |              0.60 µs |             0.56 µs |
| Integer loop, 20 iterations   |           9.94 µs |         11.19 µs |              2.91 µs |             1.05 µs |
| Integer loop, 100 iterations  |          19.27 µs |         13.14 µs |             11.97 µs |             2.83 µs |
| Integer loop, 1000 iterations |         122.02 µs |         32.65 µs |            114.63 µs |            22.43 µs |
| Range loop, 1000 iterations   |          78.30 µs |         23.83 µs |             71.15 µs |            13.35 µs |
| Float loop, 1000 iterations   |         101.67 µs |         30.78 µs |             94.68 µs |            20.31 µs |
| Frequent tiny helper calls    |         189.28 µs |        153.13 µs |            180.95 µs |           145.30 µs |

The tiny fresh copied case took 17.88 µs. Shared templates remove most mapping setup, but very short fresh
programs can still regress. No profiling warmup or hardcoded workload pattern is required.

## Validation status

The unified source passes 909 Rust tests (2 ignored), 22 focused AOT tests in match mode, 9 dispatch tests
without AOT, 9 fused/threaded tests, 642 Monty fixtures and six focused AOT worker tests with memory-model
checks. Clippy and the no-feature check pass. Rust 1.96 passes 31 dispatch/AOT tests using the isolated,
one-line baseline JSON-cache shim described in `msrv-baseline-compat.patch`.

The full worker file in normal mode passes 95 tests and fails two exact memory-byte expectations. Both fail
in the unchanged baseline too (89 passed, with no six AOT tests). Memory-model mode passes 91 and fails six;
the identical six fail in the unchanged baseline (85 passed). Four additional failures are reference-count
assertions. `subprocess-baseline-comparison.json` and the raw logs preserve the distinction. These are not
passing full worker suites and no test tolerance was loosened for the prototype.

The initial stress run compared 100000 generated programs without a mismatch. The bounded generator then
completed one million cases in match mode, two one-million-case seeds in the combined liveness build, and
one million cases in the unified combined build, and one million cases per dispatcher in the final source. The final seeds include heap-string keys. It draws arithmetic expressions, scalar calls,
branches, range/sequence loops, parallel assignments, wrong hints and callback-sensitive dictionary keys
from a bounded grammar; it is not arbitrary-Python coverage. Logs, binary hashes and status are under
`fuzz-runs/`. A prior generator resource failure is retained there.

`unified-cross-feature-snapshots.json` verifies current suspended dumps from each typed backend restored in
either backend and the frozen pre-AOT interpreter, with all six returning 22.0. Native addresses are not serialized.
Current WASI fallback and x86_64 Linux compile checks pass. Current Windows x86_64/ARM64 templates compile
with Rust 1.96, pass relocation/tail checks and link into SDK-free PE fixtures with 46 requested exports.
The complete Windows runtime needs its normal SDK; Windows/x86 native execution is unverified on this host.
Native templates have not been run under Miri. The [audit notes](AUDIT.md) cover the local boundary review.

Earlier measurements remain in `initial-timings.csv`, `sequence-confirmation/`, `threaded-comparison/` and
`fastcall-confirmation/`, with their own frozen sources. They describe historical stages, not the final source.
