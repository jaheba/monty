# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 15
- Parallel jobs: 1
- Run duration: 1m 51s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

default: Monty 1.0.1 · before: Monty 1.0.1 · match: Monty 1.0.1 · combined: Monty 1.0.1 · CPython 3.15.0

- Reference: default
- Threshold: 3% and 0.01 ms
50 changes or failures; 9 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | default | before | match | combined | CPython |
| --- | --- | --- | --- | --- | --- |
| capacity\_planning | 29.7 / 29.1 | 29.7 / 29.1 | 29.6 / 28.9 | 29.6 / 29.0 | **9.82** / **8.83** |
| dictionary\_long\_keys | 0.56 / 0.38 | 25.2 / 25.0 | **0.45** / **0.24** | **0.45** / 0.25 | 1.12 / 0.29 |
| game\_spectral\_norm | 312 / 305 | 252 / 252 | 252 / 253 | 245 / 240 | **67.5** / **66.8** |
| perf\_float | 217 / 212 | 219 / 214 | 222 / 218 | 225 / 221 | **51.6** / **49.3** |
| perf\_unpack\_sequence | 325 / 312 | **7.05** / 6.76 | 7.08 / **6.73** | **7.05** / 6.78 | 18.6 / 17.4 |
| policy\_scoring\_batch | 8.08 / 7.66 | 6.85 / 6.56 | 6.55 / 5.98 | 6.47 / 5.95 | **5.16** / **3.97** |
| policy\_scoring\_short | 0.77 / 0.54 | 0.79 / 0.52 | 0.75 / **0.50** | **0.74** / 0.51 | 1.40 / 0.55 |
| portfolio\_exposure | 4.56 / 4.35 | 1.59 / **1.33** | 1.58 / **1.33** | **1.57** / 1.34 | 2.45 / 1.57 |
| ticket\_search | 26.9 / 25.9 | 26.7 / 26.6 | 26.7 / 26.4 | 27.1 / 27.1 | **9.43** / **6.97** |


## Runtimes and build details

- **default**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/default-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 95519301f2db9abab0a02e0d8b2c69442b3395c4718d351cb847522ee856632f

- **before**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/unified-match-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: b43a25c6400d190a0c861a4e0d6c25740821a7ee69d2c17f6f0bca3f7815fc58

- **match**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/guarded-match-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: e57f05511b42b70a1d18967ac3c75c5b96e4c78f86c315c47ef77071a5044761

- **combined**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/guarded-combined-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 9adfb37553b3233adbd8f82c7a3e2147eb64b4f86515f607a4647ac77a1cd882

- **CPython**: cpython; Python 3.15.0
  - Binary: /opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Versions/3.15/bin/python3.15
  - Build: Source revision and compiler flags unverified
  - SHA256: 15c72833c1ffc09bc4e05d1e1866784d126721ad548d9c4d8f943641262535cb
