# Typed AOT work session

Started 2026-11-12 19:46:51 UTC; requested work period ends 2026-11-13 03:46:51 UTC.
The active goal records the same scope and constraints.

## Contract

- Validate typed submissions with Monty's existing type checker.
- Treat annotations as optimization hints, with runtime guards and ordinary Python fallback on disagreement.
- Keep source evaluation order, exceptions, resource limits, suspension, snapshots and ownership correct.
- Keep native addresses out of dumps. Invalid snapshots retain the documented memory-safety contract.
- Preserve the original dirty checkout at `/Users/jaheba/p/monty`.
- Work in `/private/tmp/monty-native-study/monty`, without commits or pushes.
- Do not edit `crates/monty/src/heap/` without explicit approval.
- No delegated agents are authorized by the current instructions.

## Starting point

The copy-and-patch prototype is feature gated by `experimental-native`.
It imports a tagged frame/global image and emits one offline Rust stencil per bytecode instruction.
It improves sustained scalar loops but frequently crosses back to the interpreter around unsupported operations.
All 877 Rust tests and 642 Monty fixtures passed before this session.
The initial source, including benchmark CLI controls, is saved in
`/private/tmp/monty-native-study/native-prototype-before-aot.patch` against HEAD
`9a9f8784bf6cb36b09f183b53d7d829bcd807bdf`.

The frozen native prototype CLI is
`../native-regions/flying-circus-artifacts/bin/monty` relative to this experiment directory.
Its SHA256 is `bfdfc25c707f9b92f6b10a741bec598364bf08fe6e041dbdbb745c8568a3c1d3`.
Baseline uses `--no-native`; candidate uses the default experimental native mode.
`--native-stats` is diagnostic only and is excluded from timed runs.
The mode is explicitly propagated to the protocol worker thread.

The sibling spam dependency is a source archive at `../spam` from tokspam revision
`24c3a27486cbff301fe5b9586ce725ebf5a829e3`, with package/library names adapted to this baseline's expected API.
The real `/Users/jaheba/p/spam` checkout is unchanged.
Builds use `CARGO_TARGET_DIR=/Users/jaheba/p/monty/target` and the release profile has fat LTO and one codegen unit.
Avoid running builds alongside benchmarks.

## Initial Flying Circus comparison

The full serial quick scan completed in 8m43s with three samples per workload and mode.
Three pre-existing failures match in both modes; no compatibility change was reported.
Timing classifications remain inconclusive at this sample count.
The complete report is `../native-regions/flying-circus-full/report.html`.
A 15-sample confirmation on eight frozen workloads completed in 3m40s; its report directory is
`../native-regions/flying-circus-confirmation` and its log is
`../native-regions/flying-circus-artifacts/confirmation.log`.
The relevant exec session was 53811 when launched.

## Implementation direction

1. Add an independent `experimental-aot` mode and preserve scalar parameter hints through parsing and preparation.
2. Lower supported typed functions into a small explicit-value control-flow representation before native emission.
   Infer scalar operations from hints, literals and dataflow; keep checked integer arithmetic.
3. Start with side-effect-free function bodies. Keep the ordinary VM frame installed and its original inputs owned.
   A guarded scalar kernel can return a scalar through the existing `ReturnValue` handler.
   A failed speculative kernel can use the unchanged interpreter frame when no Python-visible effects occurred.
4. Remove repeated tag tests and operand-stack operations inside proven typed code.
   Add pure builtin range loops, then guarded read-only list/dictionary access if practical.
5. Keep periodic resource and GC checkpoints. Use fallback for callbacks, suspension and unsupported effects until a
   precise resumable region boundary is implemented.
6. Investigate accounting executable memory so memory-limited sessions can safely use optimized code.
   The current native prototype conservatively disables native execution under `max_memory`.
7. Validate runtime type disagreements, big integers, aliases, exception locations, generators, suspension/dumps,
   deadlines and memory limits, then benchmark typed programs with validation cost clearly separated or included.

Useful code locations:

