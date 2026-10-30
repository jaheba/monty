# String access benchmarks

These scripts compare existing Monty executables and CPython through Flying Circus.
Use a Flying Circus checkout containing the `agent_unicode_*` workloads and distinct labels for saved builds.
The scripts validate workload outputs and write fixtures, executable hashes, timings, RSS samples and reports to a new
output directory. Generated results are kept outside version control.

```sh
python3 crates/monty-bench/experiments/string-access/run.py \
  --circus /path/to/flying-circus \
  --runtime baseline=/path/to/baseline/monty \
  --runtime candidate=/path/to/candidate/monty \
  --runtime cpython=/path/to/python \
  --output playground/string-access-comparison
```

`run.py` covers ASCII, German, Chinese, 10% Latin-1 and 10% emoji strings.
It measures cold, random, nearby and endpoint reads, plus search operations and retained-string memory.
Memory cases retain 3,000 distinct strings of about 4,096 characters, with 64 accesses per string.
Four agent workloads parse about 1.2 MiB of JSON, filter and rank support records, then produce excerpts and source URLs.
Use repeated `--only` patterns to select workload stems, such as `--only 'memory_index_*'`.

`affix_slices.py` accepts the same runtime, checkout and output arguments.
It measures affix hits, misses, tuples and bounded checks on 16,384-character strings, plus 64-character and whole-string
slices. It also includes the four agent workloads.

Request timing includes compilation and execution of the benchmark body, with fixture preparation excluded.
RSS is a fresh worker's whole-process peak, including preparation, retained values and temporary allocations.
Runs are serial; avoid simultaneous builds or benchmarks.

## Policy and findings

The production cache stores Unicode character length and the last character/byte position.
Only traversals longer than eight characters count toward constructing sparse checkpoints.
Checkpoints are spaced 32 code points apart and built after scanning 32 times the UTF-8 byte length.
Compact offsets cost about 0.125 bytes per character; strings too large for u32 offsets use native-width offsets.
ASCII, endpoints and short local reads avoid checkpoint construction.

A stride of eight was 10–22% faster on sustained random access but used four times the checkpoint storage.
Agent workloads performed similarly with either spacing.
An earlier 16x scan budget built indexes too soon for 64-access strings, causing 15–29% runtime regressions.
The selected policy avoids that allocation, though positive-only 64-access cases retain about 4–7% bookkeeping overhead.

An inline cursor layout removed cursor allocations and runtime borrow checks, saving about 0.35 MiB across 3,000 Unicode
strings. It regressed several access workloads by 2–4%, so the boxed layout remains the default.
The measurements do not isolate the CPU cause of those regressions.

Representative repeated-request medians, measured October 30, 2026 on macOS arm64 with Rust 1.99.0, fat LTO and 30 samples:

| Workload | Saved baseline | Optimized |
| --- | ---: | ---: |
| Mixed Unicode random access, 20,000 lookups | 19.394 ms | 3.371 ms |
| Chinese startswith, 100 calls | 3.103 ms | 0.053 ms |
| Chinese 64-character slices, 1,024 calls | 2.747 ms | 0.517 ms |
| German agent workload | 5.878 ms | 4.513 ms |
| Chinese agent workload | 7.728 ms | 5.898 ms |

These are staged comparisons: random-access and agent baselines precede Unicode caching but already include earlier
ASCII indexing and lowercase changes; affix and slice baselines include the Unicode cache.
The standalone `../unicode-index/` harness compares additional index policies without VM costs.
