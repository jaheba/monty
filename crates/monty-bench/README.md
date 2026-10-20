# Benchmarks

The CodSpeed workflow measures wall time and memory allocations.
Normal pull requests run the six Rust-defined interpreter benchmarks: end-to-end execution, parsing, module cloning,
REPL feeds after many snippets, session dumps and session loads.
Python-script benchmarks run only in the full suite.
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
  '^(end_to_end|parse_1k_assigns|clone_module|repl_feed_after_2k_snippets|session_dump|session_load)__monty$'
```

CI selects the measurement mode with `cargo codspeed build -m walltime` or `-m memory`.