- `crates/monty/src/parse.rs`: `ParsedParam`, `ParsedSignature`, `parse_params_with_defaults`.
- `crates/monty/src/prepare.rs`: `prepare_function_def` creates `PreparedFunctionDef`.
- `crates/monty/src/expressions.rs`: `PreparedFunctionDef` metadata.
- `crates/monty/src/bytecode/compiler.rs`: `compile_function_body`, `emit_make_function`.
- `crates/monty/src/bytecode/code.rs`: transient native cache, constants and source metadata.
- `crates/monty/src/bytecode/native/`: current ABI, linker and offline stencils.
- `crates/monty/src/bytecode/vm/native.rs`: synchronous invocation and state materialization.
- `crates/monty-type-checking/src/type_check.rs`: existing checker returns diagnostics, not typed compiler IR.
- `crates/monty-types/src/resource.rs`: public `LIVE_MEMORY`, tracker preflights and memory/time probes.
- `crates/monty-alloc/src/lib.rs`: hard allocator accounting and breach handling.

## Current status at 20:49 UTC

The first scalar/range AOT implementation compiles and passes five differential integration tests
(with many individual programs), including memory-model checks. Typed hints survive parsing, preparation,
compilation and serde. Type analysis builds an explicit-value CFG, removes stack/local traffic within each
block and emits parallel copies at block edges. Native arithmetic is checked; integer overflow, zero division,
wrong runtime types and inexact mixed numeric comparisons fall back to the original function.

Pure direct range loops use private range state; builtin globals are guarded against shadowing.
Side effects, unknown calls, exception handlers and suspension remain interpreted. Typed pure helpers may
execute before and after a suspended caller. Original frame arguments remain rooted and unchanged.
A per-frame attempt marker avoids repeatedly speculating on a loop backedge after a miss.

`experimental-aot` is independent in behavior from the prior instruction-wise native tier. Its public TLS
switch defaults off. Tests explicitly disable the old native tier. Ahead-of-execution type/CFG lowering occurs
when compilation is enabled, while native linking happens at first eligible entry and is cached per Code.
Both transient caches are skipped in snapshots and lazily rebuilt after restore.

Executable mappings now have an allocator accounting lease (`monty-alloc::ExternalMemory`) and a bounded
process-wide mapping budget. The AOT linker preflights mapping/metadata allocation and its invocation buffer
against the soft budget; ordinary native code retains its max_memory fallback. This accounting change still
requires subprocess/hard-limit regression tests and review.

A first release build of `examples/typed_aot.rs` is running (exec session 69348). Do not change library source
or start another build until it completes; then freeze its binary and source hash before timings.
No builds may overlap timing. New suspension, shadowed-builtin and timeout tests have been written but have
not yet run. Native source has not been formatted/linted yet. Known warnings: deprecated Atomic fetch_update
on Rust 1.99 (MSRV remains 1.96), and unused Block.ip.

Next: initial fresh/retained timings; resource/suspension tests; read-only typed list/dict kernels; type validation
CLI; broader correctness/cross-target checks and application benchmarks. End the requested work period at
03:46:51 UTC, not before. No commits, pushes or heap-module edits.

## Update at 21:18 UTC

User steering: focus on workloads where Monty is currently slow. Do not do Unicode optimization: the user
already has work there. No Unicode optimization has been implemented in this worktree. String hints exist
only for dictionary keys (and the length IR currently conservatively needs exclusion for string inputs).

Eight focused tests (including snapshot/resume, builtin shadowing, timeout) passed. The full Monty Rust
suite also passed with experimental-aot,memory-model-checks; raw output is scalar-full-tests.log.
A third clippy pass left only CLI imports / excessive-bools findings; those have now been edited, but another
lint pass is due. The new experimental-aot CLI --aot flag enables existing validation for standalone inputs,
sets typed execution on its worker thread and disables the old native tier. Protocol clients retain control
of Configure.type_check (the kernel stays guarded even without validation). CLI integration is not yet tested.

