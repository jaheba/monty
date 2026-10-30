# Production string access comparison

`run.py` compares Monty executables and CPython through Flying Circus. Each fixture validates its output.
Pass existing binaries with distinct labels; the script does not build or overwrite executables.

```sh
python3 crates/monty-bench/experiments/string-access/run.py \
  --runtime baseline=/absolute/path/to/baseline \
  --runtime sparse8=/absolute/path/to/sparse8 \
  --runtime sparse32=/absolute/path/to/sparse32 \
  --runtime cpython=/absolute/path/to/python \
  --output playground/string-access-comparison
```

The default is 30 timing samples and five separate peak-RSS samples, with serial measurement.
The output contains generated fixtures, executable hashes, the exact command, raw results and HTML/JSON/Markdown reports.
Timing includes compilation and execution of the benchmark body. Fixture preparation is excluded.
RSS includes the whole fresh worker, fixture preparation, retained objects and temporary allocations.

The access matrix uses 16,384-character strings: ASCII prose, German prose, 10% Latin-1 accents, Chinese mixed with
ASCII metadata, and 10% emoji. Cold access makes 16 lookups; random, nearby and endpoint patterns make 4,096.
Half the non-endpoint lookups use negative indexes. Nearby reads advance within a small region, allowing the cursor
to avoid checkpoint construction.

The memory cases retain 3,000 independently constructed strings of about 4,096 characters and access each 64 times.
They expose checkpoint storage and cursor metadata costs rather than hiding them behind one shared string.

Search cases use 16 Ki characters with an ASCII incident identifier inserted every 1,024 characters. They cover
membership, successful and unsuccessful find, bounded find, rfind, counts, empty-needle counts and finding every
occurrence with successive start positions.

Four Flying Circus agent workloads parse about 1.2 MiB of JSON, filter and rank support records, extract excerpts,
and produce a report with source URLs. These compare German transliteration, German, German with rare emoji, and
Chinese. The mixed-string indexing microbenchmarks in `benches/python/` are also included.

The standalone `../unicode-index/` experiment compares full indexes and additional policies without VM costs.
Production uses a cursor and adaptive sparse checkpoints; the full index remains an experimental speed ceiling.

## Cache policy

Heap strings and interned literals cache their character length and last accessed character/byte boundary.
ASCII strings and the first/last character bypass the Unicode cursor. Short backwards movements use up to eight
UTF-8 characters; other movements use forward skipping from the best available checkpoint or cursor.

The production policy places one checkpoint every 32 code points. A checkpoint stores a four-byte offset for strings
up to 4 GiB; larger strings use native-width offsets. The compact buffer costs about 0.125 bytes per code point,
plus the lazily allocated cursor state. The heap slot size remains unchanged.

Only traversals longer than eight characters count toward index construction. Construction starts after accumulated
scanning reaches 32 times the UTF-8 byte length. Sequential, nearby and endpoint reads therefore avoid checkpoint
buffers. Buffers are preflighted against the resource tracker, and all cache state is omitted from snapshots and reset
on clone. Session-local cache updates use `Cell`/`RefCell`; the shared core-string templates contain only immutable
prehashed text.

## Initial VM comparison

The first comparison used a 16x scan budget for both spacing 8 and 32, with 30 timing samples and five RSS samples.
Spacing 8 was 10–22% faster on sustained random-access workloads, but used four times as much checkpoint storage.
The German and Chinese agent workloads were about 15% faster with either spacing.

The 3,000-string cases exposed premature index construction: at only 64 positive accesses per string, runtime rose
15–29%. Retained Latin-1 text used 25.7 MiB before the cache, 34.0 MiB with spacing 8, and 28.0 MiB with spacing 32.
Those are whole-process RSS medians, not exact object sizes or significance claims.

This prompted the later 32x trigger and removal of cache locking. The final run checks that tuning against the
original baseline and CPython. The initial spacing-32 build also added cached empty-needle counts; that case should
not be used to attribute a difference solely to checkpoint spacing.

## Final VM comparison

Measured October 30, 2026 on macOS arm64, Rust 1.99.0, the workspace release profile (fat LTO, one codegen unit),
and CPython 3.14.2. There are 20 timing samples per runtime and mode, three separate RSS samples, and three warmups.
The baseline already includes the earlier ASCII indexing and ASCII-run lowercase changes; it is not pristine
`origin/main`. All builds start from `d2ad8bbd` plus the uncommitted string work.

Repeated request medians in milliseconds:

