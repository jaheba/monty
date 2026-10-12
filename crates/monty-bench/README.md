# List storage benchmarks

## Application workloads

`workloads/` contains standalone Flying Circus benchmarks with deterministic inputs and checked output:

| Workload | Numeric lists | Operations |
| --- | --- | --- |
| `list_int_sales` | 200,000 daily revenues in cents | Weekly and rolling monthly sums, slices, copy, sort |
| `list_int_latency` | 150,000 latency samples in microseconds | Percentiles, replay-buffer copy and extension, updates, histogram |
| `list_float_sensor` | 160,000 raw and calibrated readings | Calibration, downsampling, block means and variances |
| `list_float_portfolio` | 4,096 positions over 120 trading days | Indexed valuation, daily sums, copies, periodic rebalancing |

Construction is timed along with processing; there is no prebuilt input fixture.
Sales reports exercise typed slices and integer reductions.
Latency and sales sorting currently expand numeric storage into `Value` elements.
Sensor summaries exercise float copies and slices alongside ordinary float reductions.
Portfolio valuation spends more time in Python arithmetic and indexing, so it serves as a control for dispatch costs.
All workloads retain homogeneous numeric lists throughout processing.
Outputs use integer totals or rounded float checksums; Flying Circus compares them with CPython.

Run all four against a cached baseline and a release candidate:

```sh
flying-circus --diff --samples 20 \
  --benchmark crates/monty-bench/workloads/list_int_sales.py \
  --benchmark crates/monty-bench/workloads/list_int_latency.py \
  --benchmark crates/monty-bench/workloads/list_float_sensor.py \
  --benchmark crates/monty-bench/workloads/list_float_portfolio.py \
  -r baseline=@main -r candidate=target/release/monty
```

Add `--include-defaults` to run these together with the full built-in suite.
The cache name must identify the intended baseline; `@main` does not follow Git branches.

The local comparison on 2026-10-12 used 20 timing samples per mode, three repeated warmups and ten RSS samples.
Both builds produced the expected CPython output for all four workloads.

| Workload | One-shot before / after | Repeated before / after | Peak RSS before / after |
| --- | ---: | ---: | ---: |
| Integer sales | 87.34 / 32.13 ms | 93.35 / 34.52 ms | 16.56 / 13.55 MiB |
| Integer latency | 49.46 / 48.64 ms | 56.15 / 51.62 ms | 19.67 / 18.69 MiB |
| Float sensor | 52.70 / 50.52 ms | 55.56 / 54.11 ms | 17.94 / 13.73 MiB |
| Float portfolio | 82.84 / 82.81 ms | 90.29 / 91.30 ms | 11.09 / 10.77 MiB |

Sales processing was about 2.7× faster in both modes.
The harness classified the latency workload's 8% repeated improvement as inconclusive.
Other timing changes stayed below the 5% reporting threshold.
Peak RSS decreased by 18% for sales, 5% for latency and 23% for sensor processing.
The baseline was `@tagged-heap-main-f190cda9`; the candidate was the release build with specialized lists.
These measurements precede the typed float reduction.

With the typed float reduction, a subsequent 20-sample run against the same cached baseline measured:

| Workload | One-shot before / after | Repeated before / after |
| --- | ---: | ---: |
| Float sensor | 52.64 / 43.76 ms | 51.08 / 42.85 ms |
| Float portfolio | 83.40 / 72.82 ms | 82.49 / 72.10 ms |

The harness classified both improvements as significant: 16–17% for sensor processing and about 13% for portfolio valuation.
The isolated million-element float sum fell from 20.08 ms to 1.06 ms, about 19× faster.
Application gains are smaller because list construction, calibration and indexed Python arithmetic still contribute.

## Operation benchmarks

`benches/lists.rs` measures list construction, numeric reductions, Python iteration, copies, slices,
repetition, indexing, replacement and promotion at 16, 4,096 and 1,000,000 elements.
Nullable integers, mixed numbers and strings serve as generic-storage controls.

The storage hypothesis is that eight-byte numeric slots reduce retained memory and bulk-copy traffic.
The operation hypothesis is that an integer reduction can avoid materializing and adding a `Value` per element.
A Python `for` loop still dispatches bytecode and arithmetic, so compact storage alone should change it much less.
A first incompatible mutation must allocate and copy a generic buffer; its cost should grow with list length.