Initial release microbenchmarks are frozen in initial-artifacts/ (binary, source.patch and hashes), and raw
9-sample medians are initial-timings.csv. These exclude type-checker cost. Fresh loops at 1000 iterations
improved 2.1-2.7x; retained loops improved 4.6-5.2x. Tiny fresh functions lost 3x and helper calls lost 20%.
No admission/cost model yet. Aot_enabled is now checked at frame entry only to reduce untyped dispatch overhead;
per-frame failed-attempt state prevents repeated speculation at a loop backedge.

The full fresh three-sample CPython comparison completed in gap-scan/ (CPython 3.15.0 versus the frozen
baseline Monty --no-native). gap-ranking.json ranks relative and absolute gaps. perf_unpack_sequence was
about 18x slower in Monty, game_spectral_norm about 4.7x, object-heavy examples 4-7x. Unicode gaps are excluded
from further work per user instruction. These screening medians are not confirmation measurements.

Samply profiling initially failed with Unknown(1100) under sandbox, then succeeded with escalated execution
of the same isolated child (session2359 completed): profiles/unpack-baseline.json, 12 runs, roughly4000samples.
Names are unsymbolized in the profile. Need symbolize before claiming which frames cost time. Frozen binary
Mach-O code id is in the profile and the runtime original binary still exists in shared target/release/monty.

The AOT range representation was improved from slot handles to three-word value triples. This allows range
locals and aliases, including repeated creation inside loops, without shared cursor corruption. No transient
range Vec remains. Canonical local/stack slots reserve three words, scalar SSA temps one word. Parallel copies
carry range triples. All twelve current focused tests pass (sequence-tests.log).

Read-only sequence/dictionary work is implemented and passing tests: homogeneous list/tuple scalar views,
union list[T]|tuple[T,...], guarded indexing, unpacking, len, dict[str,T] hits, dynamic/literal string keys,
wrong elements / missing keys / custom callbacks fallback. First sequence access lazily copies and validates
an unboxed array, retained for the invocation; original VM arguments remain GC roots. Dictionary reads use
the safe Rust helper and fall back before equal-hash Python key comparisons. No heap-module edits.
Sequence setup still copies the whole input and needs a threshold/hybrid path to avoid large sparse reads.
Code now uses BuiltinHint::{Range,Len} rather than range-only metadata. Length's unused Kind field needs fixing.

Next useful capability: perf_unpack_sequence returns a final tuple and reads loop-assigned locals after the
loop. Current scalar type analysis rejects those locals as possibly unbound. Plan a proper Maybe(Element)
local type with a bound flag in canonical slot word1, exact fallback on unbound loads, deferred failed blocks
until dataflow converges, and homogeneous virtual tuple construction directly before ReturnValue. Preserve
interpreter UnboundLocalError on zero iterations. This broadens general sequence code, rather than matching a
benchmark loop. Ordinary tuple/list mutation and arbitrary effectful functions still fallback in full.

No builds or benchmarks are currently running. Last sequence test session37174 completed successfully.
Do not commit/push or edit heap. Continue until 03:46:51 UTC.


## Update at 23:02 UTC

Separate combined worktree: /private/tmp/monty-native-study/threaded-aot, same baseline.
Original user threaded worktree is unchanged. Combined dispatch enters AOT only at calls/frame entry.
A Restart status unwinds handler chains when native execution replenishes instruction budget, preserving
bounded native stack depth. Fifteen focused tests passed in both variants, including repeated calls.

Frozen pre-fastcall binaries/source: match-artifacts/ and combined-artifacts/. The latter SHA256 is
 dd25a44f3f559f6b9fa5418c485e81b29c01d9ceb6d6720103ec528006e217a2.
threaded-comparison/ contains fifteen samples per scenario and five modes (match/threaded each with/without
AOT, plus CPython). Repeated unpack medians: 342.03 / 313.48 / 12.96 / 12.93 / 17.59 ms.
Spectral norm: 315.93 / 315.67 / 267.43 / 292.91 / 66.63 ms. Combining is useful but is not automatically
fastest. Other selected workloads were essentially unchanged. No compatibility changes in this cohort.

Main current source adds scalar direct calls: one finite primitive-only block, exact positional binding,
normal recursion admission/guard, reserved complete instruction budget (including ReturnValue), and
no callbacks. Guard failure leaves the callable/arguments intact for ordinary execution. New fast_calls
stats prove path selection. Main and combined scalar-call-tests logs both show 15 passed. No new timing
yet for this change. Combined error handling was tightened after those tests and needs another pass.