| Workload | Before Unicode cache/search work | Tuned | CPython |
| --- | ---: | ---: | ---: |
| Mixed Unicode random access, 20,000 lookups | 19.565 | 3.547 | 2.132 |
| German random access, 4,096 lookups | 2.835 | 0.763 | 0.390 |
| Chinese random access, 4,096 lookups | 4.830 | 0.885 | 0.496 |
| German nearby access | 2.677 | 0.516 | 0.408 |
| Chinese nearby access | 4.307 | 0.554 | 0.469 |
| Chinese `find` hit, 100 calls | 3.211 | 0.082 | 0.136 |
| Chinese `find` miss, 100 calls | 3.371 | 0.281 | 0.433 |
| Chinese non-empty `count`, 100 calls | 3.381 | 0.267 | 0.647 |
| Chinese successive finds | 5.128 | 0.160 | 0.197 |
| German agent workload | 6.089 | 5.090 | 5.672 |
| German with rare emoji, agent workload | 6.174 | 5.088 | 6.401 |
| Chinese agent workload | 7.951 | 6.534 | 4.210 |

For 3,000 separately retained strings, one-shot peak RSS medians in MiB:

| Corpus | Before | Tuned | CPython |
| --- | ---: | ---: | ---: |
| German | 25.62 | 25.95 | 32.97 |
| 10% Latin-1 accents | 25.67 | 25.98 | 32.95 |
| Chinese mixed with ASCII | 34.17 | 34.52 | 49.27 |
| 10% emoji | 28.55 | 28.86 | 80.69 |

These 64-access cases retain cursor state but do not build checkpoint buffers. Their positive-only accesses remain
4–7% slower in repeated mode from bookkeeping: German 41.107→43.375 ms, Latin-1 41.862→43.985 ms, Chinese
53.341→55.403 ms, emoji 46.080→49.386 ms. This is a remaining tradeoff, despite removing premature index allocation.
The selected policy targets repeated random access while keeping moderate and local access memory small.

ASCII random-access medians were 2.675→2.608 ms and interned ASCII 2.507→2.441 ms. Cold 16-access timings are near
the process/protocol timing floor; consult raw samples and the diff classifications rather than treating small changes
as established wins. Three RSS samples describe measured medians, not statistical significance.

`policy-comparison.csv`, `final-comparison.csv` and `provenance.json` retain measurements and executable hashes.
The complete local reports are under `playground/string-access-comparison/results/` and
`playground/string-access-final/results/`. The frozen tuned module snapshot uses the equivalent pre-Clippy `match`
spelling of the out-of-range early return; the policy and lookup algorithm are the same as the final source.

## Affix checks and contiguous slices

`affix_slices.py` compares saved builds on 16,384-character ASCII, German and Chinese text.
Fixtures construct heap strings before the timed body.
Each affix case makes 100 calls, including hits, misses, tuples and bounded checks near the middle of the text.
Slice cases make 1,024 random or nearby 64-character excerpts, or 100 whole-string copies.
The four large agent workloads are included as application-level checks.

```sh
python3 crates/monty-bench/experiments/string-access/affix_slices.py \
  --runtime baseline=/absolute/path/to/unicode-cache-build \
  --runtime candidate=/absolute/path/to/affix-slice-build \
  --runtime cpython=/absolute/path/to/python \
  --output playground/affix-slice-comparison
```

The baseline for this comparison includes commit `c8e8d2a9`'s cache policy and search work.
The new paths reuse its character lengths and offsets for bounded affix checks and contiguous slices.
Unbounded affix checks compare UTF-8 directly.
Stepped slices retain their iterator-based implementation.

Measured October 30, 2026 with the same macOS arm64 toolchain as the preceding comparison, 30 timing samples and
three RSS samples per workload. Repeated request medians in milliseconds:

| Workload | Cache baseline | Affix/slice changes | CPython |
| --- | ---: | ---: | ---: |
| German startswith, 100 calls | 1.461 | 0.056 | 0.085 |
| Chinese startswith, 100 calls | 3.103 | 0.053 | 0.087 |
| Chinese endswith, 100 calls | 3.108 | 0.047 | 0.089 |
| German random 64-character slices, 1,024 calls | 1.868 | 0.480 | 0.259 |
| Chinese random 64-character slices, 1,024 calls | 2.747 | 0.517 | 0.277 |
| Chinese whole-string slices, 100 calls | 10.402 | 0.107 | 0.084 |
| German agent workload | 5.069 | 4.609 | 5.640 |
| German with rare emoji, agent workload | 5.157 | 4.588 | 6.158 |
| Chinese agent workload | 6.552 | 6.014 | 4.118 |

No candidate regressions or compatibility failures were detected across the 34 workloads.
ASCII slice differences were not established as improvements.
The whole-string slice comparison includes Monty's copy while CPython can reuse the original immutable string.
Consult `affix-slice-comparison.csv` and `affix-slice-provenance.json` for results and executable hashes.
The complete local report is `playground/affix-slice-comparison/results/report.html`.


