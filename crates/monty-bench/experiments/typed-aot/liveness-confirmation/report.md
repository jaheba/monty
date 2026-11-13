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

default: Monty 1.0.1 · threaded: Monty 1.0.1 · before: Monty 1.0.1 · live: Monty 1.0.1 · match: Monty 1.0.1 · CPython 3.15.0

- Reference: default
- Threshold: 3% and 0.01 ms
46 changes or failures; 34 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | default | threaded | before | live | match | CPython |
| --- | --- | --- | --- | --- | --- | --- |
| capacity\_planning | 29.8 / 29.1 | 29.5 / 28.8 | 29.6 / 29.1 | 29.6 / 28.9 | 29.7 / 29.1 | **9.84** / **8.77** |
| game\_spectral\_norm | 312 / 305 | 319 / 318 | 239 / 239 | 238 / 238 | 251 / 252 | **67.7** / **66.8** |
| perf\_float | 218 / 213 | 224 / 218 | 226 / 221 | 224 / 218 | 219 / 214 | **51.9** / **49.4** |
| perf\_unpack\_sequence | 326 / 313 | 321 / 315 | **7.26** / **7.00** | 7.37 / 7.01 | 7.30 / **7.00** | 18.5 / 17.5 |
| policy\_scoring\_batch | 8.08 / 7.66 | 8.60 / 7.99 | 6.73 / 6.25 | 6.81 / 6.45 | 6.96 / 6.55 | **5.13** / **4.10** |
| policy\_scoring\_short | 0.78 / 0.55 | 0.81 / 0.55 | 0.79 / 0.52 | 0.80 / **0.50** | **0.77** / 0.52 | 1.49 / 0.56 |
| portfolio\_exposure | 4.59 / 4.42 | 4.40 / 4.11 | 1.71 / 1.47 | 1.64 / 1.40 | **1.59** / **1.38** | 2.49 / 1.59 |
| ticket\_search | 26.9 / 26.2 | 27.4 / 26.9 | 27.3 / 26.4 | 27.2 / 26.9 | 27.1 / 26.5 | **9.58** / **6.78** |


## Runtimes and build details

- **default**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/default-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 95519301f2db9abab0a02e0d8b2c69442b3395c4718d351cb847522ee856632f

- **threaded**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/liveness-combined-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: e9ab28dba72d38e030019d9985c9815ce386ea064dbae5e0885929afd591d344

- **before**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/final-combined-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: bd99f9980a33c73c11d0e280993b7e4fbdc8cc74ad42ca129fe36d09b3368ad0

- **live**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/liveness-combined-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: e9ab28dba72d38e030019d9985c9815ce386ea064dbae5e0885929afd591d344

- **match**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/liveness-match-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 085ae6b44d998b088a68f0d465fa70e1ff56600b87bbafa65c40610a033370a9

- **CPython**: cpython; Python 3.15.0
  - Binary: /opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Versions/3.15/bin/python3.15
  - Build: Source revision and compiler flags unverified
  - SHA256: 15c72833c1ffc09bc4e05d1e1866784d126721ad548d9c4d8f943641262535cb