SmallVec invocation buffers, hybrid sequence views (copy <=4096 items, checked sparse reads above),
Maybe-bound locals, scalar tuple returns, icache-coherence 49.0.5 (MSRV1.96), strict standalone --aot
validation are now implemented. CLI -t remains advisory; --aot rejects diagnostics. Protocol validation
remains host controlled. perf_float annotation cohort does not pass checker; do not call it validated.

Worker tests previously passed: validation/fallback, memory+time limits with worker recovery, 160 resets
refunding executable mappings. New allocator hard-ceiling and strict CLI tests remain to run. Full latest
clippy, default/feature full tests, cross-target and actual MSRV checks remain. Policy scoring short/batch
workloads have been written but not measured or validated yet. No heap changes, commits or pushes.
Continue until 03:46:51 UTC.


## Update at 23:16 UTC

Main now supports homogeneous private tuple construction immediately unpacked (parallel assignment),
None/bool singleton identity, and direct readonly list/tuple for loops. Sequence cursors carry descriptor,
index and length in three words; small validated buffers or checked sparse reads share existing view rules.
Nineteen focused tests pass, including generated scalar edge cases and nested-loop/parallel-copy cases.
Latest full Rust suite passes with experimental-aot,memory-model-checks (latest-full-tests.log).
The datatest harness now has an experimental-aot feature that explicitly enables the tier per fixture;
642 Monty fixtures passed with memory-model checks (aot-fixtures.log). Worker tests: four passed, including
strict standalone CLI rejection, checked dynamic fallback, resource recovery and 160 reset/refund cycles
(latest-worker-tests.log). Allocator tests including executable-memory hard ceiling pass (allocator-tests.log).
Clippy for interpreter/runtime/allocator all targets is clean (latest-clippy.log).

Actual Rust 1.96 check fails on an unchanged baseline JSON cache loop over &mut Box<[T;16384]>.
HEAD has the same line. An isolated msrv-source copy with only that loop changed to .iter_mut() passes
cargo check for monty-runtime with experimental-aot on Rust1.96. No compatibility edit was made in the
main or combined worktree; msrv-baseline-compat.patch records the separate shim. Raw logs preserved.

Public documentation now explains the experimental feature and controls in root README, docs (new nav
entry), Rust quickstart, interpreter/runtime/allocator READMEs. There is no new CPython divergence requiring
a limitations entry: optimization eligibility falls back to existing behavior.

A fresh match release build is running (session7595, fastcall-release-build.log). Freeze it before building
another variant. Do not edit native source until completion. The forthcoming benchmark cohort includes
policy_scoring_short/batch and portfolio_exposure; validated_comparison.py separately measures Configure,
Feed (including optional validation), and their sum using Flying Circus's protocol client. No results yet.

Next experiment: reuse trusted offline templates directly, rather than copying identical machine functions
into per-plan RX mappings. Current operands are already in patch records, so this could remove most fresh
linking cost; indirect branch prediction may get worse. Keep both modes for a measured comparison. Building
an rlib archive alongside checked stencil objects should allow ordinary static linking with no runtime code
mapping for the shared variant. This is a proposal only at this timestamp.

No Unicode optimization, heap edits, commits or pushes. Continue until03:46:51UTC.


## Update at 23:34 UTC

Frozen fastcall binary: fastcall-artifacts/, SHA397795d38e7d99da29c73392c26b2de82383f66c6afce7fff91ff68a663c2031.
Fifteen-sample Flying Circus confirmation with three separate RSS samples is fastcall-confirmation/.
Repeated medians (baseline / previous AOT / new AOT / CPython ms): spectral 320 /267 /256 /66.9;
unpack324 /12.9 /12.9 /17.4; short policy0.56 /0.55 /0.56 /0.57; batch policy8.45 /6.58 /6.69 /4.41;
portfolio exposure4.70 /4.58 /1.44 /1.59. All five sources pass standalone type validation. The portfolio
scenario is newly written application-style code; do not present it as a standard Flying Circus workload.
No AOT regressions were classified; the single report regression is CPython versus Monty on short policy.

