# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 3
- Parallel jobs: 1
- Run duration: 8m 43s
- Repeated warmups: 1
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

baseline: Monty 1.0.1 · native: Monty 1.0.1

Quick run: reduced sampling for a rough comparison. Repeat with default settings to confirm changes.

- Reference: baseline
- Threshold: 1% and 0.01 ms
9 changes or failures; 239 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | baseline | native |
| --- | --- | --- |
| game\_pidigits | failed / failed | failed / failed |
| rustpython\_nqueens | failed / failed | failed / failed |
| rustpython\_regex\_effbot | failed / failed | failed / failed |


## Peak RSS · MiB · one-shot

| Workload | baseline | native |
| --- | --- | --- |
| game\_pidigits | failed | failed |
| rustpython\_nqueens | failed | failed |
| rustpython\_regex\_effbot | failed | failed |


## Failures

- baseline / game\_pidigits / one-shot: ["TypeError: 'generator' object is not an iterator"]
- baseline / rustpython\_nqueens / one-shot: ["TypeError: 'generator' object is not an iterator"]
- baseline / rustpython\_regex\_effbot / one-shot: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- native / game\_pidigits / one-shot: ["TypeError: 'generator' object is not an iterator"]
- native / rustpython\_nqueens / one-shot: ["TypeError: 'generator' object is not an iterator"]
- native / rustpython\_regex\_effbot / one-shot: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- baseline / game\_pidigits / repeated: ["TypeError: 'generator' object is not an iterator"]
- baseline / rustpython\_nqueens / repeated: ["TypeError: 'generator' object is not an iterator"]
- baseline / rustpython\_regex\_effbot / repeated: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- native / game\_pidigits / repeated: ["TypeError: 'generator' object is not an iterator"]
- native / rustpython\_nqueens / repeated: ["TypeError: 'generator' object is not an iterator"]
- native / rustpython\_regex\_effbot / repeated: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]

## Runtimes and build details

- **baseline**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/native-regions/flying-circus-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: bfdfc25c707f9b92f6b10a741bec598364bf08fe6e041dbdbb745c8568a3c1d3

- **native**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/native-regions/flying-circus-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: bfdfc25c707f9b92f6b10a741bec598364bf08fe6e041dbdbb745c8568a3c1d3
