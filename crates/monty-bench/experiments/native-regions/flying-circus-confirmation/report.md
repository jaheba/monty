# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 15
- Parallel jobs: 1
- Run duration: 3m 40s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

baseline: Monty 1.0.1 · native: Monty 1.0.1

- Reference: baseline
- Threshold: 1% and 0.01 ms
13 changes or failures; 3 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | baseline | native |
| --- | --- | --- |
| capacity\_planning | **28.9** / **28.4** | 160 / 160 |
| text\_csv\_cleanup\_medium | **704** / **699** | 3850 / 3820 |
| ticket\_search | **27.3** / **26.5** | 102 / 101 |
| perf\_float | **221** / **215** | 1030 / 1030 |
| rustpython\_micro\_cmp | 1.98 / 1.88 | **0.95** / **0.88** |
| expense\_report | **0.37** / **0.18** | 0.42 / 0.21 |
| text\_json\_transform\_small | **0.57** / **0.32** | 0.65 / 0.43 |


## Runtimes and build details

- **baseline**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/native-regions/flying-circus-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: bfdfc25c707f9b92f6b10a741bec598364bf08fe6e041dbdbb745c8568a3c1d3

- **native**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/native-regions/flying-circus-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: bfdfc25c707f9b92f6b10a741bec598364bf08fe6e041dbdbb745c8568a3c1d3
