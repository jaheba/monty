# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 15
- Parallel jobs: 1
- Run duration: 16m 31s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

default: Monty 1.0.1 · threaded: Monty 1.0.1 · typed: Monty 1.0.1

- Reference: default
- Threshold: 3% and 0.01 ms
79 changes or failures; 133 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | default | threaded | typed |
| --- | --- | --- | --- |
| portfolio\_risk | **57.3** / **57.0** | 65.0 / 64.5 | 65.1 / 64.8 |
| ticket\_search | **26.8** / **26.3** | 27.3 / 26.9 | 27.2 / 27.2 |
| game\_pidigits | failed / failed | failed / failed | failed / failed |
| game\_spectral\_norm | **312** / **306** | 329 / 323 | 320 / 323 |
| perf\_float | **218** / **213** | 225 / 219 | 225 / 220 |
| perf\_unpack\_sequence | 325 / **312** | **320** / 316 | 321 / 321 |
| text\_line\_logs\_medium | **17.0** / **17.1** | 18.1 / 17.8 | 18.1 / 17.8 |
| text\_regex\_logs\_medium | **23.5** / **23.4** | 25.2 / 25.1 | 25.1 / 24.9 |
| text\_csv\_aggregate\_small | 3.81 / 3.57 | **3.59** / 3.35 | 3.64 / **3.34** |
| text\_csv\_aggregate\_medium | 335 / 330 | **316** / **313** | **316** / **313** |
| text\_csv\_cleanup\_small | 7.11 / 6.80 | **6.59** / 6.50 | 6.71 / **6.44** |
| text\_csv\_cleanup\_medium | 669 / 662 | **627** / **624** | **627** / **624** |
| text\_json\_lines\_medium | **14.3** / **14.1** | 14.7 / 14.6 | 14.9 / 14.8 |
| memory\_string\_list\_short | **174** / **166** | 184 / 179 | 184 / 173 |
| memory\_string\_list\_long | **248** / **238** | 257 / 249 | 257 / 245 |
| memory\_string\_index\_short | **85.6** / **80.6** | 90.1 / 85.7 | 90.4 / 84.7 |
| memory\_customer\_records | 309 / **311** | 311 / 321 | **308** / 321 |
| rustpython\_mandelbrot | **15.6** / **15.4** | 18.7 / 18.6 | 18.8 / 18.6 |
| rustpython\_nqueens | failed / failed | failed / failed | failed / failed |
| rustpython\_regex\_effbot | failed / failed | failed / failed | failed / failed |
| rustpython\_micro\_cmp | **1.78** / **1.71** | 2.12 / 2.02 | 2.15 / 2.02 |
| rustpython\_micro\_sorted\_500 | **0.61** / **0.55** | 0.68 / 0.60 | 0.66 / 0.58 |
| rustpython\_micro\_strings | **1.04** / **0.95** | 1.18 / 1.08 | 1.19 / 1.10 |


## Peak RSS · MiB · one-shot

| Workload | default | threaded | typed |
| --- | --- | --- | --- |
| game\_pidigits | failed | failed | failed |
| rustpython\_nqueens | failed | failed | failed |
| rustpython\_regex\_effbot | failed | failed | failed |


## Failures

- default / game\_pidigits / one-shot: ["TypeError: 'generator' object is not an iterator"]
- default / rustpython\_nqueens / one-shot: ["TypeError: 'generator' object is not an iterator"]
- default / rustpython\_regex\_effbot / one-shot: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- threaded / game\_pidigits / one-shot: ["TypeError: 'generator' object is not an iterator"]
- threaded / rustpython\_nqueens / one-shot: ["TypeError: 'generator' object is not an iterator"]
- threaded / rustpython\_regex\_effbot / one-shot: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- typed / game\_pidigits / one-shot: ["TypeError: 'generator' object is not an iterator"]
- typed / rustpython\_nqueens / one-shot: ["TypeError: 'generator' object is not an iterator"]
- typed / rustpython\_regex\_effbot / one-shot: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- default / game\_pidigits / repeated: ["TypeError: 'generator' object is not an iterator"]
- default / rustpython\_nqueens / repeated: ["TypeError: 'generator' object is not an iterator"]
- default / rustpython\_regex\_effbot / repeated: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- threaded / game\_pidigits / repeated: ["TypeError: 'generator' object is not an iterator"]
- threaded / rustpython\_nqueens / repeated: ["TypeError: 'generator' object is not an iterator"]
- threaded / rustpython\_regex\_effbot / repeated: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- typed / game\_pidigits / repeated: ["TypeError: 'generator' object is not an iterator"]
- typed / rustpython\_nqueens / repeated: ["TypeError: 'generator' object is not an iterator"]
- typed / rustpython\_regex\_effbot / repeated: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]

## Runtimes and build details

- **default**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/default-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 95519301f2db9abab0a02e0d8b2c69442b3395c4718d351cb847522ee856632f

- **threaded**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/liveness-combined-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: e9ab28dba72d38e030019d9985c9815ce386ea064dbae5e0885929afd591d344

- **typed**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/liveness-combined-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: e9ab28dba72d38e030019d9985c9815ce386ea064dbae5e0885929afd591d344
