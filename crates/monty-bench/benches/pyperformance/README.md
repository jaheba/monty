# pyperformance workloads

These workloads come from [pyperformance](https://github.com/python/pyperformance) at
`ccc0aeb7ad46d65b6dcd4160e0fdda4d885852dd`, under the MIT license in `LICENSE`.
The scripts live in `../python/pyperformance_*.py`.
The harness discovers these top-level Python files automatically and registers `<stem>__monty` and `<stem>__cpython`.
Both interpreters execute the same adapted source and validate the same assertions.

Run all pairs with:

```bash
cargo bench -p monty-bench --bench main -- pyperformance
```

Append `--test` to check results without collecting timing samples.
CPython cases run locally; CodSpeed simulation runs only the Monty cases.
The workloads use the main harness's standard Criterion sampling settings.
Each script checks its own result with `assert`; the harness does not depend on its final expression.

To override sampling settings:

```bash
cargo bench -p monty-bench --bench main -- pyperformance --sample-size 100 --warm-up-time 3 --measurement-time 10
```

| Workload        | Size                                            |
| --------------- | ----------------------------------------------- |
| fannkuch        | n = 6                                           |
| spectral_norm   | 24 elements, 10 power iterations                |
| nbody           | 500 steps                                       |
| barnes_hut      | 50 particles, 1 step, theta 0.5                 |
| float           | 3,000 points                                    |
| unpack_sequence | 400 tuple unpackings and 400 list unpackings    |
| json_dumps      | All four upstream cases and their repeat counts |
| json_loads      | All three upstream fixtures, 20 parses each     |
| gc_traversal    | 500 levels, two collection calls                |

The `pyperf` runner, command-line functions and internal timers are removed.
Criterion performs repetition, so each invocation of a loop-based workload runs one outer iteration.
Compilation happens before timing, but module definitions and fixture construction run on every invocation,
following the existing `main` benchmark harness.
This includes setup that upstream pyperformance excludes from its timed region; results are not directly comparable
with upstream pyperformance timings.

Numeric workloads check their computed values with tolerances.
Sequence unpacking checks a checksum, and JSON loads checks the decoded fixtures.
The float workload changes `class Point(object)` to `class Point` because Monty rejects explicit base classes.
Fannkuch replaces cached bound methods with direct calls and slice assignment with indexed writes.
The heavier workloads use reduced input sizes to target less than 100 ms of simulated execution in CodSpeed.
The algorithms otherwise remain unchanged; Monty and CPython use the same sizes.
This target is provisional until confirmed by a CodSpeed run.