Each timed operation uses a fresh REPL with a compiled function and a prebuilt list.
Setup and input destruction are outside the timer; host call dispatch and scalar result export are inside it.
Construction cases build their lists inside the timed function.
The `len_int` case measures the fixed call overhead.
Fresh sessions prevent mutating cases from measuring an already-promoted list on subsequent iterations.

Copy `benches/lists.rs` and its `[[bench]]` target from `Cargo.toml` into the unspecialized revision first.
Run the same harness on that revision to record a baseline, then compare on the specialized revision:

```sh
cargo bench -p monty-bench --bench lists --features worker-alloc -- --save-baseline lists-before
cargo bench -p monty-bench --bench lists --features worker-alloc -- --baseline lists-before
cargo bench -p monty-bench --bench lists --features worker-alloc -- --memory
```

Set `PYO3_PYTHON` if the benchmark crate cannot locate the development Python interpreter.
The timing harness uses ten samples, a 100 ms warm-up and a 300 ms measurement target per case.
The worker allocator counts allocations but has no hard ceiling armed for timings.
`--memory` reports retained bytes for the whole session, including interpreter tables and source text.
It does not measure transient peaks during construction or promotion.
Criterion stores timing samples and estimates under `target/criterion/lists/`.

The implementation specializes immediate `i64` and `f64` elements, with generic fallback for other types.
Numeric copy, slice, concatenation and repetition keep typed storage.
Integer `sum()` uses an exact `i128` accumulator and checks the execution deadline every block of 64 elements.
Float `sum()` walks typed storage directly, retaining the existing left-to-right addition order.
Empty lists and unsupported start types use the generic reduction.
Snapshots serialize the list variants directly; dump version 14 replaces the previous sequence-of-`Value` encoding.

## Local comparison

Measurements below compare the encapsulated `Vec<Value>` implementation against compact numeric storage
on the same Apple Silicon host on 2026-10-12, using the release profile and worker allocator.
They include the integer reduction and numeric bulk-operation paths, rather than storage alone.
These initial measurements precede the typed float reduction.

Mean timings for 1,000,000 elements:

| Operation | Before | After | Before / after |
| --- | ---: | ---: | ---: |
| Integer sum | 21.228 ms | 0.425 ms | 49.9× |
| Float sum | 20.076 ms | 19.497 ms | 1.03× |
| Python integer loop | 54.567 ms | 52.446 ms | 1.04× |
| Integer copy | 2.655 ms | 0.512 ms | 5.18× |
| Integer slice | 13.639 ms | 1.567 ms | 8.71× |
| Integer repetition | 8.742 ms | 1.055 ms | 8.29× |
| Build from range | 8.875 ms | 9.291 ms | 0.96× |
| Build by append | 47.282 ms | 45.815 ms | 1.03× |
| Append incompatible value | 0.024 ms | 0.584 ms | 0.04× |

The reduction and bulk operations improve substantially. Float reductions and Python loops change little.
Promotion is the main tradeoff: the first incompatible append copies the entire numeric buffer,
where the original generic list could use spare capacity. At 4,096 elements, range construction was
about 6.5% slower and mixed-list copying about 7.7% slower. These short runs do not establish small
improvements reliably; longer runs are appropriate before drawing conclusions about differences of a few percent.

Retained whole-session memory for 1,000,000 elements:

| Elements | Before | After |
| --- | ---: | ---: |
| Integers | 16,818,113 B | 8,040,641 B |
| Floats | 16,817,867 B | 8,429,259 B |
| Nullable integers | 16,817,881 B | 16,817,881 B |
| Mixed numbers | 16,817,891 B | 16,817,891 B |

The integer range constructor compacts capacity to the element count; the float comprehension retains
spare capacity. Nullable integers remain generic. A nullable specialization would need its own
benchmarks and a representation that distinguishes every integer from `None` without a sentinel collision.

Validation passed with memory-model checks: 869 interpreter tests, all 655 Monty fixtures and 93 worker
subprocess tests. Tests cover promotion failure without mutation, reference cleanup, cycles, active
iterators, numeric edge cases, deadline checks and snapshot compatibility. The new Python fixture also
passed CPython. The full CPython fixture run had eight failures in unchanged fixtures on the installed
Python 3.14.2, involving formatting, base64/binascii, search bounds and platform-specific time errors.
