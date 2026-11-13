# Local boundary review

This review applies the repository's `review-security` checks to the prototype and its callers.
It is a local review of uncommitted code, with no PR or external review threads.

## Execution and ownership

- User programs select typed operations; they do not supply machine code, compiler source or function addresses.
    The offline templates are compiled from repository Rust during the build.
- Parameter guards establish exact primitive/base-container types.
    Container elements are guarded before use; subclasses, missing dictionary keys and callback-sensitive keys return to ordinary execution.
- Eligible bodies have no Python-visible effects.
    On a guard miss, original bound arguments and frame state remain available for replay from entry.
    Defaults and argument expressions are not evaluated again.
- Scalar direct calls require exact positional binding and a finite primitive-only block.
    They retain recursion admission and a recursion token, reserve the complete body/return budget, and leave callable/arguments intact on a miss.
- Original argument objects remain VM roots.
    Sequence buffers own copied scalar words; large sequences use checked Rust reads.
    No heap borrow or reader handle survives a callback or GC checkpoint.
- Patch graphs own fixed `Vec` allocations.
    Internal addresses are derived from one raw base, and initialization uses that base before publication.
    Published patches are immutable; moving an artifact does not move or retag the vector allocation.
- Register, view and array accesses are bounded.
    Large views have no data pointer, and the index/iteration paths use the checked helper for them.
    Unpacking is limited to at most256 elements, so it cannot read a large-view null pointer after the length check.

The local-liveness pass prunes only value copies across block boundaries.
Arithmetic, discarded local loads and bound guards remain executable, preserving errors and fallback.
Source-instruction weights are unchanged by removing these zero-weight copies.

## Checkpoints and suspension

The combined dispatcher separates source-instruction fuel from its bounded Rust handler-chain depth.
Native checkpoints replenish source fuel without extending the physical handler chain.
Fused instructions charge their additional source operations to fuel.
Existing deadline, nested-callback, frame-change and ownership checks pass with this separation.

Suspending/effectful bodies remain interpreted.
Pure helpers can execute before and after host calls.
`TypedCode` serializes only the existing `Code.H` hints payload; plans, patch addresses and machine mappings are rebuilt locally.
Tests restore suspended CBOR dumps between both backends, and a separate protocol check restores their dumps in the pre-AOT interpreter.
The existing snapshot provenance/integrity contract is unchanged.

## Resources and sandbox

The shared backend uses statically linked templates and allocates no per-plan executable mapping.
The copied comparison backend uses RW construction followed by RX protection, instruction-cache synchronization,
a process mapping cap, and an allocator-accounting lease retained until unmapping.
Linker scratch and invocation buffers have bounded growth/preflight checks.
The allocator hard-ceiling test and repeated session-reset test pass.

Compiled code introduces no filesystem, environment, network or subprocess operations.
General calls and effects execute through existing interpreter/host boundaries.
No heap-module or mount-boundary file has been changed.
The runtime does not invoke a Rust compiler; build-time compilation is separate from sandbox execution.

## Evidence and limits

- Focused differential tests cover wrong hints, overflow/domain errors, callback effects, range aliases,
    sequence thresholds, parallel copies, unbound locals, recursion, and changing inputs with one cached compilation.
- The initial generated stress run executed 100000 programs against the interpreter and both backends without a mismatch.
    The bounded generator subsequently completed one million cases in both match and combined-liveness builds,
    with 67310 successful direct scalar calls per backend in each run.
    Some programs deliberately violate hints; this tests guards rather than static checker acceptance.
    An earlier long run stopped on generated string amplification: both modes hit the soft memory limit.
    That source/log is retained, and wrong elements now use None to keep generated allocations bounded.
- Full interpreter, worker, fusion and memory-model checks are recorded in the worklog and raw logs.
- Rust 1.96 testing uses a separate one-line fix for an unchanged baseline JSON-cache iteration error.
- Windows templates were built for x86_64 and aarch64; complete Windows compilation is blocked locally by the baseline
    `stacker` dependency's Windows SDK requirement.
    Native Windows execution remains unverified.
- The current combined x86_64 Linux backend compiles; this host cannot execute x86 binaries.
- No Miri execution of native templates has been claimed.

## Worker-suite baseline comparison

The full subprocess integration file under `memory-model-checks` reports six failures: four reference-count
assertions and two exact live-byte expectations. All six reproduce with both experimental features disabled,
and in an unchanged archive of baseline commit `9a9f8784bf6cb36b09f183b53d7d829bcd807bdf`.
The baseline lock was refreshed offline for the same archived sibling spam dependency; the small adaptation
is recorded separately. No interpreter source was changed in that baseline archive.

`subprocess-baseline-comparison.json` records the identical failure names, with raw logs for all three runs.
The normal worker configuration is checked separately; these baseline failures must not be described as a
passing full memory-model worker suite. The six focused AOT worker tests do pass in memory-model mode.

Normal worker mode passes 95 tests and fails two byte-expectation assertions. Both failures reproduce on the
unchanged baseline (89 passed, without the six added AOT tests), with nearly identical reported usage.
The reference-count assertions are specific to memory-model mode. No test tolerance or unrelated source was
changed to conceal these baseline failures.

## Final cache and entry changes

Native dictionary reads now request the existing canonical cached hash only after checking an exact string.
The dictionary borrow begins after that scoped hash operation, so no heap borrow crosses mutable VM access.
The query remains rooted in the original frame; the temporary heap reader releases its reader count normally.
Identity shortcuts avoid re-comparing a heap key against itself. Callback-sensitive colliding keys still miss
before equality, and replay remains effect-free. Two final million-case runs include dynamic heap-string keys.

The final source and Cargo hashes match both frozen executable source patches; changed Rust files pass rustfmt.
The final snapshot check again restores both producers in both backends and the pre-AOT interpreter.
