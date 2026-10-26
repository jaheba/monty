# Compare Monty's nesting guards with Tokspam

Run from the repository root with a local Tokspam checkout:

```sh
python3 scripts/nesting_comparison/run.py \
  --spam /path/to/tokspam \
  --output /tmp/monty-nesting-comparison \
  --corpus crates/monty/test_cases \
  --generator-manifest /path/to/tokspam-fuzzer/corpus/manifest.json

python3 scripts/nesting_comparison/summarize.py /tmp/monty-nesting-comparison

python3 scripts/nesting_comparison/check_types.py /tmp/monty-nesting-comparison
```

`--corpus` reads Python files as source and can be repeated.
`--generator-manifest` executes trusted local generator scripts and checks each output against its recorded SHA-256.
Use the manifest for the Tokspam attack corpus: its `cases/*.py` files generate the attack sources.
Omit that argument to compare only generated shapes and Monty fixtures.

The runner builds a temporary Rust crate with the production `source_nesting.rs` included directly.
It uses the current Ruff version, the release nesting limit of 200, and a local dependency named `spam`.
It leaves Monty's dependencies and runtime behavior unchanged.
Cargo runs offline; dependencies must already be cached.
The type-checker runner accepts `--online` to fetch missing dependencies.
It preserves the repository lockfile, including pinned dependencies that have since been yanked.

Each input runs in a separate process with a ten-second timeout.
Results include process failures, completed or partial Tokspam scores, guard decisions, and exploratory timings.
The output directory contains the inputs, JSONL results, revision metadata, and compiled probe.

## What is compared

- Compiler guard: the existing parser-recursion scan versus `max_nesting_depth = 200`, with the same 4 KiB shortcut.
- Type-checking guard: the existing parser scan plus recovered-AST walk versus parser and AST limits of 200.
- Adjusted type policy: the same Tokspam policy with AST depth 199 to test the common leaf-count offset.

Tokspam also receives an annotation-depth limit of eight, matching Monty's intended quoted-string bound.
Other Tokspam score limits are disabled, and incomplete scans count as rejection.
Generated shapes run with forced scanning, the production length shortcut, and padding above that shortcut.
Corpus inputs run with both forced scanning and the production shortcut.

Monty's depth numbers are the minimum passing limits, found by binary search up to 512.
A null parser score means no tested limit passes, including rejection by the separate quoted-string depth cap.
A null AST score means either the AST exceeds that cap or the forced parser scan rejected the source.
The recursive visitor is never called if the forced parser scan rejects at 200.
Consequently, some short inputs have unknown current type-checking decisions despite passing the length shortcut.
They are excluded from decision differences, rather than assumed to pass or fail.
For other inputs, the type-checking comparison calls the same two production functions in the same order.

The AST check parses a recovered AST even for syntactically invalid source, as the production guard does.
Guard acceptance says nothing about syntax validity, compiler support, or downstream parser safety.
This experiment compares the guards; it does not run ty or compile and execute the input programs.

## Differences to resolve before replacement

The baseline is in [results.md](results.md).
The rerun after fixing annotation failures and malformed `if` accounting is in
[results-after-fixes.md](results-after-fixes.md).
The fixes make all 710 Monty fixture guard decisions agree and reject the repeated-`if` attack with `AstDepth`.
The depth-model differences below remain; descriptions of the two fixed cases refer to the baseline.
The extended [depth follow-up](results-depth-followup.md) adds quoted-layer accounting and eight malformed-conditional
families, and records actual type-checker outcomes.

Each quoted-type rescan now charges one AST level and reduces the child's remaining AST budget by one.
The zero-based leaf convention remains unchanged; exact equality with an expression-node count is not required.
The added malformed-source cases still have bounded score offsets at the acceptance boundary.
Accepted-source type checking found six timeouts and two allocator-limit aborts under the probe's resource caps.
Seven also pass the existing guard, so these results do not establish that either depth policy bounds type-checker cost.

`check_types.py` runs accepted generated and attack sources in separate processes with a 10-second timeout and 512 MiB
allocator ceiling; it does not execute Python source.
Use `--reuse PREVIOUS_OUTPUT` to reuse recorded outcomes for byte-identical sources and `--target-dir PATH` to reuse build
artifacts.
The result rows mark reused evidence explicitly; the summary includes both old and new runs.

Parser estimates match for the simple bracket, operator, lambda, indentation, and formatting shapes.
String handling differs: Monty scans every sufficiently long string as possible annotation source, while Tokspam selects
likely annotation positions and excludes `Literal` values and `Annotated` metadata.
Tokspam also decodes annotation escapes; Monty's initial scan measures their raw spelling.

AST scores are different models, rather than one scale with a fixed offset:

- Ten additions, attributes, or unary operators have Monty depth 11 and Tokspam depth 10.
- Ten nested calls or explicitly grouped additions have Monty depth 11 and Tokspam depth 20.
- Ten grouping parentheses have Monty depth 1 and Tokspam depth 10.
- Ten union operators inside a quoted annotation have Monty depth 12 and Tokspam depth 10.
  Additional quoted layers change the difference.

At the same limit, some ordinary chains pass Tokspam one level beyond Monty's AST limit.
Lowering Tokspam's AST limit to 199 fixes those boundaries but introduces additional conservative rejections.
It does not correct nested-annotation offsets or double-counted structural shapes.

The Monty fixture `class__annotations.py` also stops Tokspam with `LexicalStructure`.
Small reproducers include `x: "it's"`, `x: r'raw\d'`, and `x: "a" r"b\n"`.
These are valid outer Python syntax whose annotation contents are not valid expressions.
A replacement needs a policy for failed annotation scans that preserves accepted outer source without treating partial
depth measurements as complete.

Timings are short release-build measurements, not a stable benchmark.
They compare guard work only and reuse Tokspam's allocated scratch between calls.
The AST path can save a full Ruff parse, while replacing the lexer-only guard can cost more on ordinary source.

The attack corpus exposes additional differences:

- Two large mixed-postfix sources abort with stack overflow in the production AST guard at limit 200, before score probing.
  Tokspam's type policy rejects both with `AstDepth` before building an AST.
  This is a standalone release probe, not an end-to-end worker or ty run.
- `x = 1 ` followed by 300,000 `if ` tokens passes Tokspam with AST score zero.
  Monty's recovered-AST check rejects it above the probe cap of 512.
  Replacing that check would remove an existing rejection on malformed input.
- Long `elif` chains have Monty expression depth one but Tokspam AST depth proportional to the number of clauses.
- A source using `case` as a variable triggers Monty's conservative pattern accounting, while Tokspam accepts its parser
  depth; its AST policy still rejects the long addition chain.
