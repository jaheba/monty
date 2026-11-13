# Typed execution

The optional Rust feature `typed-ir` compiles eligible typed functions to guarded register instructions.
Build the CLI with `cargo build --release -p monty-runtime --features typed-ir`, then run `monty --typed-ir program.py`.
For a file or `-c` program, `--typed-ir` rejects type-checking diagnostics before execution.
The standalone flag requires one of those inputs and rejects interactive mode.

## Type hints and fallback

Hints select candidate implementations.
Runtime guards check actual argument types, accessed container values, integer overflow and operation domains.
A guard failure resumes ordinary execution with the original arguments.
Incorrect hints do not introduce runtime type errors by themselves.
Primitive guards require exact runtime types.
An integer accepted by the checker for a `float` parameter falls back instead of changing the runtime value's type.

Eligible bodies include scalar arithmetic and branches, range loops, read-only scalar list/tuple loops,
sequence indexing and unpacking, and string-keyed dictionary reads returning scalars.
Other bodies retain ordinary execution and Monty's existing [Python subset](limitations/index.md).

Functions containing Python-visible effects or suspension points use ordinary execution.
Eligible pure helpers called before or after a suspension can use typed instructions.
Snapshots contain bytecode and hints; compiled instruction caches are rebuilt after loading.
Existing [snapshot trust requirements](snapshots.md) apply.

## Embedding and costs

Rust embedders enable the feature and call `monty::typed_ir::set_enabled(true)` on the execution thread.
They arrange [type validation](type-checking.md) separately.
Protocol workers launched with `--typed-ir` retain the parent's `Configure.type_check` setting.
Runtime guards remain active independently of validation.
Python and JavaScript bindings do not expose this option.

Type analysis happens during compilation when typed execution is enabled.
Code compiled before enabling it is analyzed on its first eligible call.
The first eligible call creates specialized register instructions, cached for subsequent calls of the same code object.
An ordinary Rust loop executes unboxed values and specialized container reads.
Short functions can cost more to compile than they save.
Compiled loops retain source-instruction checkpoints for resource limits.
Original arguments remain VM roots, and temporary scalar buffers belong to the invocation.
