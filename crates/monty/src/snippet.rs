//! Runtime compilation shared by imports and dynamic evaluation.

use std::sync::Arc;

use ruff_text_size::TextSize;

use crate::{
    args::Signature,
    bytecode::{Compiler, FrameNamespace, VM},
    exception_private::{ExcType, RunResult, SimpleException},
    expressions::{Identifier, Node},
    function::Function,
    heap::DropGuard,
    intern::{CompileInterns, FunctionId, StaticStrings},
    name_map::NameMap,
    parse::{
        CodeRange, ParseError, ParsedModule, parse_expression_with_interner, parse_module_with_filename_id,
        reject_leading_bom,
    },
    prepare::{SnippetNames, prepare_snippet},
    value::Value,
};

/// What runtime-compiled source is: it decides how the source parses, the
/// filename its frames show and how its compile errors read.
#[derive(Clone, Copy)]
pub(crate) enum SnippetSource<'a> {
    /// An `eval()` expression, shown as `<string>`.
    Eval,
    /// An `exec()` module body, shown as `<string>`.
    Exec,
    /// An imported `.py` file's module body, shown as its virtual `path`.
    File { path: &'a str },
}

/// Compiles privately and publishes only after the snippet's frame is admitted.
///
/// Provisional global slots are released if compilation or admission fails.
pub(crate) fn compile_and_push(
    kind: SnippetSource<'_>,
    source: &Arc<str>,
    names: SnippetNames,
    namespace: Box<FrameNamespace>,
    vm: &mut VM<'_>,
) -> RunResult<FunctionId> {
    let globals_len = vm.global_names.len();
    let mut namespace_guard = DropGuard::new(namespace, vm);
    let (_, vm) = namespace_guard.as_parts_mut();
    match compile_snippet(kind, source, names, vm) {
        Ok((func_id, overlay)) => {
            // `push_snippet_frame` owns the namespace from here, on every path.
            let (namespace, vm) = namespace_guard.into_parts();
            let pushed = vm.push_snippet_frame(func_id, Some(overlay), namespace);
            if pushed.is_ok() {
                vm.globals.resize_with(vm.global_names.len(), || Value::Undefined);
            } else {
                vm.global_names.truncate(globals_len);
            }
            pushed.map(|()| func_id)
        }
        // The guard releases the namespace's dict references.
        Err(err) => {
            let (_, vm) = namespace_guard.as_parts_mut();
            vm.global_names.truncate(globals_len);
            Err(err)
        }
    }
}

/// Parses, prepares and compiles `source` into an unpublished overlay, returning
/// the snippet function's ID within it.
fn compile_snippet<'h>(
    kind: SnippetSource<'_>,
    source: &Arc<str>,
    names: SnippetNames,
    vm: &mut VM<'h>,
) -> RunResult<(FunctionId, CompileInterns<'h>)> {
    let mut overlay = CompileInterns::new(vm.interns);
    let filename_id = match kind {
        SnippetSource::File { path } => overlay.add_file_source(Arc::from(path), Arc::clone(source)),
        SnippetSource::Eval | SnippetSource::Exec => overlay.add_eval_source(Arc::clone(source)),
    };
    let options = vm.env.options;
    let compile_error = |e: ParseError| match kind {
        SnippetSource::File { path } => e.into_file_run_error(path, source),
        SnippetSource::Eval | SnippetSource::Exec => e.into_run_error(source),
    };
    // `eval()` / `exec()` inherit the caller's `from __future__ import annotations`,
    // as CPython's do; an imported file starts afresh.
    let inherited_future = match kind {
        SnippetSource::Eval | SnippetSource::Exec => vm.frame_future_annotations(),
        SnippetSource::File { .. } => false,
    };
    let ParsedModule {
        nodes,
        future_annotations,
    } = match kind {
        // A file read from disk may open with a BOM; an `exec()` string may not.
        SnippetSource::Exec => reject_leading_bom(source, TextSize::default(), filename_id).and_then(|()| {
            parse_module_with_filename_id(
                source,
                filename_id,
                &mut overlay,
                options.source_scan_threshold,
                inherited_future,
            )
        }),
        SnippetSource::File { .. } => parse_module_with_filename_id(
            source,
            filename_id,
            &mut overlay,
            options.source_scan_threshold,
            inherited_future,
        ),
        SnippetSource::Eval => {
            parse_expression_with_interner(source, filename_id, &mut overlay, options.source_scan_threshold).map(
                |expr| ParsedModule {
                    nodes: vec![Node::Return(Some(expr))],
                    future_annotations: inherited_future,
                },
            )
        }
    }
    .map_err(compile_error)?;

    let globals_by_name = names == SnippetNames::NameOverDict;
    let mut scratch = NameMap::new();
    let globals = if globals_by_name {
        &mut scratch
    } else {
        &mut *vm.global_names
    };
    let nodes = prepare_snippet(nodes, &overlay, globals, names, options.shards).map_err(compile_error)?;
    let code = Compiler::compile_snippet(
        &nodes,
        &mut overlay,
        globals,
        options,
        globals_by_name,
        future_annotations,
    )
    .map_err(|e| match kind {
        SnippetSource::File { path } => e.into_file_run_error(path, source),
        SnippetSource::Eval | SnippetSource::Exec => e.into_run_error(source),
    })?;

    let position = CodeRange {
        filename: filename_id,
        start_byte: 0,
        end_byte: u32::try_from(source.len()).unwrap_or(u32::MAX),
    };
    let function = Function::new(
        Identifier::new(overlay.intern_static(StaticStrings::Module), position),
        Signature::default(),
        0,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        0,
        false,
        code,
    );
    let index = overlay.functions_len();
    let func_id = u16::try_from(index).map(FunctionId::from_index).map_err(|_| {
        SimpleException::new_msg(
            ExcType::SyntaxError,
            format!("session defines too many functions; maximum is {}", u16::MAX),
        )
    })?;

    overlay.push_function(function);
    Ok((func_id, overlay))
}
