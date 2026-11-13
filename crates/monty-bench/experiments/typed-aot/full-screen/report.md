# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 3
- Parallel jobs: 1
- Run duration: 2m 50s
- Repeated warmups: 1
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

threaded: Monty 1.0.1 · typed: Monty 1.0.1

Quick run: reduced sampling for a rough comparison. Repeat with default settings to confirm changes.

- Reference: threaded
- Threshold: 1% and 0.01 ms
9 changes or failures; 121 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload | threaded | typed |
| --- | --- | --- |
| game\_pidigits | failed / failed | failed / failed |
| rustpython\_nqueens | failed / failed | failed / failed |
| rustpython\_regex\_effbot | failed / failed | failed / failed |


## Peak RSS · MiB · one-shot

| Workload | threaded | typed |
| --- | --- | --- |
| game\_pidigits | failed | failed |
| rustpython\_nqueens | failed | failed |
| rustpython\_regex\_effbot | failed | failed |


## Failures

- threaded / game\_pidigits / one-shot: ["TypeError: 'generator' object is not an iterator"]
- threaded / rustpython\_nqueens / one-shot: ["TypeError: 'generator' object is not an iterator"]
- threaded / rustpython\_regex\_effbot / one-shot: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- typed / game\_pidigits / one-shot: ["TypeError: 'generator' object is not an iterator"]
- typed / rustpython\_nqueens / one-shot: ["TypeError: 'generator' object is not an iterator"]
- typed / rustpython\_regex\_effbot / one-shot: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- threaded / game\_pidigits / repeated: ["TypeError: 'generator' object is not an iterator"]
- threaded / rustpython\_nqueens / repeated: ["TypeError: 'generator' object is not an iterator"]
- threaded / rustpython\_regex\_effbot / repeated: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]
- typed / game\_pidigits / repeated: ["TypeError: 'generator' object is not an iterator"]
- typed / rustpython\_nqueens / repeated: ["TypeError: 'generator' object is not an iterator"]
- typed / rustpython\_regex\_effbot / repeated: ["AttributeError: 'function' object has no attribute 'data' and no \_\_dict\_\_ for setting new attributes"]

## Runtimes and build details

- **threaded**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/combined-budget-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 1a683ea6d70bf975c665e662845cab0d1c2a3fb25a31443d3f7b0d8413c98360

- **typed**: monty; monty-runtime 1.0.1
  - Binary: /private/tmp/monty-native-study/monty/crates/monty-bench/experiments/typed-aot/combined-budget-artifacts/bin/monty
  - Build: Source revision and compiler flags unverified
  - SHA256: 1a683ea6d70bf975c665e662845cab0d1c2a3fb25a31443d3f7b0d8413c98360
