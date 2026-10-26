# Monty / Tokspam comparison

Monty: `45552c73cb1743bad6b0bac9adfbf90c0e5af644`.
Tokspam: `d0fb3100aff5161025a7601a2bc87fddee0280ff`.
Compiler: `rustc 1.99.0 (b940084d7 2026-09-28)`.
Tokspam tracked patch SHA-256: `ec9d044f5a60fcd84370be238a25c2f5df6990dd9738dd33375a6a1d37f8dcf2`.

3404 inputs; 4 process failures.
75 type-check comparisons skipped because the forced parser scan rejects input that the 4 KiB shortcut permits.

Limits are 200 for parser and AST depth, and 8 for quoted annotations.
The adjusted policy uses AST depth 199; it changes no other limit.

## Decisions

| Group | Inputs | Parser differences | Type differences | Adjusted type differences |
| --- | ---: | ---: | ---: | ---: |
| Generated | 1682 | 35 | 108 | 99 |
| Monty fixtures | 710 | 0 | 0 | 0 |
| Monty fixtures (forced scan) | 710 | 0 | 0 | 0 |
| Attack corpus | 149 | 1 | 3 | 3 |
| Attack corpus (forced scan) | 149 | 1 | 3 | 3 |

## Process failures

Failures are excluded from the decision and timing tables.

### attack:postfix-mix-aa70e35d95af

Exit: `-6`.

```text
spam: {"parser_ok":true,"type_error":"AstDepth","type_ok":false}
phase: production AST guard at limit 200

thread 'main' (37114169) has overflowed its stack
fatal runtime error: stack overflow, aborting

```

### attack:postfix-mix-aa70e35d95af#forced

Exit: `-6`.

```text
spam: {"parser_ok":true,"type_error":"AstDepth","type_ok":false}
phase: production AST guard at limit 200

thread 'main' (37114177) has overflowed its stack
fatal runtime error: stack overflow, aborting

```

### attack:postfix-mix-b5da4501d238

Exit: `-6`.

```text
spam: {"parser_ok":true,"type_error":"AstDepth","type_ok":false}
phase: production AST guard at limit 200

thread 'main' (37114185) has overflowed its stack
fatal runtime error: stack overflow, aborting

```

### attack:postfix-mix-b5da4501d238#forced

Exit: `-6`.

```text
spam: {"parser_ok":true,"type_error":"AstDepth","type_ok":false}
phase: production AST guard at limit 200

thread 'main' (37114215) has overflowed its stack
fatal runtime error: stack overflow, aborting

```


## Scores at ten repetitions

Monty scores are the smallest limit at which its existing check passes.
Tokspam scores come from unlimited `analyze`; stopped scans have partial scores.

A null Monty score means either the score exceeds the probe cap of 512, or AST parsing was skipped.

| Shape | Monty parser | Spam parser | Monty AST | Spam AST |
| --- | ---: | ---: | ---: | ---: |
| parentheses | 10 | 10 | 1 | 10 |
| lists | 10 | 10 | 11 | 10 |
| unary | 10 | 10 | 11 | 10 |
| not | 10 | 10 | 11 | 10 |
| power | 10 | 10 | 11 | 10 |
| lambda | 20 | 20 | 11 | 20 |
| conditional | 20 | 20 | 11 | 20 |
| addition | 0 | 0 | 11 | 10 |
| attributes | 0 | 0 | 11 | 10 |
| calls | 1 | 1 | 11 | 10 |
| subscripts | 1 | 1 | 11 | 10 |
| union | 0 | 0 | 11 | 10 |
| boolean | 0 | 0 | 2 | 0 |
| comparison | 0 | 0 | 2 | 0 |
| siblings | 2 | 2 | 3 | 2 |
| indentation | 10 | 10 | 1 | 10 |
| annotation_parens | 10 | 10 | 2 | 10 |
| annotation_union | 0 | 0 | 12 | 10 |
| annotation_subscript_union | 1 | 1 | 13 | 12 |
| annotation_nested_union | 1 | 1 | 14 | 12 |
| nested_calls | 10 | 10 | 11 | 20 |
| left_grouped | 10 | 10 | 11 | 20 |
| mixed_postfix | 1 | 1 | 31 | 30 |
| lambda_parameters | 2 | 2 | 2 | 2 |
| comprehension | 1 | 1 | 2 | 1 |
| ordinary_string | 10 | 0 | 2 | 0 |
| literal_metadata | 10 | 1 | 3 | 2 |
| annotated_metadata | 10 | 1 | 4 | 2 |
| escaped_annotation | 0 | 10 | 1 | 10 |
| format_spec | 22 | 22 | 2 | 22 |
| case_pattern | 11 | 11 | 11 | 21 |

