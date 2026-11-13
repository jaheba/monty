# Native region prototype

An optional copy-and-patch compiler for ordinary Monty bytecode.
It compiles operations and control flow independently, without whole-loop recognizers or runtime profiling.
The baseline is commit `9a9f8784bf6cb36b09f183b53d7d829bcd807bdf`, with experimental fusion disabled.
The original working checkout's uncommitted changes are not part of this experiment.

## Implementation

`monty/build.rs` compiles 34 standalone Rust stencils and extracts their machine code with the `object` crate.
The build rejects external relocations within a stencil and checks that continuations are tail calls in LLVM IR.
The runtime copies one stencil per bytecode instruction, links its destinations and makes the mapping executable.
An immutable artifact is cached on each `Code` after its first execution attempt.
Artifact addresses are skipped by serialization and reconstructed after loading a snapshot.

Supported operations include local/global loads and stores, inline integer arithmetic and comparisons, scalar truth
tests, conditional/unconditional branches, builtin range advancement, and successful string-key dictionary reads
returning scalar values.
The dictionary probe exits before comparing a colliding key that could execute Python.
Missing keys, non-scalar dictionary results, other operand types and unsupported opcodes use the interpreter.
Integer overflow exits before the arithmetic operation so the interpreter can produce a big integer.

Each native invocation imports a scalar image of the current frame and globals.
Non-scalar slots borrow the original VM values; their owning references stay in the unchanged VM throughout native execution.
Callbacks use the existing safe heap API and cannot allocate or execute Python.
The range callback advances the real iterator only after checking output capacity, so aliases observe ordinary progress.
Before returning to the interpreter, both images are materialized while all borrowed sources are still alive.
The VM then releases replaced values and resumes at the first unexecuted bytecode instruction.
The existing instruction countdown bounds each invocation and retains deadline, GC and print checkpoints.

Calls, returns, other iterator types and suspension run through the interpreter.
Native arithmetic before and after those operations remains available.
Generator frames can execute native regions between yields.
The prototype has no register allocation, cross-operation type propagation, call inlining or dictionary writes.
Its scalar image still receives loads and stores between stencils.

## Limits

- Native execution is disabled when `max_memory` is configured: executable mappings are outside allocator accounting.
- Executable mappings have an 8 MiB process-wide budget, with conservative 64 KiB rounding per artifact.
- A function is limited to 64 KiB of bytecode and 4 MiB of emitted machine code.
- An invocation admits at most 8,192 existing frame/global slots and the ordinary checkpoint's instruction budget.
- The independent `experimental-fusion` feature disables native artifact generation.
- AArch64 macOS execution is tested locally; x86-64 Linux is cross-checked, and WASI uses the interpreter.
- This is an uncommitted research prototype with a new unsafe code-generation boundary.
- The experimental feature requires Rust 1.97 through its cache-coherence dependency; this run uses Rust 1.99.

## Run

From the experiment worktree:

```sh
cargo test -p monty --features experimental-native,memory-model-checks
cargo clippy -p monty --features experimental-native --all-targets -- -D warnings
cargo run -p monty --release --features experimental-native --example native_regions -- 9
```

This baseline expects a sibling `python-spam` dependency with its earlier API.
The experiment's sibling dependency is archived from `jaheba/tokspam` commit
`24c3a27486cbff301fe5b9586ce725ebf5a829e3`; only its Cargo package/library names are restored to `python-spam`/`spam`.
The current `/Users/jaheba/p/spam` checkout is unchanged.

## Measurement

`native_regions` compares enabled and disabled native execution in the same release executable.
The disabled mode includes the feature's code-object layout and dispatch branch.
Fresh samples include source parsing, bytecode compilation, native compilation, execution and runner destruction.
Retained samples reuse compiled program objects after one checked execution.
The twelve cases are defined in `crates/monty/examples/native_regions.rs` and remain the same in both modes.
Each sample runs for at least 25 ms; mode order alternates, and output checks precede each timed sample.
Compilation, native-instruction and dictionary-read counters accompany timings to establish execution coverage.
Results are local workload measurements, with no synthetic aggregate score.

Results and validation logs are stored in this directory.

## Local results

AArch64 macOS, release profile with fat LTO, nine paired samples per case and timing boundary.
Speedup is interpreter time divided by native time; values below 1 indicate regressions.

| Case | Fresh interpreter / native, µs | Fresh speedup | Retained speedup |
| --- | ---: | ---: | ---: |
| tiny_expression | 3.57 / 14.17 | 0.252× | 0.845× |
| short_while | 6.73 / 16.62 | 0.405× | 1.924× |
| while_1000 | 114.04 / 58.19 | 1.960× | 2.724× |
| branches | 157.72 / 73.48 | 2.146× | 2.852× |
| for_range | 71.20 / 43.07 | 1.653× | 2.524× |
| dict_short | 9.96 / 21.35 | 0.466× | 1.404× |
| dict_loop | 163.90 / 105.25 | 1.557× | 1.803× |
| dict_mutation | 179.68 / 239.57 | 0.750× | 0.782× |
| helper_calls | 177.88 / 397.30 | 0.448× | 0.454× |
| local_loop | 117.38 / 73.17 | 1.604× | 2.691× |
| float_fallback | 93.23 / 156.58 | 0.595× | 0.645× |
| string_fallback | 4.29 / 15.28 | 0.281× | 0.742× |

The integer loops, branches, range iteration and repeated dictionary reads improve without whole-loop recognition.
Small fresh programs lose to setup and artifact lifetime costs; native compilation itself averages about 3–5 µs
per artifact in these samples.
Frequent calls, dictionary writes and float arithmetic return to the interpreter often.
The helper-call case executes about six native instructions per entry, compared with roughly 250 in the long scalar loops.
That difference is consistent with state materialization dominating short regions, though no CPU profile was collected.

`initial-timings.csv` records the first implementation, before builtin range advancement was supported.
Its range loop was about 0.52× interpreter speed fresh and 0.56× retained.
The final implementation adds a reusable range-advancement operation and reaches 1.65× fresh and 2.52× retained
on the same frozen case.
That follow-up also preserves the last native instruction offset for checkpoint errors and checks mapping addresses
after changing executable permissions.
The other eleven cases were retained throughout the follow-up.
This establishes operation coverage on these cases, not general application speed.

## Validation

- 877 Rust tests passed with `experimental-native,memory-model-checks`, including nine focused native tests.
- All 642 Monty fixture cases passed with the same features; the CPython half was filtered out of this run.
- Clippy passed for all Monty targets with warnings denied.
- The default build passed.
- x86-64 Linux cross-check and stencil relocation audit passed; x86 execution was not tested.
- WASI compilation passed with native execution omitted.
- Tests cover arithmetic overflow, exception locations, colliding Python keys, iterator aliases, deadlines, generators,
  external-call suspension, serialization, restored native artifacts and range progress across suspension.

Raw samples are in `timings.csv`; `summary.csv` includes coverage and compilation counters.
Run `python3 summarize.py` to regenerate the summary.
`metadata.json` records the baseline, toolchain and runtime source hashes.
The source patch is stored at `/private/tmp/monty-native-study/prototype.patch`.
