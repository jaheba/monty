# Reproducing the study

Branch `typed-aot-study` contains the prototype source and this study's measurements.
The implementation commit is `c5f6c0e2`; the separate results commit contains the benchmark and audit evidence.
Both features are opt-in. The user's existing source checkout was preserved.
Compiled executables remain in the isolated temporary study; their hashes and build metadata are retained here.

Check out this branch directly to inspect or build it. Extract `dependency-source.tar.gz` beside the checkout,
creating its sibling `spam/` directory. This is the archived tokspam revision recorded in `dependency-source.json`,
with its package/library renamed to match the baseline. Hashes identify the exact dependency contents.
The separate MSRV JSON-cache shim is not part of the main implementation; normal builds used Rust 1.99,
and Rust 1.96 checks used the documented isolated shim.

The frozen `guarded-match-artifacts/source.patch` and `guarded-combined-artifacts/source.patch` apply to baseline
commit `9a9f8784bf6cb36b09f183b53d7d829bcd807bdf`, not on top of this branch. They retain the exact measured source.

Run `make install-py` in the isolated checkout before building. Build `monty-runtime` in release mode with
`--features experimental-aot` for match dispatch, or `--features experimental-aot,experimental-threaded` for
combined dispatch. Run the resulting CLI with `--aot program.py`; this performs strict type validation.
`--native-stats` reports coverage, `--aot-copied` chooses copied templates, and `--aot --no-native` keeps
validation while disabling both optimized execution tiers.

The experiment's Python build/benchmark helpers assume the isolated study's directory layout and installed
Flying Circus client. Do not run them against the user's dirty checkout. Each report records executable hashes,
arguments and raw samples; artifact directories include compiler/profile metadata, source hashes and patches.
`session.json` names the eight-hour window. `WORKLOG.md` preserves intermediate results and failed experiments.
