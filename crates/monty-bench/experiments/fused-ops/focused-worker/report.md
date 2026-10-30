# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 20
- Parallel jobs: 1
- Run duration: 22s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

baseline: Monty 1.0.1 · candidate: Monty 1.0.1

- Reference: baseline
- Threshold: 1% and 0.01 ms
    19 changes or failures; 6 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload         | baseline    | candidate           |
| ---------------- | ----------- | ------------------- |
| agg_rows         | 10.2 / 9.81 | **9.63** / **9.44** |
| attr_dispatch    | 11.5 / 10.8 | **11.2** / **10.6** |
| fib_25           | 22.2 / 21.9 | **19.7** / **19.6** |
| local_add        | 6.41 / 6.13 | **4.48** / **4.28** |
| local_arithmetic | 16.2 / 15.9 | **9.85** / **9.63** |
| local_compare    | 13.9 / 13.6 | **6.83** / **6.54** |
| local_subscript  | 8.97 / 8.64 | **7.32** / **7.03** |
| pair_tuples      | 12.9 / 11.0 | **11.7** / **9.89** |
| small_int_math   | 6.83 / 6.60 | **3.03** / **2.86** |
| str_parse        | 7.31 / 6.96 | **7.19** / **6.71** |

## Runtimes and build details

- **baseline**: monty; monty-runtime 1.0.1

    - Binary: /private/tmp/monty-fused-ops/baseline
    - Build: Source revision and compiler flags unverified
    - SHA256: 22d2cc7a1384be1ee79cb2ab42b43e1ecb585bdacb19b99bd6432e54f93abe49

- **candidate**: monty; monty-runtime 1.0.1

    - Binary: /private/tmp/monty-fused-ops/focused
    - Build: Source revision and compiler flags unverified
    - SHA256: 6e3f4678815b9f1e6565d50dc3ba5ac7459dd37623be615cbb1a225f9f3ff18e
