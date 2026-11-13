# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 15
- Parallel jobs: 1
- Run duration: 1m 59s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

match: Monty 1.0.1 · threaded: Monty 1.0.1 · match\_aot: Monty 1.0.1 · threaded\_aot: Monty 1.0.1 · CPython 3.15.0

- Reference: match
- Threshold: 1% and 0.01 ms
21 changes or failures; 7 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | match | threaded | match\_aot | threaded\_aot | CPython |
| --- | --- | --- | --- | --- | --- |
| perf\_unpack\_sequence | 348 / 342 | 316 / 313 | 13.3 / 13.0 | **13.2** / **12.9** | 18.5 / 17.6 |
| game\_spectral\_norm | 317 / 316 | 312 / 316 | 270 / 267 | 291 / 293 | **67.5** / **66.6** |
| capacity\_planning | 29.4 / 28.9 | 29.1 / 28.8 | 29.4 / 28.9 | 29.2 / 28.7 | **9.74** / **8.74** |
| perf\_float | 221 / 217 | 223 / 218 | 221 / 217 | 223 / 217 | **51.6** / **48.9** |
| ticket\_search | 27.1 / 27.2 | 27.0 / 26.7 | 27.2 / 26.9 | 27.3 / 26.6 | **9.51** / **6.93** |


## Runtimes and build details

- **match**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/match-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 80dd74708ad2d506adbe91683e4b33287f5cd2efa978e1e21882e5a6f6fdb925

- **threaded**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/combined-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: dd25a44f3f559f6b9fa5418c485e81b29c01d9ceb6d6720103ec528006e217a2

- **match\_aot**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/match-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 80dd74708ad2d506adbe91683e4b33287f5cd2efa978e1e21882e5a6f6fdb925

- **threaded\_aot**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/combined-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: dd25a44f3f559f6b9fa5418c485e81b29c01d9ceb6d6720103ec528006e217a2

- **CPython**: cpython; Python 3.15.0
  - Binary: /opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Versions/3.15/bin/python3.15
  - Build: Source revision and compiler flags unverified
  - SHA256: 15c72833c1ffc09bc4e05d1e1866784d126721ad548d9c4d8f943641262535cb
