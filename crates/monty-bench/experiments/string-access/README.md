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