## Inline cursor comparison

The inline variant replaces `RefCell<Option<Box<State>>>` with inline `Cell` fields for the cursor and scan counter,
plus `OnceCell<Box<Offsets>>` for the immutable checkpoint table.
Reads before checkpoint construction allocate no cache storage and perform no runtime borrow checks.
Checkpoint spacing and the 32x scan budget are unchanged.
The 64-bit layout uses 40 bytes of cache metadata and a 72-byte string payload; the existing 80-byte heap-slot limit
still passes. Interned entries gain inline metadata, so retained-memory measurements include that tradeoff.

Use repeated `--only` filters to select the access, memory and agent cases from `run.py`:

```sh
python3 crates/monty-bench/experiments/string-access/run.py \
  --runtime boxed=/absolute/path/to/affix-slice-build \
  --runtime inline=/absolute/path/to/inline-cache-build \
  --runtime uncached=/absolute/path/to/pre-cache-build \
  --runtime cpython=/absolute/path/to/python \
  --only 'access_*' --only 'memory_index_*' --only 'agent_unicode_*' --only 'string_*' \
  --samples 30 --memory-samples 0 --output playground/inline-cache-timing
```

For RSS, select only `memory_index_*` with `--samples 10 --memory-samples 10` and a new output directory.
The `uncached` build predates Unicode caching, affix and slicing optimizations; use it as a memory-case reference,
not an isolated layout comparison for the agent workloads.


The experiment remains in `inline-cursor.patch`; the production source keeps the boxed cache.
Apply the patch with `git apply crates/monty-bench/experiments/string-access/inline-cursor.patch` before building the
inline candidate. It applies to the documented boxed implementation in this branch.

Measured October 30, 2026 on macOS arm64 with the same release toolchain.
The timing run uses 30 samples per runtime and mode across 33 workloads.
The retained-memory run uses ten timing samples and ten fresh-process RSS samples for each of five cases.
All cases validate their output; neither run had compatibility or measurement failures.

Repeated request medians, milliseconds:

| Workload | Boxed cache | Inline cache | Before Unicode caching |
| --- | ---: | ---: | ---: |
| 3,000 German strings, 64 accesses each | 42.743 | 42.491 | 40.753 |
| 3,000 strings with 10% Latin-1, 64 accesses each | 43.563 | 43.528 | 41.439 |
| 3,000 Chinese strings, 64 accesses each | 54.603 | 55.912 | 52.964 |
| 3,000 strings with 10% emoji, 64 accesses each | 48.635 | 48.543 | 45.504 |
| Interned ASCII random access | 2.406 | 2.494 | 2.403 |
| Mixed Unicode random access | 3.371 | 3.459 | 19.394 |
| Chinese random access | 0.801 | 0.827 | 4.703 |
| German agent workload | 4.513 | 4.473 | 5.878 |
| Chinese agent workload | 5.898 | 5.882 | 7.728 |

The inline variant has established regressions in Chinese random access (about 3%), Chinese retained-string access
(about 2.4%), interned ASCII access (3.6%), and mixed Unicode random access (2.6%).
The moderate German, Latin-1 and emoji timing differences do not establish a general runtime improvement.
The extra cost relative to the uncached 64-access cases therefore remains.

One-shot whole-process RSS medians, MiB, from ten independent samples:

| Corpus | Boxed cache | Inline cache | Before Unicode caching | CPython |
| --- | ---: | ---: | ---: | ---: |
| ASCII | 25.703 | 25.562 | 25.641 | 32.953 |
| German | 25.945 | 25.594 | 25.641 | 32.961 |
| 10% Latin-1 | 25.945 | 25.578 | 25.656 | 32.922 |
| Chinese | 34.445 | 34.125 | 34.172 | 49.234 |
| 10% emoji | 28.844 | 28.484 | 28.562 | 80.641 |

Removing the separate cursor allocation saves about 0.32–0.37 MiB for the retained Unicode strings.
The German, Latin-1 and emoji RSS improvements exceed the harness's 1% significance threshold; the Chinese change
is below that threshold. These are whole-process peaks rather than exact live cache sizes.
The small RSS benefit does not justify the observed runtime regressions for the default policy.
The measurements do not isolate the CPU cause; larger payloads and interned entries are possible costs to profile.

`inline-cursor-timing.csv`, `inline-cursor-rss.csv` and `inline-cursor-provenance.json` retain the evidence.
Complete local reports are `playground/inline-cache-timing/results/report.html` and
`playground/inline-cache-rss/results/report.html`.