validated-confirmation/ contains a separate 15-sample protocol run with Configure.type_check true/false.
Validation-on Feed medians interp/AOT ms: short policy0.602/0.537; batch8.546/6.812;
portfolio4.715/1.465; spectral318.434/254.964; unpack320.227/13.520.
This is reused workers and repeated source after three warmups; type checker caches are warm. Initial
standalone validation costs about20ms for short policy, so cold/varied-source measurements are still due.
Custom harness was copied into results before editing. Current harness adds cold workers, varied source and
shared backend options, but those new modes have not been measured yet.

Shared templates are implemented as a second backend (Backend::Shared, CLI --aot --aot-shared). Build.rs
also emits an rlib archive of the checked optimized stencils; ordinary static linking supplies template
addresses. No executable mapping/lease is created for this backend. Both backends pass all19 focused tests,
including generated cases. The linker now preflights bounded scratch allocation. Patch tables use fixed
Vec buffers initialized through one raw base, avoiding references followed by mutation and Box move/retag
questions for their stored pointers. This change was applied to the old native prototype as well.

Five worker tests now pass (shared-worker-tests.log), including suspended CBOR dumps restored into a fresh
process using the opposite backend. Hints/code rebuild native artifacts after load, with no native addresses
serialized. The validation+snapshot test uses16MiB because lazy checker state alone exceeds512KiB; the
separate reset/refund test still uses512KiB and memory/time tests remain4MiB. Shared clippy is clean.
The legacy untyped native tier is now opt-in in AOT-feature library builds and disabled in AOT CLI builds,
so a typed-backend comparison cannot silently enable the earlier slower untyped tier.

The repository security-review skill is being applied locally to code generation, raw-buffer ownership,
callback guards, resource accounting and suspension. There is no PR for a reviewer-thread subagent to inspect.
A shared-backend release build is in progress(session24410). Freeze it, test/compile the combined variant,
then compare copied/shared plus match/threaded. No performance claim for shared templates yet.


## Update at 00:22 UTC

Typed/native features are now independent. experimental-aot activates its own optional build/runtime deps,
without compiling or enabling the earlier untyped tier. Its CLI --no-native disables typed execution too,
while --aot still requests strict validation. Build.rs now writes a pure object archive with the Rust
ar_archive_writer build dependency (already transitive in the baseline), selecting conventional .lib/.a
names. An attempted +verbatim link modifier failed for macOS unit-test linking and was removed.

Combined typed-only full suite passes906 tests,2ignored (combined-typed-full-tests.log). Six worker tests pass
with interpreter memory-model checks, including50 host replies per backend,40 native returns and10 deliberate
list-element hint violations falling back (container-reply-tests.log). A reproducible generated-control-flow
stress example has been added and a20-case debug smoke passes; long release stress is not yet run.

Frozen current typed-only binaries: match-typed-artifacts/ SHA86b9c52addc738fa9b9790e5e6e990573b19f04b8235cdf62745a9a12174cdaa;
combined-typed-artifacts/ SHA28686f0240b0b6b802d7c29491f4a9a8e16783b82c01e835f1326d98a8d85e64.
Example executables/hashes are in typed-example-artifacts/.

shared-threaded-confirmation/ contains15 samples,3RSS,5startup and3warmups, serial, eight workloads, seven
modes. Repeated medians ms (match /threaded /match copied /match shared /threaded copied /threaded shared):
spectral319 /313 /260 /260 /268 /268;
unpack321 /312 /12.8 /6.94 /12.6 /6.94;
short policy0.58 /0.56 /0.54 /0.52 /0.53 /0.51;
batch policy8.28 /7.83 /6.58 /6.51 /6.30 /6.31;
portfolio4.59 /4.14 /1.41 /1.42 /1.40 /1.42.
Shared templates are clearly better on repetitive unpacking and similar on the other eligible programs.
No compatibility changes/failures. Capacity, ticket and float controls largely fall back; perf_float does
not pass checker. Stock Flying Circus startup with --aot includes cold CLI validation, so it cannot isolate
native-backend setup. Request timings have Configure.type_check=false. Warm/cold/varied validation reruns due.

