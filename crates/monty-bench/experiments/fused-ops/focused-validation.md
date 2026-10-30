# Focused fusion validation

- `cargo test -p monty --features experimental-fusion,memory-model-checks --tests`: 892 tests passed.
- The same six fusion regression tests also pass with `experimental-fusion` disabled.
- All fourteen workloads produce the same stdout under CPython 3.14 and both release worker binaries.
- `cargo clippy -p monty --features experimental-fusion,memory-model-checks --lib --test fused_ops --example fused_ops -- -D warnings`: passed.
- Focused rustfmt checks and `git diff --check`: passed.
- The ignored `workload_fusion_census` diagnostic passed and produced `focused-census.csv`.
- Added coverage includes overflow promotion, float and string fallbacks, bool fallback parity,
    signed modulo, zero divisors, `i64::MIN % -1`, and unbound local operand ordering.
- No heap implementation files were changed.
- The feature build pins its own opcode fingerprint; cross-build dump compatibility remains deferred.

The earlier broader Clippy invocation is blocked by pre-existing findings described in [validation.md](validation.md).

## Isolated commit validation

The staged fusion changes were tested separately from the unrelated dirty working-tree edits.
All 863 interpreter tests passed with `experimental-fusion,memory-model-checks` enabled.
This check used the base commit's `python-spam` dependency API, from revision `1de73bb` of the local Spam repository.
The earlier 892-test run and all benchmark timings used the dirty working tree recorded in the provenance files.

The bool regression case checks constant-operand and generic-operand fallback parity, so it does not assume the unrelated
bool behavior change present in that working tree.
The experiment Python scripts pass Ruff; source snapshots are excluded from formatting to preserve the measured input text.
