# Shared execution benchmarks

Measured against PR #1088 at `f7ddd917` using Rust 1.99.0 on macOS ARM64.
Each sample measures 100 complete feeds, including compilation and session creation; the table reports the median
of five samples in microseconds per feed.
The release profile matches the repository: fat LTO, one codegen unit, stripped symbols.
Limits and forced-GC checks are disabled.
These short local runs do not establish a general performance gain.

| Workload           | Before (µs) | Final (µs) | Change |
| ------------------ | ----------: | ---------: | -----: |
| python/calls       |       41.88 |      41.99 |  +0.3% |
| javascript/calls   |      223.83 |     205.51 |  -8.2% |
| python/unwind      |       37.22 |      35.52 |  -4.6% |
| javascript/unwind  |      171.87 |     157.14 |  -8.6% |
| python/strings     |      538.45 |     509.58 |  -5.4% |
| javascript/strings |      725.53 |     925.04 | +27.5% |

The call workload makes 500 interpreted calls per feed; unwind makes 100 calls with alternating exceptions.
The string workload repeatedly slices, searches and runs regexes over text containing astral characters.
MontyScript's string/regex regression adds scalar-index scans and UTF-8→UTF-16 conversions for the existing regex
executor; their individual contributions were not profiled.
Python's production call result differs by 0.3%.
Earlier exploratory builds without LTO showed a roughly 14% Python call regression that did not reproduce with the
repository's production profile; those builds are not the table above.

## Reproduce

Use the same harness and lockfile for both checkouts.
The harness deliberately compares each language with its own baseline, not with the other language.

```sh
cargo new --name shared-execution-bench /tmp/shared-execution-bench
cargo add --manifest-path /tmp/shared-execution-bench/Cargo.toml --path "$PWD/crates/monty"
cargo add --manifest-path /tmp/shared-execution-bench/Cargo.toml --path "$PWD/crates/monty-ecma"
cargo add --manifest-path /tmp/shared-execution-bench/Cargo.toml --path "$PWD/crates/monty-types"
cp design/benchmarks/shared_execution.rs /tmp/shared-execution-bench/src/main.rs
cat >> /tmp/shared-execution-bench/Cargo.toml <<'PROFILE'

[profile.release]
lto = "fat"
codegen-units = 1
strip = true
PROFILE
cargo run --release --manifest-path /tmp/shared-execution-bench/Cargo.toml
```

For the baseline, change the three dependency paths to a checkout of `f7ddd917`, retaining the harness, profile and lockfile.

## Bytecode-backed map

Measured against `2e8619e0` with `map_frames.rs`, the release profile above, and the same five samples of
100 complete feeds per sample.
These runs include compilation and realm creation.

| Workload                  | Before (µs) | After (µs) |  Change |
| ------------------------- | ----------: | ---------: | ------: |
| 20 maps over 1,000 values |     2634.09 |    7006.86 | +166.0% |
| 500 ordinary calls        |      211.97 |     202.38 |   -4.5% |

The map loop now dispatches bytecode for element checks, loop control and result writes, where the previous loop
ran directly in Rust.
Callbacks retain ordinary interpreter frames in both versions; the new outer frame makes the loop resumable.
The map workload is 2.66 times slower in this local measurement.
The ordinary-call result does not establish a performance gain.

To reproduce, substitute `map_frames.rs` for `shared_execution.rs` in the harness instructions above and compare
against `2e8619e0` using the same dependency paths, lockfile and release profile.
