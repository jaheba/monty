# Focused fusion results

The eight-opcode implementation improves the arithmetic and comparison workloads in both execution-only and worker
measurements.
Unlike the broad experiment, it avoids intermediate stack values and uses checked integer fast paths.

| Workload         | Static fusion sites | Parse-once change | Repeated worker change | Worker classification |
| ---------------- | ------------------: | ----------------: | ---------------------: | --------------------- |
| agg_rows         |                   8 |             -2.8% |                  -3.8% | improvement           |
| attr_dispatch    |                   0 |             -1.5% |                  -1.6% | improvement           |
| dict_comp        |                   1 |            -22.2% |                  -8.9% | inconclusive          |
| fib_25           |                   2 |             -9.0% |                 -10.7% | improvement           |
| fstring_report   |                   3 |             -3.1% |                  -3.6% | inconclusive          |
| list_comp        |                   1 |            -32.0% |                 -16.9% | inconclusive          |
| local_add        |                   1 |            -29.2% |                 -30.2% | improvement           |
| local_arithmetic |                   5 |            -41.0% |                 -39.6% | improvement           |
| local_compare    |                   4 |            -50.0% |                 -52.0% | improvement           |
| local_moves      |                   1 |             -0.4% |                  +0.3% | unchanged             |
| local_subscript  |                   1 |            -14.0% |                 -18.7% | improvement           |
| pair_tuples      |                   1 |             -8.9% |                  -9.6% | improvement           |
| small_int_math   |                   3 |            -53.4% |                 -56.7% | improvement           |
| str_parse        |                   3 |             -2.9% |                  -3.6% | improvement           |

Negative changes mean less execution time; positive changes mean more.
Parse-once values are descriptive medians across 45 samples per build/workload, collected in three alternating rounds.
The dedicated local-add improvement held between 29.1% and 29.5% in the paired rounds; local arithmetic improved between
40.8% and 41.2%, and local comparisons between 49.8% and 50.2%.

Worker timings include compilation, execution and output delivery.
Flying Circus used 20 samples per build/workload/mode, three repeated warmups, three separate RSS samples, five startup
samples and serial execution.
Its corrected bootstrap comparisons use a 1% relative threshold and a 0.01 ms absolute threshold.
It reports 19 improvements, zero regressions, six inconclusive comparisons and no failures or changed outputs.
“Inconclusive” does not establish unchanged performance.

The isolated small-constant arithmetic loop improved by more than half.
Among the existing workloads, Fibonacci improved by 10.7%, tuple-pair construction by 9.6%, aggregation by 3.8% and text
parsing by 3.6% in repeated worker requests.
The arithmetic and comparison microbenchmarks demonstrate potential; they do not predict a general application speedup.

`attr_dispatch` contains zero fused sites, so its small improvement cannot result from executing fused instructions.
Changes to compiled code layout remain a confound for small differences in workloads with little fusion coverage.

This experiment combines fusion, stack elimination, integer specialization and batched checkpoint bookkeeping.
It does not isolate each contribution; an ordinary-opcode integer-fast-path baseline would be needed for that comparison.

The focused worker is 23,385,888 bytes versus 23,385,856 bytes for the baseline (+32 bytes).
The focused direct runner is 7,069,008 bytes versus 7,068,992 bytes (+16 bytes).
Binary sizes do not measure instruction-cache behavior or isolate individual handler sizes.

The feature remains disabled by default, and cross-build dump compatibility is deferred.
The baseline is the same saved dirty-tree build as the broad experiment, with the same compiler and release settings.
All fourteen workloads matched CPython's output under both worker binaries.
892 interpreter tests passed with reference-count checking enabled, and the six fusion regression tests passed with fusion
disabled too.

- [Parse-once samples and paired-round ranges](focused-direct/results.md)
- [Interactive worker report](focused-worker/report.html)
- [Worker statistical comparisons](focused-worker/diff.json)
- [Static fusion coverage](focused-census.csv), without execution-frequency weights
- [Build provenance](focused-provenance.txt) and [binary hashes](focused-runtime-sha256.txt)
- [Validation details](focused-validation.md)
- [Earlier broad experiment](results.md)