An actual Rust1.96 host run passed19 integration tests with both backends in isolated msrv-source plus the
separate baseline JSON-cache shim. Full Windows cargo check was blocked by the baseline stacker C dependency
requiring windows.h on macOS. Independently ran the actual build script for Windows x86_64 and aarch64 with
Rust1.96:46 stencils per target passed relocation/tail-continuation checks and static archives were produced.
The aarch64 archive linked into a standalone PE DLL with all46exports. x86_64 linking only lacks _fltused,
the CRT float marker; add a normal marker fixture for the SDK-free archive check and record that boundary.
Actual Windows execution is unverified. Logs and windows-stencil-checks.json preserve these distinctions.

cross-feature-snapshots.json checks six producer/consumer pairs: copied/shared dumps loaded in either typed
backend and the frozen pre-AOT interpreter. Every suspended call resumed to22.0; validation state retained.
No native addresses are serialized. New cache organization must preserve the existing Code.H hints payload.

Next changes are not yet implemented: make shared templates the default (retain explicit copied selector);
move hints and boxed plan/artifact caches into one optional typed metadata allocation rather than embedding
large OnceLocks in every Code; add same-Code changing-input coverage. In the combined dispatcher, decouple
source-instruction fuel (255) from bounded ordinary Rust handler-chain depth (15). Current coupling refuels
and unwinds on frequent compiled calls and explains some integration cost. Keep independent counters so
native execution never replenishes the physical chain budget. Test all re-entry/fusion/resource paths.

No builds/benchmarks are currently running. No Unicode, heap changes, commits or pushes. Deadline03:46:51UTC.

## Update at 01:29 UTC

Shared templates are now the default; copied templates remain an explicit comparison. Typed metadata and
boxed caches occupy one optional pointer in Code. Its H dump payload is still hints only. The changing-input
test passes with one compilation across empty, small, 4096/4097-boundary and large inputs, plus guard fallback.
Twenty focused tests now pass. Six worker tests cover validation, resource recovery, resets and suspension.

The combined threaded dispatcher now separates its source-instruction fuel from physical Rust handler depth.
Native execution cannot replenish the physical chain. Full combined tests, fusion dispatch tests and clippy
pass; logs are budget-full-tests.log, budget-fusion-tests.log and budget-clippy.log.

budget-confirmation/ contains fifteen timing samples, three RSS samples and serial execution using frozen
combined-budget-artifacts/ and compact-artifacts/ binaries. Threaded interpreter/combined typed repeated
medians in ms: spectral 318/239; unpack 315/6.91; short policy 0.56/0.52; batch policy 8.06/6.54;
portfolio 4.10/1.41. Unsupported controls capacity, ticket and float were 28.9/28.8, 26.7/26.9 and 216/216.
The earlier combined version took 271ms on spectral norm. No new typed regression was classified.

The full Flying Circus quick screen finished: 121 inconclusive workloads, no compatibility changes, and
three identical baseline failures in each mode (generator iteration in pidigits/nqueens; function attributes
in regex_effbot). Its three samples are screening evidence only, not proof of no regressions.

The first release differential stress run completed 100000 cases with both backends, seed 137, without a
mismatch (fuzz-runs/match-first/). Windows x86_64 and aarch64 template archives both linked in SDK-free PE
fixtures with all 46 exports; x86 uses a normal _fltused CRT marker fixture. Windows runtime remains untested.

Current source additionally rejects unsupported bodies before CFG analysis and skips hint metadata for wholly
untyped functions; twenty focused tests pass, but these admission changes are not in the frozen binaries yet.
Cold varied-source validation measurements are running against the frozen combined-budget executable.
Next: warm varied-source validation, current microbenchmarks, snapshot compatibility, final builds, longer
differential stress and current cross-target checks. No Unicode or heap edits, commits or pushes.


## Update at 01:49 UTC

