# Benchmarks

The CodSpeed workflow measures wall time and memory allocations.
Normal pull requests run 12 interpreter benchmarks covering compilation, calls, attribute lookup, collections, JSON,
strings, garbage collection and session dumps.
The filter is `PR_BENCHMARK_FILTER` in [the workflow](../../.github/workflows/codspeed.yml).
Reduced runs build only the `main` target and skip the release worker used by pool benchmarks.

The full suite runs on pushes to `main`, manual workflow runs, and pull requests with the `Full Benchmarks` label.
Adding that label starts a full run immediately; further pushes keep running the full suite while the label is present.
Removing it restores reduced runs on subsequent pushes.
Other label changes do not start benchmark jobs.
Full runs include all benchmark targets and build the release worker.
They also maintain the main-branch baselines for the reduced suite, whose benchmark names and build settings are unchanged.

To run the reduced suite locally:

```bash
cargo codspeed build -p monty-bench --profile codspeed --features codspeed --bench main
cargo codspeed run -p monty-bench --bench main \
  '^(end_to_end|kitchen_sink|func_call_kwargs|attr_dispatch_minimal|list_append_str|dict_comp|json_loads|json_dumps|str_parse|gc_collect|session_dump|session_load)__monty$'
```

CI selects the measurement mode with `cargo codspeed build -m walltime` or `-m memory`.
