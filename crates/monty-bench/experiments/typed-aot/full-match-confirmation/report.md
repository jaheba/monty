# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 15
- Parallel jobs: 1
- Run duration: 11m 6s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

default: Monty 1.0.1 · typed\_match: Monty 1.0.1

- Reference: default
- Threshold: 3% and 0.01 ms
24 changes or failures; 50 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | default | typed\_match |
| --- | --- | --- |
| portfolio\_risk | **56.9** / **56.6** | 60.4 / 59.8 |
| game\_pidigits | failed / failed | failed / failed |
| perf\_barnes\_hut | **4220** / **4210** | 4310 / 4330 |
| text\_csv\_aggregate\_medium | **336** / **330** | 345 / 341 |
| text\_csv\_cleanup\_small | **7.14** / **6.83** | 7.36 / 7.13 |
| text\_csv\_cleanup\_medium | **669** / **660** | 694 / 689 |
| stdout\_lines | **2.19** / **1.85** | 2.26 / 1.98 |
| perf\_richards | **171** / **171** | 179 / 178 |
| rustpython\_nqueens | failed / failed | failed / failed |
| rustpython\_pystone | **283** / **283** | 291 / 292 |
| rustpython\_regex\_effbot | failed / failed | failed / failed |
| rustpython\_micro\_cmp | **1.83** / **1.68** | 2.02 / 1.92 |
| rustpython\_micro\_sorted\_900 | **1.04** / **0.93** | 1.10 / 0.99 |


## Peak RSS · MiB · one-shot

| Workload | default | typed\_match |
| --- | --- | --- |
| game\_pidigits | failed | failed |
| rustpython\_nqueens | failed | failed |
| rustpython\_regex\_effbot | failed | failed |


## Failures

- default / game\_pidigits / one-shot: ["TypeError: 'generator' object is not an iterator"]
- default / rustpython\_nqueens / one-shot: ["TypeError: 'generator' object is not an iterator"]
- default / rustpython\_regex\_effbot / one-shot: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- typed\_match / game\_pidigits / one-shot: ["TypeError: 'generator' object is not an iterator"]
- typed\_match / rustpython\_nqueens / one-shot: ["TypeError: 'generator' object is not an iterator"]
- typed\_match / rustpython\_regex\_effbot / one-shot: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- default / game\_pidigits / repeated: ["TypeError: 'generator' object is not an iterator"]
- default / rustpython\_nqueens / repeated: ["TypeError: 'generator' object is not an iterator"]
- default / rustpython\_regex\_effbot / repeated: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- typed\_match / game\_pidigits / repeated: ["TypeError: 'generator' object is not an iterator"]
- typed\_match / rustpython\_nqueens / repeated: ["TypeError: 'generator' object is not an iterator"]
- typed\_match / rustpython\_regex\_effbot / repeated: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]

## Runtimes and build details

- **default**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/default-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 95519301f2db9abab0a02e0d8b2c69442b3395c4718d351cb847522ee856632f

- **typed\_match**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/unified-match-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: b43a25c6400d190a0c861a4e0d6c25740821a7ee69d2c17f6f0bca3f7815fc58
