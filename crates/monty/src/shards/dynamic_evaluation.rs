//! Implementation of the eval() and exec() builtin functions.
//!
//! Both compile their source at runtime against the session's intern tables
//! and push a frame for it, so the snippet runs like a called function: it
//! can suspend to the host, and its result lands on the caller's stack.

use std::{str, sync::Arc};

use monty_types::{BuiltinsFunctions, Shard, Shards};

use crate::{
    args::{ArgValues, FromArgs},
    bytecode::{CallResult, FrameGlobals, FrameNamespace, MissingNames, VM},
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, RunError, RunResult, SimpleException},
    heap::{DropGuard, HeapData, HeapId},
    prepare::SnippetNames,
    snippet::{SnippetSource, compile_and_push},
    types::{Type, py_trait::PyTrait},
    value::Value,
};

pub(crate) const SHARD: Shard = Shard::DynamicEvaluation;
pub(crate) const BUILTINS: [BuiltinsFunctions; 2] = [BuiltinsFunctions::Eval, BuiltinsFunctions::Exec];

fn require(shards: Shards) -> RunResult<()> {
    if shards.contains(SHARD) {
        Ok(())
    } else {
        Err(SimpleException::new_msg(ExcType::PermissionError, "dynamic evaluation shard is disabled").into())
    }
}

/// Arguments of `eval(source, /, globals=None, locals=None)`.
#[derive(FromArgs)]
#[from_args(name = "eval", at_most_total)]
struct EvalArgs {
    #[from_args(pos_only)]
    source: Value,
    #[from_args(default = Value::None)]
    globals: Value,
    #[from_args(default = Value::None)]
    locals: Value,
}

/// Arguments of `exec(source, /, globals=None, locals=None, *, closure=None)`.
#[derive(FromArgs)]
#[from_args(name = "exec")]
struct ExecArgs {
    #[from_args(pos_only)]
    source: Value,
    #[from_args(default = Value::None)]
    globals: Value,
    #[from_args(default = Value::None)]
    locals: Value,
    #[from_args(kw_only, default = Value::None)]
    closure: Value,
}

/// Implementation of the `eval()` builtin function.
///
/// Parses `source` as one expression and runs it in the given (or the
/// caller's) namespace; the expression's value is the call's result.
pub fn builtin_eval(vm: &mut VM<'_>, args: ArgValues) -> RunResult<CallResult> {
    let EvalArgs {
        source,
        globals,
        locals,
    } = EvalArgs::from_args(args, vm)?;
    defer_drop!(source, vm);
    defer_drop!(globals, vm);
    defer_drop!(locals, vm);

    require(vm.env.options.shards)?;
    let text = snippet_source("eval", source, vm)?;
    let globals = globals_dict(globals, Builtin::Eval, vm)?;
    let locals = locals_dict(locals, Builtin::Eval, vm)?;
    run_snippet(Builtin::Eval, &text, globals, locals, vm)
}

/// Implementation of the `exec()` builtin function.
///
/// Compiles `source` as a module and runs it in the given (or the caller's)
/// namespace; the call's result is `None`.
pub fn builtin_exec(vm: &mut VM<'_>, args: ArgValues) -> RunResult<CallResult> {
    let ExecArgs {
        source,
        globals,
        locals,
        closure,
    } = ExecArgs::from_args(args, vm)?;
    defer_drop!(source, vm);
    defer_drop!(globals, vm);
    defer_drop!(locals, vm);
    defer_drop!(closure, vm);

    if !matches!(closure, Value::None) {
        return Err(ExcType::type_error(
            "closure can only be used when source is a code object",
        ));
    }
    require(vm.env.options.shards)?;
    let text = snippet_source("exec", source, vm)?;
    let globals = globals_dict(globals, Builtin::Exec, vm)?;
    let locals = locals_dict(locals, Builtin::Exec, vm)?;
    run_snippet(Builtin::Exec, &text, globals, locals, vm)
}

