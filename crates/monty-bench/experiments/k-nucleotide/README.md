# K-nucleotide slicing and lookup

`../../benches/python/k_nucleotide.py` generates 10,000 deterministic DNA bases and counts 69,961 windows of
length 1, 2, 3, 4, 6, 12 and 18.
It checks a checksum of ranked one/two-base frequencies and five query counts.
Input generation is part of the full benchmark's execution time.
The short fragments exercise reserved ASCII characters, inline strings and heap strings.

## Implementation

The VM recognizes adjacent `BuildSlice` and `BinarySubscr` instructions at runtime.
It coerces bounds in their original order, then consumes both instructions together.
Contiguous builtin strings copy their UTF-8 range without allocating a slice object.
Other receivers and steps receive an ordinary slice object through the existing subscript protocol.
The fused operation records the subscript instruction's position for errors.
Bytecode and snapshot formats stay compatible.

Dictionary and set probes first compare candidates using native equality, without preparing callback candidate buffers.
A pair that might run Python equality transfers to the existing guarded probe, which preserves comparison order,
revalidates entries after mutation, and polls resource limits on restarts.
The callback probe is outlined so its buffers stay outside the common native-key path.

Regression cases compare fused and explicit slices across ASCII/Unicode storage, negative and oversized bounds,
`__index__` callbacks, retained slice objects, failures, and external-call snapshot/resume.
Dictionary cases cover custom-key collisions, reverse equality, and clearing/replacing a table during comparison.

## Reproduction

The native benchmark harness registers the script automatically:

```sh
cargo bench -p monty-bench --bench main -- k_nucleotide
```

For request timing and whole-process peak RSS with saved production workers:

```sh
flying-circus --diff --threshold 1 --samples 50 --memory-samples 10 \
  -r before=/absolute/path/to/before -r optimized=/absolute/path/to/optimized \
  -r cpython=python3 --benchmark crates/monty-bench/benches/python/k_nucleotide.py
```

Flying Circus request timing includes workload compilation and execution, but excludes worker startup and session reset.
RSS comes from separate fresh-worker runs and includes startup; it is not exact live object storage.
Raw experiment outputs remain outside the PR.

## Measurements

macOS arm64, Rust 1.99.0 release with fat LTO, against the Unicode PR's saved production worker.
A 100-sample reversed-order check measured repeated-request medians:

| Workload                         |    Before | Optimized | CPython 3.14.2 |
| -------------------------------- | --------: | --------: | -------------: |
| Full k-nucleotide                | 24.276 ms | 22.077 ms |       9.939 ms |
| Count prebuilt 12-base fragments |  2.496 ms |  2.379 ms |       0.903 ms |
| Range-slicing control            |  1.350 ms |  1.327 ms |       1.045 ms |

The full benchmark improves by 9.1%; prebuilt-fragment counting improves by 4.7%.
The range-control difference is statistically inconclusive.
An earlier 50-sample sweep measured an 11.6% full-workload improvement, but absolute times shifted across all runtimes
while the machine was under load; the reversed-order check is the clearer timing comparison.
Ten separate peak-RSS samples measured approximately 14.2 MiB for both Monty builds and 20.0 MiB for CPython.

The slice-only change improved full-workload time by about 8% in its initial comparison.
The native probe added a further dictionary-counting gain of about 5–6% in the three-build comparison.
The first string-only dispatch guard regressed a range-slicing control by about 2%; fusing generic adjacent dispatch
removed the observed regression in the final sweep and recheck.
Callback-heavy keys and list/tuple/bytes slicing were also measured; their final timing comparisons were unchanged
or inconclusive, so these tests establish no general speedup for those workloads.

Library tests, dump compatibility, resource-limit tests, and targeted string/slice/dict/set differential tests pass.
The differential checks also cover lookup mutation, hash behavior and reference-count cleanup.
External-call slice bounds pass with reference-count checks and with snapshot round trips enabled.
