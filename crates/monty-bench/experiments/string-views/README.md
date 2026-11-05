# String views

This experiment is based directly on upstream `2a3269043e544d0e7bd99e3e5cf0c306a8246076`.
The baseline is that unmodified revision; the candidate adds only string views and their tests and benchmarks.

## Representation

Heap strings below 128 bytes keep `Box<str>` storage.
Larger heap strings use `Arc<Box<str>>` and a byte range, preserving their existing buffer without copying it into an Arc.
A contiguous slice shares that buffer when it is at least 128 UTF-8 bytes and at least one quarter of the original allocation.
Otherwise it copies; results up to 15 bytes use main's existing inline representation.
Empty results reuse the empty string, and full heap-string slices reuse their source value.

Chained views add byte offsets directly, retaining no intermediate strings.
Backing lifetime follows the Arc reference count, independently of the source string's heap lifetime.
No ownership promotion or collector changes are needed.
True ASCII status propagates to views, while non-ASCII slices recompute their own status when first needed.
Unicode slicing still computes character boundaries; it does not become a constant-time operation.

Interned-string slices and non-unit steps materialize their results.
String methods borrow visible bytes through main's existing `HeapView` abstraction.
Snapshots keep the existing text-only format and restore independent storage.

## Interpreter measurements

The baseline and candidate use the same `profiling` profile, with optimization and no LTO.
Ordinary workload times are medians of seven executions after one warmup; compilation is excluded.
These are indicative local timings, without exclusive control of CPU load or confidence intervals.

The CSV timings and allocation columns come from separate executions in the same allocator-instrumented process.
Allocation-count and peak recording are disabled for timing samples, but live-byte accounting stays enabled.
Counts include reallocations, and peak bytes measure additional live allocator bytes rather than RSS.
The retained rows keep a live REPL after dropping the original string.
Snapshot and retained rows are single measurements; snapshot includes both dumping and loading.
The published example runs the same timing workloads without allocator instrumentation:

```sh
cargo run -p monty --profile profiling --example string_views
```

| Workload   | Baseline time |  View time | Baseline allocations | View allocations |
| ---------- | ------------: | ---------: | -------------------: | ---------------: |
| half       |       48.8 µs |    31.9 µs |                   11 |               11 |
| prefix     |    9,112.9 µs | 2,562.1 µs |                19994 |              129 |
| suffix     |    7,937.3 µs | 2,619.5 µs |                19994 |              129 |
| chain      |       60.5 µs |    38.8 µs |                   14 |               14 |
| lines      |      132.6 µs |   151.3 µs |                 1041 |             2044 |
| tokens     |      154.4 µs |   151.8 µs |                   19 |               20 |
| retain     |       29.9 µs |    29.1 µs |                   11 |               13 |
| unicode    |      349.7 µs |   334.3 µs |                   11 |               11 |
| operations |      474.0 µs |   467.2 µs |                  311 |              611 |
| snapshot   |      631.0 µs | 2,296.6 µs |                   48 |               49 |

Main already caches ASCII status and copies contiguous UTF-8 slices without collecting characters.
The main benefits are avoiding large substring buffers and repeated shrinking copies.
Line splitting still materializes results and exposes the added header allocation on strings at least 128 bytes long.
A retained half-string keeps the million-byte backing alive; a retained 128-byte token owns only its copied bytes.

## Threshold matrix

```sh
cargo test -p monty --profile profiling --lib benchmark_string_view_policies -- --ignored --nocapture
```

`policies.csv` compares always-copy, always-view above 15 bytes, and twelve combinations of minimum lengths
32/64/128/256 with retained fractions 12.5%/25%/50%.
The storage-only benchmark excludes VM objects, character-boundary searches and initial string construction.
Copy lengths and allocation counts are derived from the storage operations, not allocator instrumentation.
Each independent payload uses one allocation, plus a shared header when it reaches the policy minimum.
Peak and retained figures count backing payload bytes only.
The lines/tokens rows discard each result while retaining the source; the retain row replaces the source.
Single-slice timings are too short for threshold tuning.

Shrinking a 98,304-byte string down to one byte copies:

| Policy                           | Bytes copied after construction | Peak backing payload bytes |
| -------------------------------- | ------------------------------: | -------------------------: |
| Always copy                      |                   4,831,789,056 |                    196,607 |
| 12.5% retained, 128-byte minimum |                          22,141 |                    110,591 |
| 25% retained, 128-byte minimum   |                          40,764 |                    122,879 |
| 50% retained, 128-byte minimum   |                         106,231 |                    147,455 |

The minimum adds a bounded copying tail when strings become too short for views.
The geometric copying estimate alone omits that tail.
Keep 128 bytes and 25% as initial defaults; these workloads do not establish a universally best threshold.

## Storage construction

Both runner types retain their existing `Send` support, which excludes `Rc<str>`.
`Arc<str>` preserves `Send` and needs only one persistent allocation, but converting an existing box copies its bytes.
The shared box instead preserves the payload and adds a separate header.

```sh
rustc -O crates/monty-bench/experiments/string-views/storage_construction.rs -o /tmp/string-storage
/tmp/string-storage
```

`storage-construction.csv` records seven-batch median construction/drop times, including allocating the input buffer.
At one million bytes, `Arc<str>` took 37.3 µs and `Arc<Box<str>>` took 16.6 µs.
At eight bytes, `Arc<str>` took 59 ns and the shared box took 69 ns; production strings of this size remain inline.

## Validation

Storage tests check original backing identity, flattened offsets, threshold rounding, backing release and serialization.
Interpreter tests cover Unicode boundaries, slice steps and bounds, full-slice identity, hashing, dictionary keys,
formatting, concatenation, methods, source release, `Send` and snapshots with live iterators.
A subprocess test checks sharing and backing release with an armed allocator and a 1 MiB soft limit.

Validation on this clean base passed 1,026 interpreter tests with `memory-model-checks`, all 104 subprocess tests,
and all 24 borrow-checker compile-fail cases (including focused reruns after line-number-only snapshot updates).
`make format-rs`, `make lint-rs`, the example's Clippy check and the storage-policy benchmark also passed.
