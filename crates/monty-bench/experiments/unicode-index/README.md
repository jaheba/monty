# Lazy Unicode index experiments

These experiments compare lookup policies before changing Monty's string representation.
The standalone Rust harness copies the current `get_char_at_index` algorithm as its baseline.
It excludes VM dispatch, Python argument conversion and heap-object ownership costs.
The numbers below are algorithm costs, not predicted interpreter speedups.

## Reproduce

From the repository root, using a new output directory:

```sh
python3 crates/monty-bench/experiments/unicode-index/run.py --output playground/unicode-index-results
```

This builds with `rustc --edition=2024 -O`, validates all policies, runs the timing matrix, and measures RSS.
Only Rust and the Python standard library are required.
RSS collection supports Linux and macOS; other platforms still run the timing matrix.
Use `--rss-samples 0` to skip RSS or `--skip-matrix` to collect only memory samples.

`matrix.csv`, `memory.csv` and `provenance.json` contain the recorded run from October 30, 2026 on macOS arm64.
Each timing entry is the median of seven samples, rotating policy order between samples.
The samples include constructing and freeing the cache, but exclude creating the text and request positions.
Single-access samples batch 16 independent cache lifetimes; 16-access samples batch eight.
Other samples use one lifetime each.
RSS entries are medians of three independent processes.

## Matrix

The 22 policies cover cached character length, last-position cursors, sparse checkpoints, full indexes and non-ASCII
deltas.
Checkpoint spacing is 8, 32 or 128 code points; a full index stores every offset.
Fixed lazy triggers are 1, 8 or 64 nontrivial accesses.
Adaptive policies start with a cursor and build checkpoints after costly traversals total 1, 4, 16 or 64 times the text
size.
Traversals of at most eight characters do not count toward the adaptive budget.

Inputs contain 256, 4,096 or 65,536 code points.
Corpora cover ASCII, German prose, 10% Latin-1 accents, Chinese prose and 10% emoji.
German has about 1.9% non-ASCII code points and Chinese about 68%; exact counts accompany every result.
Each cache lifetime makes 1, 16, 256 or 4,096 requests.
Access patterns are uniform random, forward, reverse, nearby random walks, repeated positions and alternating endpoints.
Half the requests use negative indices.
All policies preserve direct access to `s[0]` and `s[-1]`; those accesses do not trigger an index or displace the
cursor.
ASCII strings never create an index.

The cursors retain the last nontrivial character position and byte position.
Forward movement uses `Chars::nth`; backwards movement is limited to 8 or 64 characters.
Long reverse traversals are avoided because reverse UTF-8 skipping is slower than forward skipping on this host.
Length is cached on demand, particularly to normalize negative indices.

## Lookup results

Nanoseconds per lookup for 4,096-character strings and 4,096 requests, including cache construction and cleanup:

| Policy | German random | German nearby | Chinese random | Index entries per string |
| --- | ---: | ---: | ---: | ---: |
| Current scan | 171.3 | 161.3 | 365.6 | 0 B |
| Cached length | 87.4 | 79.9 | 174.4 | 0 B |
| Cursor, up to 8 backwards | 96.3 | 17.6 | 161.2 | 0 B |
| Checkpoints every 8, after 8 accesses | 15.5 | 15.4 | 23.1 | 2,048 B |
| Checkpoints every 32, after 8 accesses | 34.1 | 32.7 | 46.6 | 512 B |
| Full index, after 8 accesses | 8.6 | 8.6 | 13.6 | 16,384 B |
| Cursor + adaptive checkpoints every 8, budget 16x | 22.5 | 16.7 | 28.9 | 2,048 B random; 0 B nearby |
| Cursor + adaptive checkpoints every 32, budget 16x | 51.6 | 16.3 | 61.3 | 512 B random; 0 B nearby |

The full index gives the fastest sustained random lookups but retains four bytes per code point in addition to UTF-8.
The cached-length gain in this matrix comes primarily from normalizing negative indices without recounting the text.
Checkpoints every eight characters use one eighth of that memory and recover much of the speed.
A last-position cursor is effective for sequential, reverse, nearby and repeated access, without a separate index
buffer.
Combining a cursor with a sparse index adds some bookkeeping to random lookup, but avoids building indexes for local
access.

Cold access changes the choice.
For 16 random accesses to German text, the baseline takes 137 ns/access, a cursor 74 ns, and fixed checkpoints every
eight 206 ns.
The full index takes 299 ns/access.
The adaptive 16x policy takes 75 ns/access because it has not yet built an index.
Building based only on the number of accesses is therefore a poor default.

`deltas_at8` stores `(character_position, cumulative_extra_bytes)` after each non-ASCII character.
It performs a binary search to turn a character position into a byte position.
This is compact for very sparse Unicode but becomes larger than the full index for dense Chinese.

## Memory

Fresh Rust processes retain 3,000 separately allocated 4,096-character strings and their cache states.
Each string is accessed 64 times with the random signed-index pattern.
These are prototype-process RSS values, not Monty-worker RSS values.

| German policy | Index entries per string | Peak RSS |
| --- | ---: | ---: |
| Current scan / cursor | 0 B | 17.42 MiB |
| Checkpoints every 32 | 512 B | 18.91 MiB |
| Checkpoints every 8 | 2,048 B | 23.30 MiB |
| Full index | 16,384 B | 64.48 MiB |

The table counts index buffers separately from metadata.
The harness uses the same 128-byte prototype state for all policies, including unused fields, to compare buffer costs.
That is not a proposed production layout, and cursor metadata is not free in a production object.
Actual string/arena layout and metadata allocation need measuring when integrating a selected policy.
All deferred policies retain no index entries after a single access in the memory runs.

## Search follow-up

The accompanying Flying Circus run uses the current Monty worker and CPython 3.14.2.
It searches German and Chinese runbook text with repeated incident identifiers.
Fixture preparation is outside request timing.
The full report is in `playground/mixed-search-results/report.html` in the task worktree.

Median repeated-request time for 100 calls on approximately 16K code points:

| Operation | German Monty | German CPython | Chinese Monty | Chinese CPython |
| --- | ---: | ---: | ---: | ---: |
| Successful `needle in text` | 0.05 ms | 0.11 ms | 0.06 ms | 0.12 ms |
| Successful `find()` | 1.46 ms | 0.12 ms | 4.87 ms | 0.12 ms |
| Failed `find()` | 1.61 ms | 0.62 ms | 5.10 ms | 0.57 ms |
| Bounded `find()` | 0.22 ms | 0.09 ms | 0.54 ms | 0.09 ms |
| `count()` | 1.55 ms | 0.62 ms | 5.12 ms | 0.68 ms |

Substring matching already runs directly on UTF-8 bytes.
Current `find()` and `count()` also count characters and walk the requested range before matching.
Unbounded searches should bypass those redundant walks before adding a cache.
A shared index could then accelerate character-to-byte bounds conversion and byte-to-character result conversion.
The search run does not yet implement or measure cached search paths.

## Candidate to integrate

Start with cached length and a last-position cursor, keeping the cheap endpoint paths.
Promote to a sparse index only after repeated expensive traversals; the 16x budget balances cold and sustained access
here.
Spacing eight is faster; spacing 32 reduces index entries by another factor of four.
The full index is useful as an upper bound on speed, rather than a default allocation.

The prototype's `u32` offsets are valid for its bounded inputs; larger production strings need a wider representation
or fallback.
Production caches also need allocator accounting and must remain derived state across cloning and snapshots.
Measure the selected policy through the VM before choosing defaults: the existing ASCII benchmark shows substantial
dispatch cost.
