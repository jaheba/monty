# Type-checker failures among Tokspam-accepted comparison inputs

Investigated on 2026-10-26 with the local `monty-type-checking` probe. Each
Python source was type checked in a separate process with a 512 MiB allocator
ceiling and a 10-second deadline. No generated Python was executed. The
original eight failures came from a comparison that enabled Tokspam's 200-level
parser and AST scores but **did not enable** its source, token, or work limits.

## Original failures and smaller reproductions

| Source shape | Original failure | `Limits::screening()` on original | Smaller reproduction |
| --- | --- | --- | --- |
| 198, 199, and 200 nested lists | Three 10-second timeouts | `Nesting` at level 101 | 114 lists also reach the 10-second deadline; 100-level screening rejects it. |
| 100,000 comprehension `if` clauses | 10-second timeout | `TokenCount` | 35,000 clauses overflow the type-checker process stack; 80,000 still time out. The 35,000-clause source passes screening. |
| 100,000 dict entries | 10-second timeout | `TokenCount` | 80,000 entries still time out; screening rejects them. `max_display_items = 8192` rejects earlier. |
| 100,000 sequence-pattern items | 10-second timeout | `TokenCount` | 80,000 items still time out; screening rejects them. The current width tracker rejects above 4096 as `SliceItems`. |
| 51,488 `or` terms | 512 MiB allocator abort | `TokenCount` | 8,192 terms trigger a 402,653,184-byte allocation and abort; 8,191 finish. The smaller source passes screening. |
| 52,227 `with` items | 512 MiB allocator abort | `TokenCount` | 3,529 items abort; 3,528 finish with diagnostics. The smaller source passes screening. |

The three list inputs share one shape, so the eight inputs reduce to six
distinct patterns. All eight **original** inputs are rejected by the full
screening policy: the lists by its depth limit of 100 and the five wide inputs
by its 100,000-token limit. They were not accepted under that policy.

The smaller failing sources show where those limits stop helping:

| Shape | Small failing source | Tokens | Screening result |
| --- | ---: | ---: | --- |
| Comprehension filters | 35,000 clauses, 175,023 bytes | 70,013 | Accept |
| Boolean chain | 8,192 terms, 65,537 bytes | 16,386 | Accept |
| `with` header | 3,529 items, 14,126 bytes | 7,061 | Accept |

The list case needs no new Tokspam rule if `max_nesting_depth = 100` is used.
The dict and pattern cases are caught by existing token limits at the measured
timeout sizes. Enabling the existing width limits catches them sooner, though
classifying a sequence pattern as `SliceItems` is only a lexical approximation.

The three accepted smaller failures need shape-specific controls or a
type-checker resource boundary. Tokspam now has limits of 8192 comprehension
filters, 4096 Boolean operands, and 1024 `with` items in its screening policy.
Each rejects its corresponding reproduction before the observed failure size.
The observed failure thresholds depend on this type-checker build, allocator
ceiling, host, and deadline.

A blanket token cap of 7,060 would reject three of Monty's 710 existing Python
fixtures; a 14,000-byte source cap would reject 36. Among the 709 fixtures that
Python 3.15 could parse, the largest Boolean expression has five operands, the
largest `with` statement has two items, and the largest comprehension generator
has two filters. One fixture intentionally contains invalid future-import
syntax and could not be counted with Python's AST.

## Reproduction

The original sources and outcomes are in
`/tmp/monty-nesting-depth-followup/inputs.jsonl` and
`/tmp/monty-nesting-depth-followup/type-results.jsonl`. The isolated probe is
built by `scripts/nesting_comparison/check_types.py` from
`scripts/nesting_comparison/type_check.rs`.

Smaller source forms, where `n` is the repeat count:

```python
"[" * n + "x" + "]" * n
"x = [0 for x in (0,)" + " if x" * n + " ]\n"
"x = {" + "'x':0," * n + "}\n"
"match x:\n case [" + "0," * n + "]: pass\n"
"x = " + " or ".join(["True"] * n) + "\n"
"with " + ", ".join(["cm"] * n) + ": pass\n"
```

The Python above constructs source strings for the parser and type checker;
it does not run the constructed code.
