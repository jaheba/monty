# monty-macros

Procedural macros used by the [`monty`](https://crates.io/crates/monty) crate.
Not a public crate — consumers get the macros re-exported
(`monty::args::FromArgs` / `monty::args::ToArgs`) and should not depend on
`monty-macros` directly.

## `#[derive(FromArgs)]`

Generates the `from_args` body for a Rust-implemented Python function. The
macro is deliberately thin: it emits a `static ParamSpec` describing the
signature and one call to the runtime binder
(`crates/monty/src/args/bind_native.rs`), which owns all positional/kwarg
dispatch, arity checks, duplicate detection, and reference-count cleanup.
The generated code then converts each raw slot via `FromValue` in
declaration order and applies defaults.

```rust
use monty::args::FromArgs;
use monty::value::Value;

#[derive(FromArgs)]
#[from_args(name = "function", style = c)]
struct DatetimeArgs {
    year: i32,
    month: i32,
    day: i32,
    #[from_args(default = 0)]
    hour: i32,
    #[from_args(default = Value::None)]
    tzinfo: Value,
}

let DatetimeArgs { year, month, day, hour, tzinfo } =
    DatetimeArgs::from_args(args, vm)?;
```

Fields must appear in Python signature order:
`[pos_only…] [pos_or_keyword…] [varargs] [kw_only…] [varkwargs]`, with
required fields before optional ones in each region. Field types must
implement `FromValue` (impls live in `monty::args::from_value`). Coercion
failures are structured (`FromValueFail`): wrong-type failures get their
wording from the extraction site (`bad_arg`/`bad_arg_named`, or the impl's
`type_error`), while value-level failures (`ValueError`, `OverflowError`)
surface unchanged. `str` arguments the function only reads should use
`StrArg`, which validates without copying the text and lends `&str` via
`as_str(vm)`.

### `style` — pick the CPython parser family

The `style` attribute names the argument parser the target function uses in
CPython. It selects both the error *wording* and the error *ordering*
(binding errors vs conversion errors — see `ErrorFamily` in
`crates/monty/src/args/bind_native.rs` for the exact contracts). Pick it by
looking at how the function is implemented in CPython:

| CPython implementation | `style` | tell-tale error wording |
|---|---|---|
| pure-Python `def` (`re` module, `json.dumps`, `Path.mkdir`) | `def` | `f() takes from 1 to 2 positional arguments but 3 were given` |
| Argument Clinic — most modern builtins/methods (look for clinic blocks / `*.c.h` includes in the CPython source) | `clinic` (the default — omit it) | `replace() takes at least 2 positional arguments (1 given)` |
| `PyArg_ParseTupleAndKeywords`, no `:name` in the format string | `c` | `function missing required argument 'day' (pos 3)` |
| `PyArg_ParseTupleAndKeywords` with `:name` | `c_named` | `timezone() missing required argument 'offset' (pos 1)` |
| `PyArg_UnpackTuple` (positional-only, fixed `min..max`) | `unpack` | `name expected at most 2 arguments, got 3` (`expected N …` when min == max) |

Style-derived behaviour that used to be separate flags: the C
"… positional arguments …" overflow pivot turns on automatically for
`style = c` and `style = c_named` structs with `kw_only` fields (CPython's
`vgetargskeywords` / `_PyArg_UnpackKeywords` behave the same way; the named
variant additionally says `takes exactly N positional argument(s)` when every
positional param is required, e.g. `os.stat`), and `unpack` collapses to the
exact-arity `expected N argument(s)` wording when no positional field has a
default.

Since CPython's `def` binding never type-checks, `style = def` structs
should declare fields as raw `Value` (or `StrArg`-in-body) and coerce in the
function body, so type errors carry the message the CPython function body
would produce. `bad_arg` is rejected under `style = def` for the same
reason. `varargs` is allowed: a `*args` signature never overflows, but a
missing required parameter still wants the `def` wording.

`style = unpack` models `PyArg_UnpackTuple`'s fixed positional `min..max`
range, so the derive rejects anything outside that shape: every positional
field must be `pos_only`, and `varargs`, `varkwargs`, and `at_most_total`
are all incompatible.

### Modifiers

- `at_most_total` — pre-count positionals + kwargs against all named parameters,
  including keyword-only parameters, before dispatch (`{name}() takes at most N arguments (M given)`).
  This is a per-function empirical fact, not derivable from the fields or
  the style. Litmus test: call the CPython function with valid positionals
  plus one bogus kwarg — if it reports `takes at most N arguments (M
  given)`, set the flag; if it reports `unexpected keyword argument`,
  don't. Only meaningful for the C-parser families (`clinic`/`c`/`c_named`)
  on signatures with a fixed maximum — rejected under `style = def` /
  `style = unpack` and with `varargs`/`varkwargs`.
- `vectorcall` — kwarg-free calls check positional arity first with
  `_PyArg_CheckPositional` wording (`{name} expected at most N arguments,
  got M`), modeling `tp_vectorcall` fast paths (`int`, `str`) that only
  fall back to the clinic parser when keywords are present. Requires the
  default `clinic` style plus `at_most_total` (which supplies the
  parenthesised wording for the kwargs path).
- `bad_arg` / `bad_arg_named` — report `FromValue` wrong-type failures in
  CPython's `_PyArg_BadArgument` wording (`{name}() argument {pos|'arg'}
  must be {expected}, not {got}`).
- `kwarg_error_name = "..."` — override the function name in the
  unknown-kwarg error only (`json.dumps` reports `JSONEncoder.__init__`).
  Under `style = unpack` it instead names the function in the blanket
  `takes no keyword arguments` rejection (`unicodedata.name()`).
- `kwargs_not_supported_yet` — reject every kwarg with a
  `NotImplementedError`; a Monty TODO marker.

Field-level attributes: `pos_only`, `kw_only` (must carry a `default` — the
runtime binder's fast paths skip the missing-keyword check, so required
keyword-only params are rejected at derive time), `varargs` (must be
`Vec<Value>` — elements are handed over unconverted), `varkwargs`,
`default[ = expr]`, `static_string = "..."` (override the `StaticStrings`
variant used for kwarg matching; neither applies to `varargs`/`varkwargs`
fields). All are documented inline in
[`src/from_args.rs`](src/from_args.rs), which also carries `#[cfg(test)]`
unit tests for every attribute-validation error.

## `#[derive(ToArgs)]`

Projects a struct into the `CallArgs` a host callback takes, reusing the `#[from_args(...)]` field attributes.
`varargs` fields contribute one positional argument per element.

## `#[pyattrs]`

Collects Python-visible functions on a zero-sized namespace implementing `PyAttrs` inside `monty`.
The macro moves the functions into an inherent implementation and generates `PyAttrs::ATTRS` and a const
`lookup_attr(StaticStrings)` match that returns entries from that table.
Native object structs, method bodies, and serialization stay unchanged.

```rust,ignore
use monty_macros::pyattrs;

pub(crate) struct PyListImpl;

#[pyattrs]
impl<'heap> PyAttrs<'heap> for PyListImpl {
    type Receiver = HeapReadMut<'heap, List>;

    #[pymethod]
    fn sort<'h>(list: &HeapReadMut<'h, List>, args: ArgValues, vm: &mut VM<'h>) -> RunResult<Value> {
        todo!()
    }
}
```

`#[pymethod]` registers `AttrDef::Method`; it takes a native receiver, `ArgValues`, and `&mut VM`, returning
`RunResult<Value>` or `RunResult<CallResult>`.
The adapter wraps plain values and propagates call results, including suspended VM execution.
`Receiver` names the native view: `HeapRead<'heap, T>`, `HeapReadMut<'heap, T>`, `HeapObjectRead<'heap, T>`, `PyStr<'heap>`, `PyBytes<'heap>`, or an
immediate `f64`, `i64`, or `bool`.
Capability receivers take shared references: `&HeapRead` for reads and `&HeapReadMut` for mutation.
Object guards can also supply Python identity through `&HeapObjectRead`.
Adapters borrow the existing native view from `&mut PyValue`, reusing its heap handle.
Typed heap projections are generated from the payload registry, so native payload names need not match heap enum variants.
Immediate receivers are passed by value.
String methods take `&PyStr`, covering interned, inline, and heap strings without projecting their storage.
Bytes methods take `&PyBytes`, covering interned and heap bytes.

`#[pyclassmethod]` registers `AttrDef::ClassMethod` with the signature
`(StaticStrings, PyClass, ArgValues, &mut VM) -> RunResult<CallResult>`.
Its generated adapter supplies the registered static name, so callers only pass the class object, arguments, and VM.
`PyClass` is the bound class object, represented as a builtin class or a borrowed native class handle.
`#[pyvalue]` registers `AttrDef::Value` with `(&mut VM) -> Value`.
Names map from Rust snake case to `StaticStrings` variants: `sort` becomes `Sort`, `total_seconds` becomes
`TotalSeconds`, and `r#match` becomes `Match`.
Dunder names use the `Dunder` prefix: `__len__` becomes `DunderLen`.
Declaration order determines table order.

Shared native protocol adapters are registered in `monty/src/builtin_protocols.rs`.
`Type::attrs()` joins those entries with the generated per-type table at compile time;
lookup gives the per-type implementation precedence.

Functions keep their visibility, lifetime parameters, documentation, and bodies.
Unannotated helper functions are preserved without registration.
Annotations take no arguments; conditional attributes, `self` parameters, async functions, and type or const
parameters on registered functions are rejected.
Implementation types may have lifetime parameters; Rust rejects overlapping `PyAttrs` implementations.

String methods are defined directly in `impl<'h> PyAttrs<'h> for PyStr<'h>` with `type Receiver = PyStr<'h>`.
The generated callable borrows the `PyStr` view already held by `PyValue` before invoking the method.
Bytes methods likewise use `PyBytes` directly. Both views take shared references.

## Native protocol registrations

`monty/src/builtin_protocols.rs` contains adapters for explicitly called special methods.
For example, `items.__imul__(count)` calls the native in-place hook and returns the original receiver on success.
The VM's `items *= count` operation remains responsible for operator dispatch and binary fallback.
An explicit method call preserves its own result, including `NotImplemented`.

Lookup and binding use the same entry for bound and unbound calls:

1. `Type::lookup_attr` checks the generated per-type table, then the shared protocol table.
2. An instance call passes its existing `PyValue` view to the adapter.
3. A call through a builtin type uses `py_attrs::call_unbound` to extract and validate the first positional receiver.
4. The adapter enforces the method's argument rules and invokes the native operation.

`Type::attrs()` joins the generated and shared tables into a static array for enumeration.
The two tables must have disjoint Python names, even though lookup gives generated entries precedence.
The `tables_and_lookups_agree` test checks duplicate names and verifies that enumerated entries can be looked up.

### Adding a special method

Use `#[pymethod]` in the type's `PyAttrs` implementation when the method needs a concrete native receiver or type-specific
argument handling.
Use a shared adapter in `builtin_protocols.rs` when several types can use the same `PyValue` operation and calling rules.
Register its Python name in `StaticStrings`, add its spelling to the `entries!` macro, and opt each supported type into it
in `attrs`.
A defining-type variant can reuse the Python name: the `dict_eq` registration still exposes `__eq__`.

Check method availability on instances or in the type's class hierarchy.
For example, `hasattr(str, '__or__')` can find the metaclass's union method; it does not establish that string instances
expose `__or__`.
Similarly, internal iterator size hints do not establish a Python `__len__` method.
Keep the per-type lists explicit rather than inferring attributes from `PyTrait` defaults.

Operand handling must preserve the special method's own contract.
`int.__add__(1, 2.0)` returns `NotImplemented`, while `[1].__add__((2,))` raises `TypeError`.
The adapter must not use the complete operator path to obtain a result through reflected dispatch.
For sequence repetition, validate the signed index-sized count before an empty-sequence shortcut, and permit `__index__`
callbacks to mutate the receiver before its current contents or length are read.

Base descriptors can require different behavior from an instance's override.
`dict.__eq__(counter, other)` compares mapping entries, including zero counts, while `counter.__eq__(other)` uses Counter's
comparison rules.
Keep the original receiver alive and visible to callbacks; copying a mapping before repr or equality changes what those
callbacks observe.

Adapters consume `ArgValues` and return an owned result or a `CallResult` that can suspend.
Guard arguments and temporary owned values across errors, callback execution, and allocation.
Use the existing iterator/next helpers when they carry suspension behavior, such as loading an unread file's buffer.

Cover bound and unbound calls, invalid receivers, arity and operand errors, `NotImplemented`, and successful results.
For mutation methods, also check aliases, self-references, iterator invalidation, and callbacks that resize the target.
The direct-call fixtures and `builtin_dunder_regressions.rs` contain examples compared against CPython 3.14.
Initialization and standalone method objects remain outside this change.

## Not a standalone crate

Generated code emits `crate::...` paths: `FromArgs` compiles inside `monty`, and `ToArgs` inside `monty-types`.
Cross-crate use would need `proc-macro-crate` and paths to the appropriate crate.

## Monty crates

- [`monty`](https://crates.io/crates/monty) — the core interpreter: Python parser, bytecode VM, and sandbox.
- [`monty-types`](https://crates.io/crates/monty-types) — the shared boundary data types (values, exceptions, OS calls, resource limits) hosts use without linking the interpreter.
- [`monty-fs`](https://crates.io/crates/monty-fs) — host-side filesystem mounts: maps virtual sandbox paths to real host directories.
- [`monty-runtime`](https://crates.io/crates/monty-runtime) — the `monty` binary: REPL, file runner, and subprocess worker mode.
- [`monty-pool`](https://crates.io/crates/monty-pool) — an elastic pool of crash-isolated `monty` worker subprocesses.
- [`monty-proto`](https://crates.io/crates/monty-proto) — the protobuf wire protocol spoken between pool parents and workers.
- [`monty-type-checking`](https://crates.io/crates/monty-type-checking) — type checking of sandboxed code, powered by [ty](https://docs.astral.sh/ty/).
- [`monty-typeshed`](https://crates.io/crates/monty-typeshed) — the trimmed typeshed stubs describing the stdlib subset Monty implements.
- [`monty-macros`](https://crates.io/crates/monty-macros) — the proc macros behind `monty`'s argument parsing. **this crate**
