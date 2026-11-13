# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 15
- Parallel jobs: 1
- Run duration: 3m 11s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

match: Monty 1.0.1 · threaded: Monty 1.0.1 · match\_copied: Monty 1.0.1 · match\_shared: Monty 1.0.1 · threaded\_copied: Monty 1.0.1 · threaded\_shared: Monty 1.0.1 · CPython 3.15.0

- Reference: match
- Threshold: 1% and 0.01 ms
68 changes or failures; 53 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | match | threaded | match\_copied | match\_shared | threaded\_copied | threaded\_shared | CPython |
| --- | --- | --- | --- | --- | --- | --- | --- |
| game\_spectral\_norm | 319 / 319 | 312 / 313 | 262 / 260 | 260 / 260 | 266 / 268 | 267 / 268 | **67.6** / **66.8** |
| perf\_unpack\_sequence | 323 / 321 | 315 / 312 | 13.2 / 12.8 | **7.37** / **6.94** | 12.9 / 12.6 | **7.37** / **6.94** | 18.4 / 17.4 |
| policy\_scoring\_short | 0.79 / 0.58 | 0.82 / 0.56 | 0.78 / 0.54 | **0.76** / 0.52 | 0.77 / 0.53 | **0.76** / **0.51** | 1.43 / 0.56 |
| policy\_scoring\_batch | 8.71 / 8.28 | 8.24 / 7.83 | 7.07 / 6.58 | 7.01 / 6.51 | 6.91 / 6.30 | 6.91 / 6.31 | **5.05** / **4.06** |
| portfolio\_exposure | 4.84 / 4.59 | 4.36 / 4.14 | 1.68 / 1.41 | 1.66 / 1.42 | **1.64** / **1.40** | 1.66 / 1.42 | 2.44 / 1.54 |
| capacity\_planning | 30.9 / 30.2 | 29.0 / 28.6 | 29.8 / 29.1 | 29.8 / 29.1 | 29.1 / 29.1 | 29.2 / 29.1 | **9.73** / **8.76** |
| ticket\_search | 27.5 / 26.8 | 27.4 / 26.6 | 27.1 / 26.9 | 27.2 / 26.9 | 27.1 / 26.5 | 27.0 / 26.5 | **9.14** / **6.72** |
| perf\_float | 221 / 216 | 223 / 219 | 221 / 216 | 221 / 216 | 223 / 219 | 223 / 219 | **52.0** / **49.4** |


## Runtimes and build details

- **match**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/match-typed-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 86b9c52addc738fa9b9790e5e6e990573b19f04b8235cdf62745a9a12174cdaa

- **threaded**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/combined-typed-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 28686f0240b0b6b802d7c29491f4a9a8e16783b82c01e835f1326d98a8d85e64

- **match\_copied**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/match-typed-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 86b9c52addc738fa9b9790e5e6e990573b19f04b8235cdf62745a9a12174cdaa

- **match\_shared**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/match-typed-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 86b9c52addc738fa9b9790e5e6e990573b19f04b8235cdf62745a9a12174cdaa

- **threaded\_copied**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/combined-typed-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 28686f0240b0b6b802d7c29491f4a9a8e16783b82c01e835f1326d98a8d85e64

- **threaded\_shared**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/combined-typed-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 28686f0240b0b6b802d7c29491f4a9a8e16783b82c01e835f1326d98a8d85e64

- **CPython**: cpython; Python 3.15.0
  - Binary: /opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Versions/3.15/bin/python3.15
  - Build: Source revision and compiler flags unverified
  - SHA256: 15c72833c1ffc09bc4e05d1e1866784d126721ad548d9c4d8f943641262535cb
