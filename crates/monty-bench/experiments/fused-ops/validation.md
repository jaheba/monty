# Validation

- `cargo test -p monty --features experimental-fusion,memory-model-checks --tests`: 889 tests passed across 46 test binaries.
- `cargo test -p monty --test fused_ops --features memory-model-checks`: all 3 tests passed with fusion disabled.
- All twelve benchmark scripts produced the same stdout under CPython 3.14 and both release worker binaries.
- `cargo clippy -p monty --features experimental-fusion,memory-model-checks --lib --test fused_ops --example fused_ops -- -D warnings`: passed.
- The broader Clippy invocation with `--tests` is blocked by an existing `needless_raw_string_hashes` finding in
    `crates/monty/tests/keyword_storage.rs:6`.
    Without `memory-model-checks`, it also reports three existing `forget_non_drop` findings in dict storage.
- The feature build pins its own opcode fingerprint in the existing dump-format test.
    Cross-build compatibility of dumps containing fused opcodes remains deferred.
- No heap implementation files were changed for this experiment.
