# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 15
- Parallel jobs: 1
- Run duration: 2m 44s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

threaded: Monty 1.0.1 · typed: Monty 1.0.1 · previous\_typed: Monty 1.0.1 · match: Monty 1.0.1 · match\_typed: Monty 1.0.1 · CPython 3.15.0

- Reference: threaded
- Threshold: 1% and 0.01 ms
50 changes or failures; 32 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | threaded | typed | previous\_typed | match | match\_typed | CPython |
| --- | --- | --- | --- | --- | --- | --- |
| game\_spectral\_norm | 317 / 318 | 238 / 239 | 268 / 271 | 311 / 312 | 252 / 251 | **67.7** / **66.7** |
| perf\_unpack\_sequence | 318 / 315 | 7.34 / 6.91 | 7.31 / 7.00 | 318 / 317 | **7.30** / **6.88** | 18.5 / 17.5 |
| policy\_scoring\_short | 0.85 / 0.56 | 0.80 / **0.52** | 0.77 / **0.52** | 0.79 / 0.55 | **0.74** / **0.52** | 1.36 / 0.56 |
| policy\_scoring\_batch | 8.50 / 8.06 | 6.91 / 6.54 | 6.78 / 6.39 | 8.52 / 8.18 | 7.10 / 6.62 | **5.09** / **4.18** |
| portfolio\_exposure | 4.43 / 4.10 | 1.68 / **1.41** | **1.65** / 1.42 | 4.75 / 4.46 | 1.67 / 1.43 | 2.52 / 1.58 |
| capacity\_planning | 29.6 / 28.9 | 29.7 / 28.8 | 29.3 / 30.2 | 28.8 / 28.2 | 29.9 / 29.1 | **9.83** / **8.75** |
| ticket\_search | 27.2 / 26.7 | 27.4 / 26.9 | 27.2 / 26.8 | 27.1 / 26.5 | 27.0 / 26.5 | **9.74** / **6.97** |
| perf\_float | 221 / 216 | 222 / 216 | 223 / 219 | 218 / 212 | 219 / 214 | **51.9** / **49.8** |


## Runtimes and build details

- **threaded**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/combined-budget-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 1a683ea6d70bf975c665e662845cab0d1c2a3fb25a31443d3f7b0d8413c98360

- **typed**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/combined-budget-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 1a683ea6d70bf975c665e662845cab0d1c2a3fb25a31443d3f7b0d8413c98360

- **previous\_typed**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/combined-typed-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 28686f0240b0b6b802d7c29491f4a9a8e16783b82c01e835f1326d98a8d85e64

- **match**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/compact-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 653926f768d052fffb4274e4b34036cc423d569f32a137f9e75c738727d3194f

- **match\_typed**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/compact-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 653926f768d052fffb4274e4b34036cc423d569f32a137f9e75c738727d3194f

- **CPython**: cpython; Python 3.15.0
  - Binary: /opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Versions/3.15/bin/python3.15
  - Build: Source revision and compiler flags unverified
  - SHA256: 15c72833c1ffc09bc4e05d1e1866784d126721ad548d9c4d8f943641262535cb
