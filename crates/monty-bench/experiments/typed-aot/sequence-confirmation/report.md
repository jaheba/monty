# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 15
- Parallel jobs: 1
- Run duration: 1m 14s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

baseline: Monty 1.0.1 · typed: Monty 1.0.1 · CPython 3.15.0

- Reference: baseline
- Threshold: 1% and 0.01 ms
14 changes or failures; 9 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | baseline | typed | CPython |
| --- | --- | --- | --- |
| perf\_unpack\_sequence | 320 / 321 | **13.7** / **13.2** | 18.6 / 17.5 |
| game\_spectral\_norm | 312 / 314 | 327 / 318 | **67.6** / **66.7** |
| capacity\_planning | 35.2 / 34.5 | 35.0 / 34.3 | **9.84** / **8.91** |
| perf\_float | 230 / 224 | 229 / 224 | **52.1** / **49.2** |
| ticket\_search | 27.6 / 27.0 | 27.4 / 26.8 | **9.47** / **6.80** |


## Runtimes and build details

- **baseline**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/sequence-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: adaf0f707b4387f2fcc60b66bef96cad3334c7f3974e96e31cd818e0c2153ea4

- **typed**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/sequence-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: adaf0f707b4387f2fcc60b66bef96cad3334c7f3974e96e31cd818e0c2153ea4

- **CPython**: cpython; Python 3.15.0
  - Binary: /opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Versions/3.15/bin/python3.15
  - Build: Source revision and compiler flags unverified
  - SHA256: 15c72833c1ffc09bc4e05d1e1866784d126721ad548d9c4d8f943641262535cb