## Decision differences by shape

Each shape includes raw forced scans, raw scans with the 4 KiB shortcut, and padded sources above 4 KiB.
Counts include syntactically invalid sources; a guard accepting input does not mean Python accepts it.

| Shape | Parser differences | Type differences | First differing input |
| --- | ---: | ---: | --- |
| lists | 0 | 3 | `lists/200/0/raw` |
| unary | 0 | 3 | `unary/200/0/raw` |
| not | 0 | 3 | `not/200/0/raw` |
| power | 0 | 3 | `power/200/0/raw` |
| addition | 0 | 3 | `addition/200/0/raw` |
| attributes | 0 | 3 | `attributes/200/0/raw` |
| calls | 0 | 3 | `calls/200/0/raw` |
| subscripts | 0 | 3 | `subscripts/200/0/raw` |
| union | 0 | 3 | `union/200/0/raw` |
| annotation_union | 0 | 6 | `annotation_union/199/0/raw` |
| annotation_subscript_union | 0 | 3 | `annotation_subscript_union/198/0/raw` |
| annotation_nested_union | 0 | 3 | `annotation_nested_union/198/0/raw` |
| nested_calls | 0 | 9 | `nested_calls/101/0/raw` |
| left_grouped | 0 | 9 | `left_grouped/101/0/raw` |
| ordinary_string | 8 | 8 | `ordinary_string/201/0/raw` |
| literal_metadata | 8 | 8 | `literal_metadata/201/0/raw` |
| annotated_metadata | 8 | 8 | `annotated_metadata/201/0/raw` |
| escaped_annotation | 8 | 12 | `escaped_annotation/201/0/raw` |
| case_pattern | 0 | 12 | `case_pattern/100/0/raw` |
| named_escape | 1 | 1 | `named_escape` |
| unterminated | 1 | 1 | `unterminated` |
| unclosed_bracket | 1 | 1 | `unclosed_bracket` |
| attack:attack-elif-chain-0fca47a2572b | 0 | 1 | `attack:attack-elif-chain-0fca47a2572b` |
| attack:attack-elif-chain-0fca47a2572b#forced | 0 | 1 | `attack:attack-elif-chain-0fca47a2572b#forced` |
| attack:attack-elif-chain-747772475413 | 0 | 1 | `attack:attack-elif-chain-747772475413` |
| attack:attack-elif-chain-747772475413#forced | 0 | 1 | `attack:attack-elif-chain-747772475413#forced` |
| attack:attack-elif-chain-e03498368fc3 | 0 | 1 | `attack:attack-elif-chain-e03498368fc3` |
| attack:attack-elif-chain-e03498368fc3#forced | 0 | 1 | `attack:attack-elif-chain-e03498368fc3#forced` |
| attack:case-name-add-517f3814dc6e | 1 | 0 | `attack:case-name-add-517f3814dc6e` |
| attack:case-name-add-517f3814dc6e#forced | 1 | 0 | `attack:case-name-add-517f3814dc6e#forced` |

## Exploratory timings

Release build; per-input averages over 3–30 calls, with reusable Tokspam checkers.
These short runs are not a stable benchmark. Timings exclude input generation and score probing.
Ratios below are the median of Monty time / Tokspam time, restricted to inputs both guards accept.
Parser timings additionally exclude sources skipped by the length shortcut.

| Group | Parser ratio | Type ratio |
| --- | ---: | ---: |
| Generated | 0.47× | 3.02× |
| Monty fixtures | 0.83× | 3.01× |
| Monty fixtures (forced scan) | 0.81× | 3.42× |
| Attack corpus | 0.89× | 3.32× |
| Attack corpus (forced scan) | 0.85× | 3.96× |

Raw inputs, results, and revision metadata are in the output directory.
