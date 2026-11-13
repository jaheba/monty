# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 15
- Parallel jobs: 1
- Run duration: 2m 15s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

default: Monty 1.0.1 · before: Monty 1.0.1 · match: Monty 1.0.1

- Reference: default
- Threshold: 3% and 0.01 ms
22 changes or failures; 5 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | default | before | match |
| --- | --- | --- | --- |
| portfolio\_risk | **56.8** / **56.8** | 60.0 / 59.8 | 58.5 / 58.5 |
| text\_csv\_cleanup\_medium | **669** / **663** | 693 / 688 | 698 / 693 |
| perf\_richards | **171** / **171** | 179 / 179 | 180 / 179 |
| rustpython\_pystone | **283** / **283** | 290 / 293 | 297 / 298 |
| rustpython\_micro\_cmp | **1.79** / **1.75** | 2.03 / 1.92 | 1.98 / 1.92 |
| rustpython\_micro\_sorted\_900 | **1.01** / **0.95** | 1.04 / 0.98 | 1.08 / 1.01 |
| stdout\_lines | **2.20** / **1.90** | 2.29 / 1.98 | 2.27 / 1.97 |


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