The admission/compact-metadata combined source passed 907 Rust tests (2 ignored), 9 fusion dispatch tests,
6 AOT worker tests, 642 Monty fixtures with AOT enabled, and clippy. Actual Rust 1.96 passed 29 focused tests
in the isolated source plus the documented baseline JSON-cache shim. compact-cross-feature-snapshots.json
passes all six typed/old-interpreter producer-consumer pairs. Newly added documentation links resolve;
existing generated API links are outside the lightweight file check. Unrelated mdformat indentation was restored.

Cold/warm varied-source validation runs each used 15 samples, all outputs checked against CPython. Feed
medians with validation enabled, interpreter/shared AOT, cold then warm, in ms:
short policy 13.547/13.238 and 1.309/1.221;
batch policy 21.090/19.455 and 9.047/7.335;
portfolio 16.978/14.254 and 5.151/2.267;
spectral 329.403/250.940 and 318.102/238.237;
unpack 336.490/30.591 and 324.283/18.571.
Cold workers are recreated per sample; warm workers are reused. Distinct filename/comment per sample.
Startup/readiness/reset are excluded; Configure and Feed boundaries are recorded separately.

compact-timings.csv has 15 alternating 25ms micro samples. Fresh interpreter/shared AOT us:
tiny 5.824/6.899; while20 9.942/11.185; while100 19.272/13.136; while1000 122.022/32.648;
range1000 78.295/23.832; float1000 101.665/30.781; helper calls 189.277/153.129.
Tiny copied still costs 17.878us. Retained tiny 0.599/0.563us. These exclude type checking.

Frozen admission binaries are final-combined-artifacts/ and final-match-artifacts/. The longer combined stress
run stopped at seed20261113, case348948: generated wrong string data amplified through multiplication and
both reference/copied execution hit the soft memory limit with different live-byte counts. It was a generator
resource failure, not a return-value mismatch; original source/log/run classification are retained. The bounded
generator now uses None for these wrong inputs. Scalar call generation also covers argument effects and
positional, keyword and unpacked binding. A one-million-case bounded match run is active.

A further local-liveness pass is being evaluated. It removes only local edge copies that no later path reads;
arithmetic, loads and bound guards remain to preserve discarded errors. One-block functions skip the pass.
Twenty-one combined focused tests pass, including zero-iteration/unbound and discarded-division cases.
The liveness pipeline (session97358) is rerunning clippy/full/worker/fixtures, freezing both release variants,
then building a no-feature baseline and checking WASI fallback. Keep only if application timings justify it.
No timing work is running concurrently with builds or stress. Deadline remains03:46:51UTC.


## Update at 02:21 UTC

The liveness source passes 908 Rust tests (2 ignored), 9 fusion dispatch tests, 6 worker tests, 642 fixtures,
clippy and 30 actual Rust1.96 tests with the separate baseline shim. WASI fallback and x86_64 Linux compile
checks pass. x86 execution is unavailable on this host (Rosetta is not installed). No Windows execution claim.

The bounded generator completed one million match cases, seed20261113, and one million combined-liveness
cases for each of seeds20261113 and137, without mismatches. Per-backend native returns/fast calls were
779388/67310 for20261113 and781138/67888 for137. Original resource-generating input/log is preserved.

liveness-confirmation/ has15 timing and3RSS samples, serial. Repeated medians ms, no-feature default /
threaded-only /prior combined typed /liveness combined typed /liveness match typed:
spectral305 /318 /239 /238 /252;
unpack313 /315 /7.00 /7.01 /7.00;
short policy0.55 /0.55 /0.52 /0.50 /0.52;
batch policy7.66 /7.99 /6.25 /6.45 /6.55;
portfolio4.42 /4.11 /1.47 /1.40 /1.38.
Liveness helps portfolio modestly; batch policy is somewhat slower. Threading itself is mixed, so keep it
independent of AOT. Combined binary23602144 bytes versus no-feature23336224 bytes (+265920 bytes).
The full15-sample corpus comparison is still running(session47722); no builds or fuzzing overlap timing.

