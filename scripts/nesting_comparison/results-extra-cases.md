# Additional Tokspam coverage search

Run on 2026-10-26 with the current local Tokspam changes. Generated Python was
parsed and type checked in isolated processes; it was never executed.

## Quoted types outside colon annotations

Tokspam scans quoted annotations after a colon, but treats strings in other
type-bearing positions as ordinary literals. For example, generate `T` as
`"list[" * n + "int" + "]" * n` and insert it into these sources:

| Source shape | Tokspam AST score at `n = 500` | Monty guard | Type checker |
| --- | ---: | --- | --- |
| `type A = "T"` | 0 | Rejects | Length diagnostic |
| `A: TypeAlias = "T"` | 0 | Rejects | Length diagnostic |
| `cast("T", None)` | 2 | Rejects | Length diagnostic |
| `def f[T: "T"](): pass` | 2 | Rejects | Length diagnostic |
| `def f[T = "T"](): pass` | 2 | Rejects | Length diagnostic |
| `class C(list["T"]): pass` | 4 | Rejects | Length diagnostic |
| `TypeVar("T", bound="T")` | 2 | Rejects | Length diagnostic |
| `NewType("T", "T")` | 2 | Rejects | Length diagnostic |
| `A = "T"` | 0 | Rejects | Clean; ordinary string |

Here `"T"` means the literal containing the generated text, rather than the
two-character string `T`. The imports required for `TypeAlias`, `cast`,
`TypeVar`, and `NewType` were included in the tested sources. At `n = 100`, all
nine sources pass both guards and the type checker; Monty's AST scores are
102–103 while Tokspam's are 0–4. At `n = 500` and `1500`, Tokspam accepts all
nine and Monty's string scan rejects all nine. The type checker diagnoses the
eight type-bearing forms as `invalid-syntax-in-forward-annotation` because the
string is too long. The ordinary assignment remains clean.

This is a policy difference and a systematic score undercount, but these
particular inputs did not produce a type-checker resource failure. A broad
string scan would also reject ordinary strings such as the final row. The
typed positions need separate handling if the intended policy is to screen
every string that a type checker may parse as a forward annotation.

## Malformed recovery and unusual syntax

A targeted sweep repeated 14 malformed `if`-related phrases 500 times. The
forms that grew the recovered AST past 200 also exceeded Tokspam's 200 score.
Some forms, such as repeated `if 1 `, stayed shallow in both checks. No new
unbounded undercount appeared in this sweep.

The fuzzer's unusual-syntax campaign used the rebuilt local Tokspam binary,
Python 3.15.0, and its isolated Ruff probe:

```sh
python3 tools/unusual_campaign.py \
  --probe /tmp/tokspam-fuzzer-current-target/release/tokspam-fuzzer \
  --ruff-probe tools/ruff-probe/target/release/tokspam-ruff-probe \
  --output /tmp/tokspam-unusual-current-20261026 \
  --mutation-budget 1 --feedback-rounds 0 --timeout 0.7
```

It tested 370 generated cases across 37 syntax families. Five cases caused
CPython's isolated parser/compile probe to raise `MemoryError`; Tokspam
rejected all five for nesting. There were no accepted parser failures under
this campaign's 0.7-second and 512 MiB limits. This is a sampled search, not
a proof that other syntax cannot bypass the scores.

The raw local results are in `/tmp/tokspam-new-context-scout/results.jsonl`
and `/tmp/tokspam-unusual-current-20261026/results.jsonl`.
