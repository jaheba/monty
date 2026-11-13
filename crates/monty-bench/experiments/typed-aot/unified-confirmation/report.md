# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 15
- Parallel jobs: 1
- Run duration: 2m 28s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

before: Monty 1.0.1 · combined: Monty 1.0.1 · match\_before: Monty 1.0.1 · match: Monty 1.0.1 · default: Monty 1.0.1 · CPython 3.15.0

- Reference: before
- Threshold: 3% and 0.01 ms
29 changes or failures; 21 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | before | combined | match\_before | match | default | CPython |
| --- | --- | --- | --- | --- | --- | --- |
| capacity\_planning | 29.7 / 29.0 | 29.7 / 29.2 | 29.9 / 29.1 | 29.7 / 29.0 | 29.7 / 29.1 | **9.82** / **8.76** |
| game\_spectral\_norm | 245 / 238 | 238 / 239 | 252 / 251 | 253 / 254 | 312 / 305 | **67.5** / **67.1** |
| perf\_float | 222 / 220 | 224 / 220 | 218 / 214 | 219 / 214 | 216 / 211 | **51.9** / **49.2** |
| perf\_unpack\_sequence | 7.33 / 6.95 | **7.05** / 6.88 | 7.29 / 6.94 | 7.09 / **6.79** | 324 / 313 | 18.6 / 17.5 |
| policy\_scoring\_batch | 6.80 / 6.48 | 6.82 / 6.30 | 6.88 / 6.42 | 6.95 / 6.61 | 8.03 / 7.66 | **5.10** / **4.09** |
| policy\_scoring\_short | **0.76** / 0.53 | **0.76** / **0.51** | 0.77 / **0.51** | 0.79 / 0.53 | **0.76** / 0.55 | 1.43 / 0.56 |
| portfolio\_exposure | 1.61 / 1.41 | **1.57** / 1.37 | **1.57** / 1.38 | 1.66 / **1.33** | 4.63 / 4.34 | 2.37 / 1.56 |
| ticket\_search | 27.4 / 27.0 | 27.3 / 27.1 | 27.2 / 26.7 | 26.6 / 26.2 | 27.3 / 26.1 | **9.68** / **7.02** |


## Runtimes and build details

- **before**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/liveness-combined-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: e9ab28dba72d38e030019d9985c9815ce386ea064dbae5e0885929afd591d344

- **combined**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/unified-combined-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 56726a215c9695ca7f5d1fd698826f8cebc84d834d2632f7c849d77a64604349

- **match\_before**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/liveness-match-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 085ae6b44d998b088a68f0d465fa70e1ff56600b87bbafa65c40610a033370a9

- **match**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/unified-match-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: b43a25c6400d190a0c861a4e0d6c25740821a7ee69d2c17f6f0bca3f7815fc58

- **default**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/default-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 95519301f2db9abab0a02e0d8b2c69442b3395c4718d351cb847522ee856632f

- **CPython**: cpython; Python 3.15.0
  - Binary: /opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Versions/3.15/bin/python3.15
  - Build: Source revision and compiler flags unverified
  - SHA256: 15c72833c1ffc09bc4e05d1e1866784d126721ad548d9c4d8f943641262535cb
