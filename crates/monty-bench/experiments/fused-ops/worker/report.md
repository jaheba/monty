# Performance changes

- CPU: arm · 10 logical CPUs
- OS: macOS-27.0-arm64-arm-64bit-Mach-O
- Samples: 20
- Parallel jobs: 1
- Run duration: 21s
- Repeated warmups: 3
- Timing: median ms, one-shot / repeated; startup excluded
- RSS: median MiB, separate one-shot processes including startup
- Startup: empty -c command, launch through exit, normal defaults

baseline: Monty 1.0.1 · candidate: Monty 1.0.1

- Reference: baseline
- Threshold: 1% and 0.01 ms
    9 changes or failures; 11 inconclusive (see diff.json).

## Runtime · ms · one-shot / repeated

| Workload         | baseline            | candidate           |
| ---------------- | ------------------- | ------------------- |
| attr_dispatch    | 11.5 / 10.7         | **11.1** / **10.6** |
| fstring_report   | **3.37** / **3.08** | 3.50 / 3.19         |
| local_arithmetic | **16.1** / **16.0** | 17.6 / 17.4         |
| local_compare    | **13.9** / **13.5** | 14.4 / 14.2         |
| local_subscript  | **8.93** / **8.65** | 9.38 / 9.21         |

## Runtimes and build details

- **baseline**: monty; monty-runtime 1.0.1

    - Binary: /private/tmp/monty-fused-ops/baseline
    - Build: Source revision and compiler flags unverified
    - SHA256: 22d2cc7a1384be1ee79cb2ab42b43e1ecb585bdacb19b99bd6432e54f93abe49

- **candidate**: monty; monty-runtime 1.0.1

    - Binary: /private/tmp/monty-fused-ops/candidate
    - Build: Source revision and compiler flags unverified
    - SHA256: 8954e88b1c6fdaf63e471e05e5ea032477bf32302b56c748c7b97134a66e1f58
