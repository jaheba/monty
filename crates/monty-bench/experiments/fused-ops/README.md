# Fused opcode experiments

`monty/experimental-fusion` enables eight focused superinstructions; the default build is the baseline.
See [focused-results.md](focused-results.md) for the current measurements.
The original 112-opcode experiment is recorded in [results.md](results.md).
Dump compatibility between the feature and baseline builds remains deferred.

## Focused implementation

The active registry contains three cases:

- `LoadLocal0; LoadLocal1; BinaryAdd` and `LoadLocal1; LoadLocal2; BinaryAdd` read integer locals directly and push one result.
- `LoadSmallInt; BinaryAdd/BinarySub/BinaryMul/BinaryMod` updates an integer operand in its existing stack slot.
- `CompareLt/CompareEq; JumpIfFalse` compares integer operands and branches without pushing a temporary boolean.

The integer paths use checked arithmetic.
Overflow, other operand types and undefined locals use the existing handlers, preserving their errors and ownership rules.
Modulo adjusts Rust's signed remainder to Python's divisor-sign rule; zero divisors and `i64::MIN % -1` use the fallback.

Fast paths subtract the constituent instruction count in one step only when the sequence fits before the next checkpoint.
Otherwise, the ordinary handlers run with their original checkpoint boundaries.
The comparison fast path truncates only immediate integers; heap operands always use the existing cleanup path.

Fusion replaces the first opcode and retains the following bytes.
Jumps, source locations, exception ranges and interior entry points keep their offsets.
The longest matching pattern wins and selected patterns do not overlap.

`generate.py` writes the enum variants, registry and dispatch arms, and copies `focused_vm.rs` to the VM module.
It runs rustfmt on the generated files.
`generate_broad.py` is the historical 112-opcode generator; running it replaces the active experiment's generated files.

## Reproduce

Use the repository's Python environment setup before building if needed.
Build and copy the baseline before enabling fusion:

```sh
mkdir -p /tmp/monty-fused-ops
cargo build -p monty-runtime --release
cp target/release/monty /tmp/monty-fused-ops/baseline
cargo build -p monty --release --example fused_ops
cp target/release/examples/fused_ops /tmp/monty-fused-ops/baseline-direct
cargo build -p monty-runtime --release --features monty/experimental-fusion
cp target/release/monty /tmp/monty-fused-ops/focused
cargo build -p monty --release --example fused_ops --features experimental-fusion
cp target/release/examples/fused_ops /tmp/monty-fused-ops/focused-direct
```

Run `compare.py BASELINE-DIRECT CANDIDATE-DIRECT --output NEW-DIRECTORY` for parse-once execution timings.
It alternates build order over three rounds, collecting 15 samples per workload per round.
Each sample runs the workload for at least 100 ms, excluding compilation and warmups.
Raw CSV files and median comparisons are retained.

`run_flying.py BASELINE CANDIDATE --output NEW-DIRECTORY` runs the worker comparison with 20 timing samples,
three repeated warmups, three separate RSS samples and five startup samples, serially.
Request timings include compilation, execution and output delivery; expected output comes from harness CPython.
Do not run benchmarks concurrently with builds or other benchmarks.

The original twelve scripts contain four `local_*` microbenchmarks and eight snapshots of existing interpreter benchmarks.
The focused round adds `local_add.py` and `small_int_math.py` to isolate the new arithmetic paths.
Each script prints a deterministic result; the parse-once runner replaces the final print with a result expression.

To count static fusion sites across compiled module and function bodies:

```sh
cargo test -p monty --features experimental-fusion,memory-model-checks --lib workload_fusion_census -- --ignored --nocapture
```

These counts do not weight sites by their execution frequency.
