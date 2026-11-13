# Application benchmarks

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 3
- Parallel jobs: 1
- Run duration: 2m 41s
- Repeated warmups: 1
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

CPython 3.15.0 · baseline: Monty 1.0.1

Quick run: reduced sampling for a rough comparison. Repeat with default settings to confirm changes.

## Process startup · ms

| CPython | baseline |
| --- | --- |
| 24.4 | **6.55** |


## Runtime · ms · one-shot / repeated

| Workload | CPython | baseline |
| --- | --- | --- |
| expense\_report | 1.13 / 0.21 | **0.42** / **0.11** |
| order\_cleanup | 1.27 / 0.25 | **0.46** / **0.17** |
| api\_report | 1.17 / **0.24** | **0.56** / 0.29 |
| capacity\_planning | **9.83** / **8.74** | 28.7 / 28.5 |
| portfolio\_risk | **52.5** / **52.1** | 61.5 / 61.6 |
| ticket\_search | **9.73** / **6.88** | 27.3 / 26.9 |
| game\_fannkuch | **271** / **266** | n/a |
| game\_nbody | **78.3** / **78.3** | 450 / 449 |
| game\_pidigits | **286** / **286** | failed / failed |
| game\_spectral\_norm | **67.2** / **67.0** | 316 / 313 |
| game\_regex\_dna | **254** / **249** | n/a |
| game\_meteor\_contest | **85.5** / **82.3** | n/a |
| perf\_barnes\_hut | **629** / **631** | 4260 / 4260 |
| perf\_float | **51.7** / **49.2** | 219 / 215 |
| perf\_unpack\_sequence | **18.1** / **17.5** | 319 / 318 |
| perf\_json\_dumps | 11.1 / **10.2** | **10.5** / **10.2** |
| perf\_json\_loads | 1.94 / 1.20 | **1.23** / **0.82** |
| text\_line\_logs\_small | **0.34** / **0.32** | 0.46 / 0.36 |
| text\_line\_logs\_medium | **8.41** / **7.98** | 17.5 / 17.7 |
| text\_regex\_logs\_small | **0.55** / **0.39** | 0.93 / 0.61 |
| text\_regex\_logs\_medium | **15.5** / **15.4** | 23.6 / 23.9 |
| text\_csv\_aggregate\_small | **1.47** / **1.47** | 3.92 / 3.68 |
| text\_csv\_aggregate\_medium | **82.2** / **80.1** | 347 / 346 |
| text\_csv\_cleanup\_small | **2.24** / **2.10** | 7.46 / 7.29 |
| text\_csv\_cleanup\_medium | **155** / **152** | 702 / 697 |
| text\_json\_parse\_small | **0.28** / **0.22** | 0.45 / 0.26 |
| text\_json\_parse\_medium | **8.97** / **8.39** | 9.92 / 9.17 |
| text\_json\_transform\_small | **0.39** / 0.34 | 0.56 / **0.29** |
| text\_json\_transform\_medium | **9.59** / **9.15** | 12.1 / 10.6 |
| text\_json\_lines\_small | **0.46** / 0.43 | 0.58 / **0.37** |
| text\_json\_lines\_medium | 14.8 / **14.3** | **14.2** / 14.4 |
| memory\_string\_list\_short | **90.0** / **88.4** | 176 / 172 |
| memory\_string\_list\_long | **95.4** / **94.6** | 254 / 244 |
| memory\_string\_index\_short | **67.6** / **64.1** | 87.2 / 84.5 |
| memory\_string\_index\_long | **76.1** / **74.8** | 133 / 128 |
| memory\_customer\_records | **155** / **160** | 322 / 326 |
| stdout\_lines | 4.33 / 3.11 | **2.18** / **1.90** |
| perf\_richards | **30.9** / **29.9** | 179 / 180 |
| rustpython\_base64 | 4.59 / **0.81** | **1.78** / 1.59 |
| rustpython\_decimal\_factorial | **6.69** / **1.03** | n/a |
| rustpython\_deepcopy | **5.32** / **2.66** | n/a |
| rustpython\_deltablue | **5.96** / **5.19** | n/a |
| rustpython\_generators | **77.1** / **76.7** | n/a |
| rustpython\_mandelbrot | 17.5 / 17.5 | **14.7** / **14.7** |
| rustpython\_nqueens | **58.0** / **57.3** | failed / failed |
| rustpython\_pickle | **12.8** / **9.79** | n/a |
| rustpython\_pystone | **62.9** / **62.1** | 295 / 297 |
| rustpython\_regex\_effbot | **24.7** / **23.6** | failed / failed |
| rustpython\_unpickle | **5.65** / **2.87** | n/a |
| rustpython\_micro\_addition\_100 | **0.14** / 0.10 | 0.17 / **0.05** |
| rustpython\_micro\_addition\_300 | **0.13** / **0.13** | 0.18 / 0.14 |
| rustpython\_micro\_addition\_500 | **0.13** / **0.11** | 0.19 / 0.12 |
| rustpython\_micro\_addition\_700 | **0.16** / 0.13 | 0.20 / **0.12** |
| rustpython\_micro\_addition\_900 | **0.15** / 0.14 | 0.20 / **0.13** |
| rustpython\_micro\_cmp | **1.53** / **1.62** | 1.88 / 1.83 |
| rustpython\_micro\_complex\_class | **0.15** / **0.18** | n/a |
| rustpython\_micro\_comprehension\_dict\_100 | **0.16** / 0.15 | 0.22 / **0.12** |
| rustpython\_micro\_comprehension\_dict\_300 | **0.17** / 0.14 | 0.21 / **0.13** |
| rustpython\_micro\_comprehension\_dict\_500 | **0.13** / 0.11 | 0.24 / **0.10** |
| rustpython\_micro\_comprehension\_dict\_700 | **0.16** / **0.12** | 0.26 / 0.17 |
| rustpython\_micro\_comprehension\_dict\_900 | **0.17** / **0.12** | 0.30 / 0.23 |
| rustpython\_micro\_comprehension\_list\_100 | **0.14** / 0.13 | 0.19 / **0.09** |
| rustpython\_micro\_comprehension\_list\_300 | **0.18** / 0.13 | **0.18** / **0.10** |
| rustpython\_micro\_comprehension\_list\_500 | **0.13** / 0.13 | 0.20 / **0.12** |
| rustpython\_micro\_comprehension\_list\_700 | **0.13** / 0.11 | 0.20 / **0.09** |
| rustpython\_micro\_comprehension\_list\_900 | **0.17** / 0.13 | 0.22 / **0.10** |
| rustpython\_micro\_comprehension\_set\_100 | **0.13** / 0.12 | 0.22 / **0.09** |
| rustpython\_micro\_comprehension\_set\_300 | **0.11** / **0.09** | 0.22 / 0.11 |
| rustpython\_micro\_comprehension\_set\_500 | **0.14** / **0.11** | 0.24 / **0.11** |
| rustpython\_micro\_comprehension\_set\_700 | **0.13** / 0.14 | 0.27 / **0.13** |
| rustpython\_micro\_comprehension\_set\_900 | **0.12** / **0.12** | 0.27 / 0.15 |
| rustpython\_micro\_frozenset\_100 | **0.11** / 0.08 | 0.13 / **0.07** |
| rustpython\_micro\_frozenset\_300 | **0.09** / 0.09 | 0.15 / **0.07** |
| rustpython\_micro\_frozenset\_500 | **0.10** / 0.11 | 0.12 / **0.07** |
| rustpython\_micro\_frozenset\_700 | **0.10** / 0.09 | 0.17 / **0.07** |
| rustpython\_micro\_frozenset\_900 | **0.13** / 0.12 | 0.14 / **0.07** |
| rustpython\_micro\_list\_count\_builtins\_100 | 0.11 / **0.06** | **0.10** / 0.08 |
| rustpython\_micro\_list\_count\_builtins\_300 | **0.11** / 0.10 | 0.14 / **0.08** |
| rustpython\_micro\_list\_count\_builtins\_500 | **0.11** / 0.07 | **0.11** / **0.06** |
| rustpython\_micro\_list\_count\_builtins\_700 | **0.08** / 0.09 | 0.12 / **0.06** |
| rustpython\_micro\_list\_count\_builtins\_900 | **0.09** / **0.08** | 0.11 / 0.09 |
| rustpython\_micro\_list\_count\_custom\_eq\_100 | **0.09** / **0.07** | 0.12 / 0.08 |
| rustpython\_micro\_list\_count\_custom\_eq\_300 | **0.12** / **0.09** | 0.14 / 0.12 |
| rustpython\_micro\_list\_count\_custom\_eq\_500 | **0.12** / **0.09** | 0.17 / 0.13 |
| rustpython\_micro\_list\_count\_custom\_eq\_700 | **0.14** / **0.07** | 0.22 / 0.18 |
| rustpython\_micro\_list\_count\_custom\_eq\_900 | **0.15** / **0.10** | 0.24 / 0.20 |
| rustpython\_micro\_loop\_append\_100 | **0.15** / 0.10 | **0.15** / **0.08** |
| rustpython\_micro\_loop\_append\_300 | **0.15** / **0.11** | 0.16 / **0.11** |
| rustpython\_micro\_loop\_append\_500 | **0.17** / **0.11** | 0.22 / **0.11** |
| rustpython\_micro\_loop\_append\_700 | **0.15** / 0.17 | 0.19 / **0.15** |
| rustpython\_micro\_loop\_append\_900 | **0.14** / 0.17 | 0.22 / **0.13** |
| rustpython\_micro\_loop\_string\_100 | 0.10 / 0.07 | **0.08** / **0.05** |
| rustpython\_micro\_loop\_string\_300 | 0.10 / 0.10 | **0.08** / **0.08** |
| rustpython\_micro\_loop\_string\_500 | 0.09 / 0.09 | **0.08** / **0.08** |
| rustpython\_micro\_loop\_string\_700 | **0.08** / 0.10 | **0.08** / **0.05** |
| rustpython\_micro\_loop\_string\_900 | 0.12 / **0.08** | **0.09** / **0.08** |
| rustpython\_micro\_sorted\_100 | 0.25 / **0.19** | **0.22** / 0.21 |
| rustpython\_micro\_sorted\_300 | **0.35** / **0.29** | 0.42 / 0.36 |
| rustpython\_micro\_sorted\_500 | **0.46** / **0.43** | 0.63 / 0.59 |
| rustpython\_micro\_sorted\_700 | **0.59** / **0.56** | 0.85 / 0.74 |
| rustpython\_micro\_sorted\_900 | **0.77** / **0.69** | 1.07 / 0.97 |
| rustpython\_micro\_strings | **0.82** / **0.72** | 1.13 / 1.00 |
| text\_unicode\_access\_ascii | **3.28** / **2.39** | 14.2 / 13.9 |
| memory\_unicode\_ascii | **5.41** / **2.12** | 5.89 / 5.97 |
| text\_unicode\_access\_latin1 | **3.44** / **2.38** | 25.2 / 24.6 |
| memory\_unicode\_latin1 | **5.03** / **2.33** | 9.42 / 6.29 |
| text\_unicode\_access\_cjk | **3.65** / **2.82** | 35.4 / 35.2 |
| memory\_unicode\_cjk | **8.17** / **3.81** | 14.6 / 8.51 |
| text\_unicode\_access\_emoji | **3.86** / **2.88** | 45.3 / 44.8 |
| memory\_unicode\_emoji | 17.7 / **5.81** | **15.7** / 10.2 |
| text\_unicode\_access\_mixed | **3.21** / **2.43** | 14.0 / 13.9 |
| memory\_unicode\_mixed | 13.5 / 5.61 | **6.58** / **4.18** |
| agent\_unicode\_german\_ascii\_small | **0.66** / **0.60** | 0.95 / 0.61 |
| agent\_unicode\_german\_ascii\_large | **4.33** / **4.04** | 5.55 / 5.20 |
| agent\_unicode\_german\_small | **0.78** / **0.80** | 1.09 / 0.86 |
| agent\_unicode\_german\_large | **6.60** / **6.93** | 9.29 / 8.82 |
| agent\_unicode\_german\_emoji\_small | **0.91** / **0.83** | 1.17 / 0.87 |
| agent\_unicode\_german\_emoji\_large | **7.41** / **7.28** | 9.35 / 8.98 |
| agent\_unicode\_chinese\_small | **0.61** / **0.66** | 1.02 / 0.82 |
| agent\_unicode\_chinese\_large | **4.77** / **4.49** | 8.39 / 7.89 |
| string\_random\_access\_interned | **4.16** / **3.12** | 30.5 / 30.5 |
| string\_random\_access\_interned\_ascii | **3.25** / **2.41** | 14.1 / 13.8 |
| string\_end\_access\_unicode | **2.88** / **1.82** | 28.5 / 28.1 |
| subtitle\_search | **0.28** / **0.23** | 2.90 / 2.88 |


