# Experimental typed compilation

The optional Rust feature `experimental-aot` enables guarded native execution of eligible typed functions.
It is an experimental prototype; published Python and JavaScript bindings do not expose a setting for it.
Build the CLI with `cargo build --release -p monty-runtime --features experimental-aot`, then run `monty --aot program.py`.
Standalone `--aot` checks types and rejects diagnostics before executing the file or `-c` program.
Interactive CLI execution is not supported with this experimental flag.

Threaded dispatch is a separate `experimental-threaded` feature.
Build with `--features experimental-aot,experimental-threaded` to combine it with typed compilation.
Without that feature, AOT uses the existing match dispatcher.
The two choices remain independent because dispatcher performance varies by workload.

## Type hints and fallback

Hints select candidate implementations.
Runtime guards check actual argument types, accessed container values, integer overflow and operation domains.
A guard failure resumes ordinary execution with the original arguments.
An incorrect hint therefore does not introduce a runtime type error by itself.
Primitive guards require exact runtime types.
For example, an integer accepted by the checker for a `float` parameter falls back rather than being coerced,
because coercion could change the function's result type and Python behavior.
The program retains Monty's existing [Python subset](limitations/index.md).

Eligible bodies include scalar arithmetic and branches, range loops, read-only scalar list/tuple loops,
sequence indexing and unpacking, and string-keyed dictionary reads returning scalars.
Unsupported bodies execute in the interpreter.
Unicode operations are outside this prototype.

Functions containing Python-visible effects or suspension points currently execute in the interpreter.
Pure typed helpers called before or after a suspension can use compiled execution.
Snapshots contain bytecode and hints; native code is rebuilt after loading.
Existing [snapshot trust requirements](snapshots.md) still apply.

## Embedding and costs

Rust embedders enable the feature and call `monty::experimental_aot::set_enabled(true)` on the execution thread.
They arrange [type validation](type-checking.md) separately.
Protocol workers launched with `--aot` preserve the parent's `Configure.type_check` setting.
The runtime guards remain active whether or not the host requested validation.

Type analysis precedes execution of each eligible function.
The first eligible call links operation records to precompiled Rust templates, and subsequent calls share the result.
The templates follow the submitted function's typed operations and branch links.
This tier removes boxing and redundant VM stack traffic; whole-function machine-code generation and
cross-operation native optimization remain future work.
The default shared backend allocates no per-program executable memory.
`--aot-copied` selects the copied-code comparison backend; `--aot-shared` explicitly selects the default.
`--aot --no-native` retains strict validation while disabling optimized execution.
There is no profiling warmup or persistent native-code cache.
Short functions can cost more to compile than they save; performance depends on how much eligible work the program performs.
The experimental `--native-stats` flag reports compilation, return and fallback counters on stderr.

Compiled loops retain instruction checkpoints for resource limits.
Original arguments remain VM roots, and scalar container buffers are owned by the invocation.
The copied backend's executable mappings have an explicit allocator-accounting lease and an independent process-wide mapping cap.
Source-instruction fuel is separate from the combined interpreter's bounded Rust handler-chain depth.
Native checkpoints replenish fuel without extending that chain.
On architectures without the native backend, the feature retains ordinary interpreter execution.