/// Which builtin is running: the two parse differently and word their
/// argument errors differently.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Builtin {
    Eval,
    Exec,
}

/// Compiles `source` in the namespace `globals` / `locals` describe (borrowed
/// dict ids, validated by the caller) and pushes its frame.
fn run_snippet(
    builtin: Builtin,
    source: &Arc<str>,
    globals: Option<HeapId>,
    locals: Option<HeapId>,
    vm: &mut VM<'_>,
) -> RunResult<CallResult> {
    reject_null_bytes(source)?;
    for dict in globals.into_iter().chain(locals) {
        vm.heap.inc_ref(dict);
    }
    let (names, namespace) = vm.snippet_namespace(globals, locals)?;
    let kind = match builtin {
        Builtin::Eval => SnippetSource::Eval,
        Builtin::Exec => SnippetSource::Exec,
    };
    compile_and_push(kind, source, names, namespace, vm)?;
    Ok(CallResult::FramePushed)
}

/// Evaluates one annotation string for `typing.get_type_hints()`, `ForwardRef.evaluate()`
/// and `get_annotations(eval_str=True)`.
///
/// Runs in borrowed `globals` / `locals` dicts (`None` globals means the module slots),
/// with `globals_first` letting bound slot globals shadow `locals`, and `missing`
/// choosing between `NameError` and a `ForwardRef` stringifier for an unresolved
/// name. A `forward_ref` compiles as CPython's `ForwardRef` does: a compile error or
/// leading indentation raises its "must be an expression" error.
///
/// Each distinct text and mode compiles once per session, so names are always
/// resolved at run time: a snippet compiled when `int` was the builtin must see a
/// later `int = str`. The namespace is built here, not by `snippet_namespace`,
/// which would inherit the calling frame's: the caller is frozen code.
pub(crate) fn eval_hint(
    source: &Value,
    globals: Option<HeapId>,
    locals: Option<HeapId>,
    globals_first: bool,
    forward_ref: bool,
    missing: MissingNames,
    vm: &mut VM<'_>,
) -> RunResult<CallResult> {
    let mut missing_guard = DropGuard::new(missing, vm);
    let (_, vm) = missing_guard.as_parts_mut();
    require(vm.env.options.shards)?;
    let text = snippet_source("eval", source, vm)?;
    if forward_ref && first_code_line_indented(&text) {
        return Err(ExcType::forward_ref_not_expression(&text));
    }
    let not_expression = |err: RunError| {
        if forward_ref && err.is_catchable(ExcType::SyntaxError) {
            ExcType::forward_ref_not_expression(&text)
        } else {
            err
        }
    };
    reject_null_bytes(&text).map_err(not_expression)?;
    // The namespace owns `missing` from here, on every path.
    let (missing, vm) = missing_guard.into_parts();
    for dict in globals.into_iter().chain(locals) {
        vm.heap.inc_ref(dict);
    }
    let (globals, names) = match globals {
        Some(dict) => (FrameGlobals::Dict(dict), SnippetNames::NameOverDict),
        None => (FrameGlobals::Slots, SnippetNames::NameOverSlots),
    };
    let namespace = Box::new(FrameNamespace::Snippet {
        globals,
        locals,
        globals_first,
        missing,
    });
    if let Some(func_id) = vm.interns.hint_snippet(names, &text) {
        vm.push_snippet_frame(func_id, None, namespace)?;
    } else {
        let func_id = compile_and_push(SnippetSource::Eval, &text, names, namespace, vm).map_err(not_expression)?;
        vm.interns.set_hint_snippet(names, text, func_id);
    }
    Ok(CallResult::FramePushed)
}

/// The `SyntaxError` for snippet source holding a NUL, checked before any namespace is built.
fn reject_null_bytes(source: &str) -> RunResult<()> {
    if source.contains('\0') {
        Err(SimpleException::new_msg(ExcType::SyntaxError, "source code string cannot contain null bytes").into())
    } else {
        Ok(())
    }
}

