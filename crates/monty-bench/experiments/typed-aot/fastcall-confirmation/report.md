# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 15
- Parallel jobs: 1
- Run duration: 1m 1s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

baseline: Monty 1.0.1 · previous\_aot: Monty 1.0.1 · aot: Monty 1.0.1 · CPython 3.15.0

- Reference: baseline
- Threshold: 1% and 0.01 ms
23 changes or failures; 18 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | baseline | previous\_aot | aot | CPython |
| --- | --- | --- | --- | --- |
| game\_spectral\_norm | 322 / 320 | 271 / 267 | 258 / 256 | **67.6** / **66.9** |
| perf\_unpack\_sequence | 328 / 324 | **13.2** / **12.9** | 13.3 / **12.9** | 18.6 / 17.4 |
| policy\_scoring\_short | 0.80 / 0.56 | **0.78** / **0.55** | 0.79 / 0.56 | 1.44 / 0.57 |
| policy\_scoring\_batch | 8.87 / 8.45 | 6.96 / 6.58 | 7.14 / 6.69 | **5.17** / **4.41** |
| portfolio\_exposure | 4.89 / 4.70 | 4.79 / 4.58 | **1.65** / **1.44** | 2.48 / 1.59 |


## Runtimes and build details

- **baseline**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/fastcall-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 397795d38e7d99da29c73392c26b2de82383f66c6afce7fff91ff68a663c2031

- **previous\_aot**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/match-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 80dd74708ad2d506adbe91683e4b33287f5cd2efa978e1e21882e5a6f6fdb925

- **aot**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/fastcall-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 397795d38e7d99da29c73392c26b2de82383f66c6afce7fff91ff68a663c2031

- **CPython**: cpython; Python 3.15.0
  - Binary: /opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Versions/3.15/bin/python3.15
  - Build: Source revision and compiler flags unverified
  - SHA256: 15c72833c1ffc09bc4e05d1e1866784d126721ad548d9c4d8f943641262535cb
