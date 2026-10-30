# Results

The broad dispatch-fusion experiment does not establish a general performance improvement.
The four focused workloads are unchanged or slower, and the parse-once geometric mean across all twelve workloads is
0.83% slower (descriptive aggregation, not a significance test).

The workloads that improved most in the parse-once comparison contain no fused instructions.
Their improvements cannot be attributed to executing superinstructions.
Changed dispatch code layout is a possible explanation; this experiment does not isolate it.

| Workload         | Static fusion sites | Parse-once change | Repeated worker change | Worker classification |
| ---------------- | ------------------: | ----------------: | ---------------------: | --------------------- |
| agg_rows         |                   1 |             -2.6% |                  +1.0% | inconclusive          |
| attr_dispatch    |                   0 |             -5.4% |                  -1.7% | improvement           |
| dict_comp        |                   1 |             +7.9% |                  -4.5% | unchanged             |
| fib_25           |                   3 |             -0.5% |                  +0.2% | unchanged             |
| fstring_report   |                   0 |             -3.0% |                  +3.6% | regression            |
| list_comp        |                   1 |             +3.4% |                  +6.4% | unchanged             |
| local_arithmetic |                  10 |             +5.2% |                  +8.3% | regression            |
| local_compare    |                   8 |             +1.8% |                  +4.6% | regression            |
| local_moves      |                   9 |             -1.4% |                  +0.3% | unchanged             |
| local_subscript  |                   4 |             +6.1% |                  +6.5% | regression            |
| pair_tuples      |                   1 |             +3.2% |                  +0.2% | unchanged             |
| str_parse        |                   0 |             -3.8% |                  -1.8% | inconclusive          |

Negative changes mean faster; positive changes mean slower.
Worker classifications use Flying Circus's corrected bootstrap intervals, a 1% relative threshold and 0.01 ms absolute
threshold.
The tiny comprehension workloads can exceed the relative threshold without exceeding the absolute threshold.
“Inconclusive” does not establish unchanged performance.

Parse-once measurements: 45 samples per build/workload across three alternating rounds, each sample lasting at least
100 ms, after three warmup runs per build/workload/round.
Worker measurements: 20 samples per build/workload/mode, three repeated warmups, three separate RSS samples and five
startup samples, serial execution.
The worker report records seven timing regressions and two improvements; both improvements are in `attr_dispatch`, which
contains no fusion sites.
No workload failed or changed its output.

The worker binary grew from 23,385,856 to 23,451,952 bytes (+66,096 bytes, 0.28%).
The direct runner grew from 7,068,992 to 7,118,560 bytes (+49,568 bytes, 0.70%).

These handlers remove opcode decoding and dispatch between constituent operations.
They retain the existing stack pushes/pops and per-instruction checkpoint bookkeeping.
The result does not rule out superinstructions that eliminate stack traffic, temporary comparison results or repeated
local-value ownership work.
A smaller selection and family-by-family comparisons would be needed to identify useful fusions.

- [Parse-once raw comparison](direct/results.md)
- [Worker report](worker/report.html)
- [Worker statistical comparisons](worker/diff.json)
- [Static fusion coverage](census.csv), counted across compiled module and function bodies, without dynamic execution weights
- [Build provenance](provenance.txt) and [binary hashes](runtime-sha256.txt)
- [Validation](validation.md)