/// Whether the first line of code in `source` is indented, which `compile()` rejects
/// but `eval()` strips. Like CPython's tokenizer, it skips blank and comment-only
/// lines, and a form feed resets the column.
fn first_code_line_indented(source: &str) -> bool {
    source
        .lines()
        .find_map(|line| {
            let mut indented = false;
            for c in line.chars() {
                match c {
                    ' ' | '\t' => indented = true,
                    '\x0c' => indented = false,
                    '#' => return None,
                    _ => return Some(indented),
                }
            }
            None
        })
        .unwrap_or(false)
}

/// The snippet's text: a `str`, or `bytes` decoded as UTF-8.
fn snippet_source(name: &str, source: &Value, vm: &mut VM<'_>) -> RunResult<Arc<str>> {
    if let Some(text) = source.as_str(vm) {
        return Ok(Arc::from(text));
    }

    let bytes: &[u8] = match source {
        Value::InternBytes(id) => vm.interns.get_bytes(*id),
        Value::Ref(id) => match vm.heap.get(*id) {
            HeapData::Bytes(b) => b.as_slice(),
            _ => return Err(source_type_error(name)),
        },
        _ => return Err(source_type_error(name)),
    };
    match str::from_utf8(bytes) {
        Ok(text) => Ok(Arc::from(text)),
        Err(err) => {
            let bad = bytes[err.valid_up_to()];
            let line = bytes[..err.valid_up_to()].split(|b| *b == b'\n').count();
            Err(SimpleException::new_msg(
                ExcType::SyntaxError,
                format!(
                    "Non-UTF-8 code starting with '\\x{bad:02x}' on line {line}, but no encoding declared; \
                     see https://peps.python.org/pep-0263/ for details (<string>, line {line})"
                ),
            )
            .into())
        }
    }
}

/// `TypeError` for a `source` that is neither text nor bytes.
fn source_type_error(name: &str) -> RunError {
    ExcType::type_error(format!("{name}() arg 1 must be a string, bytes or code object"))
}

/// Validates the `globals` argument: `None`, or a dict whose id is returned (borrowed).
fn globals_dict(globals: &Value, builtin: Builtin, vm: &mut VM<'_>) -> RunResult<Option<HeapId>> {
    match globals {
        Value::None => Ok(None),
        Value::Ref(id) if matches!(vm.heap.get(*id), HeapData::Dict(_)) => Ok(Some(*id)),
        other => {
            let ty = other.py_type(vm);
            Err(match builtin {
                // CPython distinguishes a non-dict mapping (anything subscriptable) from the rest.
                Builtin::Eval
                    if matches!(
                        ty.native(),
                        Some(Type::List | Type::Tuple | Type::Str | Type::Bytes | Type::Range | Type::Deque)
                    ) =>
                {
                    ExcType::type_error("globals must be a real dict; try eval(expr, {}, mapping)")
                }
                Builtin::Eval => ExcType::type_error("globals must be a dict"),
                Builtin::Exec => ExcType::type_error(format!(
                    "exec() globals must be a dict, not {}",
                    ty.name(vm.heap, vm.interns)
                )),
            })
        }
    }
}

/// Validates the `locals` argument: `None`, or a dict whose id is returned (borrowed).
fn locals_dict(locals: &Value, builtin: Builtin, vm: &mut VM<'_>) -> RunResult<Option<HeapId>> {
    match locals {
        Value::None => Ok(None),
        Value::Ref(id) if matches!(vm.heap.get(*id), HeapData::Dict(_)) => Ok(Some(*id)),
        other => Err(match builtin {
            Builtin::Eval => ExcType::type_error("locals must be a mapping"),
            Builtin::Exec => ExcType::type_error(format!(
                "locals must be a mapping or None, not {}",
                other.py_type(vm).name(vm.heap, vm.interns)
            )),
        }),
    }
}