## Peak RSS · MiB · one-shot

| Workload | CPython | baseline |
| --- | --- | --- |
| expense\_report | 15.6 | **10.3** |
| order\_cleanup | 15.7 | **10.7** |
| api\_report | 15.6 | **11.5** |
| capacity\_planning | 17.5 | **14.8** |
| portfolio\_risk | 15.7 | **10.7** |
| ticket\_search | 17.5 | **14.0** |
| game\_fannkuch | **15.6** | n/a |
| game\_nbody | 15.9 | **11.1** |
| game\_pidigits | **16.2** | failed |
| game\_spectral\_norm | 15.5 | **10.8** |
| game\_regex\_dna | **33.6** | n/a |
| game\_meteor\_contest | **18.3** | n/a |
| perf\_barnes\_hut | 17.0 | **12.7** |
| perf\_float | **32.6** | 57.0 |
| perf\_unpack\_sequence | 19.9 | **11.5** |
| perf\_json\_dumps | 17.4 | **14.4** |
| perf\_json\_loads | 16.1 | **12.0** |
| text\_line\_logs\_small | 15.7 | **10.6** |
| text\_line\_logs\_medium | 28.8 | **19.2** |
| text\_regex\_logs\_small | 15.7 | **12.3** |
| text\_regex\_logs\_medium | 24.9 | **21.0** |
| text\_csv\_aggregate\_small | 16.0 | **11.5** |
| text\_csv\_aggregate\_medium | **38.9** | 42.7 |
| text\_csv\_cleanup\_small | 16.1 | **11.6** |
| text\_csv\_cleanup\_medium | **46.4** | 52.2 |
| text\_json\_parse\_small | 15.6 | **11.5** |
| text\_json\_parse\_medium | 32.3 | **22.9** |
| text\_json\_transform\_small | 15.7 | **11.8** |
| text\_json\_transform\_medium | 32.5 | **23.6** |
| text\_json\_lines\_small | 15.7 | **11.8** |
| text\_json\_lines\_medium | 29.6 | **19.1** |
| memory\_string\_list\_short | **52.1** | 89.2 |
| memory\_string\_list\_long | **75.4** | 112 |
| memory\_string\_index\_short | **52.8** | 61.4 |
| memory\_string\_index\_long | **64.7** | 72.6 |
| memory\_customer\_records | **71.6** | 151 |
| stdout\_lines | 16.7 | **10.4** |
| perf\_richards | 18.0 | **12.5** |
| rustpython\_base64 | 16.4 | **11.0** |
| rustpython\_decimal\_factorial | **15.9** | n/a |
| rustpython\_deepcopy | **16.2** | n/a |
| rustpython\_deltablue | **17.4** | n/a |
| rustpython\_generators | **27.8** | n/a |
| rustpython\_mandelbrot | 15.5 | **10.1** |
| rustpython\_nqueens | **15.7** | failed |
| rustpython\_pickle | **16.6** | n/a |
| rustpython\_pystone | 16.1 | **11.4** |
| rustpython\_regex\_effbot | **16.6** | failed |
| rustpython\_unpickle | **16.4** | n/a |
| rustpython\_micro\_addition\_100 | 15.5 | **10.0** |
| rustpython\_micro\_addition\_300 | 15.6 | **10.0** |
| rustpython\_micro\_addition\_500 | 15.4 | **10.0** |
| rustpython\_micro\_addition\_700 | 15.6 | **10.0** |
| rustpython\_micro\_addition\_900 | 15.5 | **10.0** |
| rustpython\_micro\_cmp | 15.5 | **9.78** |
| rustpython\_micro\_complex\_class | **15.5** | n/a |
| rustpython\_micro\_comprehension\_dict\_100 | 15.6 | **10.2** |
| rustpython\_micro\_comprehension\_dict\_300 | 15.6 | **10.2** |
| rustpython\_micro\_comprehension\_dict\_500 | 15.6 | **10.2** |
| rustpython\_micro\_comprehension\_dict\_700 | 15.5 | **10.2** |
| rustpython\_micro\_comprehension\_dict\_900 | 15.5 | **10.3** |
| rustpython\_micro\_comprehension\_list\_100 | 15.4 | **10.1** |
| rustpython\_micro\_comprehension\_list\_300 | 15.5 | **10.1** |
| rustpython\_micro\_comprehension\_list\_500 | 15.5 | **10.1** |
| rustpython\_micro\_comprehension\_list\_700 | 15.5 | **10.1** |
| rustpython\_micro\_comprehension\_list\_900 | 15.5 | **10.1** |
| rustpython\_micro\_comprehension\_set\_100 | 15.5 | **10.2** |
| rustpython\_micro\_comprehension\_set\_300 | 15.5 | **10.2** |
| rustpython\_micro\_comprehension\_set\_500 | 15.6 | **10.2** |
| rustpython\_micro\_comprehension\_set\_700 | 15.5 | **10.3** |
| rustpython\_micro\_comprehension\_set\_900 | 15.5 | **10.3** |
| rustpython\_micro\_frozenset\_100 | 15.4 | **10.1** |
| rustpython\_micro\_frozenset\_300 | 15.5 | **10.0** |
| rustpython\_micro\_frozenset\_500 | 15.6 | **10.1** |
| rustpython\_micro\_frozenset\_700 | 15.5 | **10.1** |
| rustpython\_micro\_frozenset\_900 | 15.5 | **10.1** |
| rustpython\_micro\_list\_count\_builtins\_100 | 15.4 | **10.0** |
| rustpython\_micro\_list\_count\_builtins\_300 | 15.5 | **10.0** |
| rustpython\_micro\_list\_count\_builtins\_500 | 15.5 | **10.0** |
| rustpython\_micro\_list\_count\_builtins\_700 | 15.5 | **10.1** |
| rustpython\_micro\_list\_count\_builtins\_900 | 15.5 | **10.0** |
| rustpython\_micro\_list\_count\_custom\_eq\_100 | 15.5 | **10.3** |
| rustpython\_micro\_list\_count\_custom\_eq\_300 | 15.5 | **10.2** |
| rustpython\_micro\_list\_count\_custom\_eq\_500 | 15.6 | **10.2** |
| rustpython\_micro\_list\_count\_custom\_eq\_700 | 15.5 | **10.2** |
| rustpython\_micro\_list\_count\_custom\_eq\_900 | 15.6 | **10.2** |
| rustpython\_micro\_loop\_append\_100 | 15.5 | **10.1** |
| rustpython\_micro\_loop\_append\_300 | 15.6 | **10.2** |
| rustpython\_micro\_loop\_append\_500 | 15.5 | **10.2** |
| rustpython\_micro\_loop\_append\_700 | 15.5 | **10.2** |
| rustpython\_micro\_loop\_append\_900 | 15.5 | **10.2** |
| rustpython\_micro\_loop\_string\_100 | 15.5 | **9.77** |
| rustpython\_micro\_loop\_string\_300 | 15.4 | **9.77** |
| rustpython\_micro\_loop\_string\_500 | 15.4 | **9.78** |
| rustpython\_micro\_loop\_string\_700 | 15.5 | **9.75** |
| rustpython\_micro\_loop\_string\_900 | 15.4 | **9.80** |
| rustpython\_micro\_sorted\_100 | 16.0 | **10.5** |
| rustpython\_micro\_sorted\_300 | 16.1 | **10.6** |
| rustpython\_micro\_sorted\_500 | 16.1 | **10.6** |
| rustpython\_micro\_sorted\_700 | 16.2 | **10.7** |
| rustpython\_micro\_sorted\_900 | 16.2 | **10.8** |
| rustpython\_micro\_strings | 15.5 | **9.66** |
| text\_unicode\_access\_ascii | 15.6 | **10.2** |
| memory\_unicode\_ascii | 41.0 | **30.9** |
| text\_unicode\_access\_latin1 | 15.5 | **10.2** |
| memory\_unicode\_latin1 | **40.8** | 50.5 |
| text\_unicode\_access\_cjk | 15.5 | **10.2** |
| memory\_unicode\_cjk | **68.0** | 74.0 |
| text\_unicode\_access\_emoji | 15.5 | **10.3** |
| memory\_unicode\_emoji | 120 | **89.8** |
| text\_unicode\_access\_mixed | 15.6 | **10.2** |
| memory\_unicode\_mixed | 120 | **36.4** |
| agent\_unicode\_german\_ascii\_small | 16.1 | **12.6** |
| agent\_unicode\_german\_ascii\_large | 26.5 | **20.7** |
| agent\_unicode\_german\_small | 16.4 | **12.6** |
| agent\_unicode\_german\_large | 29.3 | **20.7** |
| agent\_unicode\_german\_emoji\_small | 16.7 | **12.5** |
| agent\_unicode\_german\_emoji\_large | 31.1 | **20.6** |
| agent\_unicode\_chinese\_small | 16.7 | **12.7** |
| agent\_unicode\_chinese\_large | 32.7 | **21.0** |
| string\_random\_access\_interned | 15.8 | **10.4** |
| string\_random\_access\_interned\_ascii | 15.6 | **10.2** |
| string\_end\_access\_unicode | 15.6 | **10.0** |
| subtitle\_search | 15.8 | **10.4** |


## Failures

- baseline / game\_pidigits / one-shot: ["TypeError: 'generator' object is not an iterator"]
- baseline / rustpython\_nqueens / one-shot: ["TypeError: 'generator' object is not an iterator"]
- baseline / rustpython\_regex\_effbot / one-shot: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- baseline / game\_pidigits / repeated: ["TypeError: 'generator' object is not an iterator"]
- baseline / rustpython\_nqueens / repeated: ["TypeError: 'generator' object is not an iterator"]
- baseline / rustpython\_regex\_effbot / repeated: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]

## Runtimes and build details

- **CPython**: cpython; Python 3.15.0
  - Binary: /opt/homebrew/Cellar/python@3.15/3.15.0/Frameworks/Python.framework/Versions/3.15/bin/python3.15
  - Build: Source revision and compiler flags unverified
  - SHA256: 15c72833c1ffc09bc4e05d1e1866784d126721ad548d9c4d8f943641262535cb

- **baseline**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/native-regions/flying-circus-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: bfdfc25c707f9b92f6b10a741bec598364bf08fe6e041dbdbb745c8568a3c1d3