Current source, not yet rebuilt: main now includes threaded dispatch behind its own experimental-threaded
feature, forwarded by monty-runtime. experimental-aot alone retains match dispatch. Both select the same
resource-fuel semantics. The old combined worktree remains a frozen comparison source; future builds use main.
A diagnostic counter optimization moves instruction counting from each operation to checkpoint fuel grants
minus unused fuel at exit. It leaves the resource countdown/checkpoint cadence unchanged. A22nd focused test
checks known prior counts (including refills, scalar calls and guard misses). Build/test matrix is prepared in
unified_checks.py and must run only after the timing script completes.


## Final evidence at 03:39 UTC

Final code is frozen as guarded-match-artifacts/ (SHAe57f05511b42b70a1d18967ac3c75c5b96e4c78f86c315c47ef77071a5044761)
and guarded-combined-artifacts/ (SHA9adfb37553b3233adbd8f82c7a3e2147eb64b4f86515f607a4647ac77a1cd882).
The independent experimental-threaded feature now lives in the same source tree as experimental-aot.
Shared templates remain the AOT default; the match dispatcher remains the default dispatcher.

The final dictionary helper uses the existing canonical cached-hash API and identity shortcut, after exact
string guards, without editing string/Unicode code. A36KiB-key regression case passes. AOT entry tests inspect
the already-used IP before its attempted flag. Final full/core909 (2ignored), focused22, worker6, fixtures642,
MSRV31 and clippy pass. No-feature check and changed-Rust formatting pass. Native-source/Cargo hashes match
both frozen patches exactly. No heap/mount files changed. Current cross-feature snapshots pass all6 pairs.

Full worker suites were also tested: normal95passed/2failed versus unchanged baseline89/2; memory-model91/6
versus unchanged baseline85/6. All failure names match. Two are exact byte expectations; four additional
memory-model failures are reference-count assertions. The baseline used unchanged Monty source and an offline
lock refresh for the same sibling spam dependency; its patch/logs are retained. No tolerances were changed.

Final15-sample guarded-confirmation repeatedms (default/match/combined): spectral305/253/240;
unpack312/6.73/6.78; short policy0.54/0.50/0.51; batch7.66/5.98/5.95; exposure4.35/1.33/1.34;
long dict key0.38/0.24/0.25. The pre-fix AOT long-key case took25.0ms due to repeated hashing. The7-case
fallback recheck still has about3–10% overhead: entry ordering did not remove it. The earlier full15-sample
match corpus run had15 regression classifications,0 improvements and unchanged compatibility. Threaded
full-corpus results were mixed/worse. Both features therefore stay opt-in; no overall speedup is claimed.

Final fresh micro us interpreter/shared: tiny5.827/6.989; while20 9.749/11.545; while100 18.919/13.279;
while1000 119.585/31.400; range1000 74.500/20.600; nested374.407/65.825; float95.839/29.616.
Retained while1000 111.619/20.766. Fifteen alternating25ms windows; validation excluded.

Final cold checked Feedms interpreter/shared: short13.680/13.532; batch21.126/18.932; exposure17.388/14.114;
long-key11.791/11.878. Fifteen samples, fresh workers, varied source/filename, startup/readiness/reset excluded.
Two final1M-case seeds pass with both backends, including new heap-string-key cases: combined20261113 and
match137. Those are bounded grammar cases, not arbitrary Python coverage. Earlier stage seeds/logs retained.

The backend is a typed operation graph over precompiled Rust templates. The copied comparison copies native
template bodies and links operand records; whole-function native emission/register allocation remain future
work. Effectful/suspending bodies stay interpreted; pure callees accelerate before/after suspension. No raw
native addresses are persisted. Windows templates build/link, WASI/x86Linux compile; Windows/x86 execution
and Miri native execution remain unverified. No public Python/JS control was added.

Preparing a persistent source/results bundle under the user's benchmark experiments directory. Implementation
source remains in the isolated checkout. No commits, pushes, publications or edits to the user's existing source.
The requested window ends03:46:51UTC; goal completion is recorded only after that time.


Requested eight-hour window completed at 2026-11-13T03:47:28.246504+00:00. Implementation, measurements and persistent bundle are complete; no commits or pushes.
