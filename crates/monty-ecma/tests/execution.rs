//! Frame cleanup and bytecode positions across host round-trips.

use std::time::Duration;

use monty_ecma::{JsDump, JsProgress, JsRepl, JsSessionRef, JsSuspension, JsSuspensionKind, dump};
use monty_types::{ExcType, ExtFunctionResult, MontyException, MontyObject, NamedValues, PrintWriter, ResourceLimits};

fn assert_no_leaks(repl: &mut JsRepl) {
    #[cfg(feature = "ref-count-return")]
    assert_eq!(repl.leaked_entries(), Vec::<String>::new());
    #[cfg(not(feature = "ref-count-return"))]
    let _ = repl;
}

fn restore(suspension: &JsSuspension) -> Box<JsSuspension> {
    let bytes = dump(JsSessionRef::Suspended(suspension)).unwrap();
    let JsDump::Suspended(suspension) = JsDump::load(&bytes).unwrap() else {
        panic!("expected a suspended dump");
    };
    suspension
}

#[test]
fn nested_closure_return_and_finally_survive_suspended_dump() {
    let repl = JsRepl::new("main.js", ResourceLimits::default()).unwrap();
    let progress = repl.feed(
        "let events = []; function outer(x) { return function(y) { try { return x + host(y); } finally { events.push('finally'); } }; } let result = outer(10)(2); [result, events.join(',')];",
        &NamedValues::new(),
        PrintWriter::Disabled,
    );
    let JsProgress::Suspended(suspension) = progress else {
        panic!("expected host call");
    };
    let mut suspension = restore(&suspension);
    match suspension.kind() {
        JsSuspensionKind::FunctionCall { function_name, .. } => assert_eq!(function_name, "host"),
        _ => panic!("expected function call"),
    }
    let progress = suspension.resume_call(ExtFunctionResult::Return(MontyObject::int(7)), PrintWriter::Disabled);
    let JsProgress::Complete { mut repl, value } = progress else {
        panic!("expected completion");
    };
    assert_eq!(value.py_repr(), "[17, 'finally']");
    assert_no_leaks(&mut repl);
    let bytes = dump(JsSessionRef::Idle(&repl)).unwrap();
    let JsDump::Idle(repl) = JsDump::load(&bytes).unwrap() else {
        panic!("expected idle dump");
    };
    let JsProgress::Complete { value, .. } = repl.feed("events.length", &NamedValues::new(), PrintWriter::Disabled)
    else {
        panic!("expected restored idle session");
    };
    assert_eq!(value, MontyObject::int(1));
}

#[test]
fn host_error_unwinds_nested_frames_and_runs_finally_once() {
    let repl = JsRepl::new("main.js", ResourceLimits::default()).unwrap();
    let progress = repl.feed(
        "let events = []; function inner() { try { return host(); } finally { events.push('inner'); } } function outer() { try { return inner(); } finally { events.push('outer'); } } try { outer(); } catch (e) { events.push(e.message); } events;",
        &NamedValues::new(),
        PrintWriter::Disabled,
    );
    let JsProgress::Suspended(suspension) = progress else {
        panic!("expected host call");
    };
    let suspension = restore(&suspension);
    let error = MontyException::new(ExcType::ValueError, Some("host failed".to_owned()));
    let JsProgress::Complete { mut repl, value } =
        suspension.resume_call(ExtFunctionResult::Error(error), PrintWriter::Disabled)
    else {
        panic!("expected caught host exception");
    };
    assert_eq!(value.py_repr(), "['inner', 'outer', 'host failed']");
    assert_no_leaks(&mut repl);
}

#[test]
fn promise_reactions_resume_once_after_a_suspended_dump() {
    let repl = JsRepl::new("main.js", ResourceLimits::default()).unwrap();
    let progress = repl.feed(
        "let events = []; let p = host(); p.then(x => { events.push(x); return x + 1; }).finally(() => events.push('finally')).then(x => events.push(x));",
        &NamedValues::new(),
        PrintWriter::Disabled,
    );
    let JsProgress::Suspended(mut suspension) = progress else {
        panic!("expected host call");
    };
    let JsSuspensionKind::FunctionCall { call_id, .. } = suspension.kind() else {
        panic!("expected function call");
    };
    let progress = suspension.resume_call(ExtFunctionResult::Future(call_id), PrintWriter::Disabled);
    let JsProgress::Suspended(suspension) = progress else {
        panic!("expected pending future");
    };
    let suspension = restore(&suspension);
    let progress = suspension.resume_futures(
        vec![(call_id, ExtFunctionResult::Return(MontyObject::int(4)))],
        PrintWriter::Disabled,
    );
    let JsProgress::Complete { repl, .. } = progress else {
        panic!("expected reactions to complete");
    };
    let JsProgress::Complete { value, .. } = repl.feed("events", &NamedValues::new(), PrintWriter::Disabled) else {
        panic!("expected retained events");
    };
    assert_eq!(value.py_repr(), "[4, 'finally', 5]");
}

#[test]
fn abort_runs_finally_before_the_error_escapes() {
    let repl = JsRepl::new("main.js", ResourceLimits::default()).unwrap();
    let mut output = String::new();
    let progress = repl.feed(
        "function f() { try { host(); } finally { console.log('cleanup'); } } f();",
        &NamedValues::new(),
        PrintWriter::collect_string(&mut output),
    );
    let JsProgress::Suspended(suspension) = progress else {
        panic!("expected host call");
    };
    let error = MontyException::new(ExcType::RuntimeError, Some("cancelled".to_owned()));
    let progress = restore(&suspension).abort(&error, PrintWriter::collect_string(&mut output));
    assert!(matches!(progress, JsProgress::Failed { .. }));
    assert_eq!(output, "cleanup\n");
}

#[test]
fn limits_remain_active_in_callbacks_coercions_and_promise_jobs() {
    for code in [
        "[1].map(() => { while (true) {} });",
        "String({toString() { while (true) {} }});",
        "function again() { queueMicrotask(again); } queueMicrotask(again);",
    ] {
        let limits = ResourceLimits::default().max_feed_duration(Duration::from_millis(5));
        let repl = JsRepl::new("limits.js", limits).unwrap();
        let JsProgress::Failed { error, .. } = repl.feed(code, &NamedValues::new(), PrintWriter::Disabled) else {
            panic!("expected a resource error for {code}");
        };
        assert!(error.summary().contains("time"), "{}", error.summary());
    }
}

#[test]
fn host_call_inside_a_promise_reaction_restores_its_completion() {
    let repl = JsRepl::new("reaction.js", ResourceLimits::default()).unwrap();
    let progress = repl.feed(
        "let events = []; Promise.resolve(2).then(x => { events.push('before'); return host(x); }).then(x => events.push(x)).finally(() => events.push('finally'));",
        &NamedValues::new(),
        PrintWriter::Disabled,
    );
    let JsProgress::Suspended(suspension) = progress else {
        panic!("expected reaction host call");
    };
    let progress =
        restore(&suspension).resume_call(ExtFunctionResult::Return(MontyObject::int(7)), PrintWriter::Disabled);
    let JsProgress::Complete { repl, .. } = progress else {
        panic!("expected reaction completion");
    };
    let JsProgress::Complete { mut repl, value } = repl.feed("events", &NamedValues::new(), PrintWriter::Disabled)
    else {
        panic!("expected events");
    };
    assert_eq!(value.py_repr(), "['before', 7, 'finally']");
    assert_no_leaks(&mut repl);
}

#[test]
fn array_map_callbacks_suspend_and_restore_each_iteration() {
    let repl = JsRepl::new("map.js", ResourceLimits::default()).unwrap();
    let mut progress = repl.feed(
        "let events = []; let input = [1, , 3]; let output = input.map(function(x, i, source) { try { events.push(i); if (i === 0) source.push(9); return host(x) + this.offset; } finally { events.push('finally'); } }, {offset: 10}); [output, events.join(','), input.length, 1 in output];",
        &NamedValues::new(), PrintWriter::Disabled,
    );
    for result in [2, 6] {
        let JsProgress::Suspended(suspension) = progress else {
            panic!("expected callback suspension");
        };
        progress = restore(&suspension).resume_call(
            ExtFunctionResult::Return(MontyObject::int(result)),
            PrintWriter::Disabled,
        );
    }
    let JsProgress::Complete { mut repl, value } = progress else {
        panic!("expected completion");
    };
    assert_eq!(value.py_repr(), "[[12, None, 16], '0,finally,2,finally', 4, False]");
    assert_no_leaks(&mut repl);
}

#[test]
fn array_map_callback_error_unwinds_partial_output() {
    let repl = JsRepl::new("map.js", ResourceLimits::default()).unwrap();
    let progress = repl.feed(
        "let events = []; try { [1, 2].map(x => { try { return host(x); } finally { events.push(x); } }); } catch (e) { events.push('caught'); } events.join(',');",
        &NamedValues::new(), PrintWriter::Disabled,
    );
    let JsProgress::Suspended(suspension) = progress else {
        panic!("expected host call");
    };
    let progress =
        restore(&suspension).resume_call(ExtFunctionResult::Return(MontyObject::int(1)), PrintWriter::Disabled);
    let JsProgress::Suspended(suspension) = progress else {
        panic!("expected second host call");
    };
    let error = MontyException::new(ExcType::ValueError, Some("failed".into()));
    let progress = restore(&suspension).resume_call(ExtFunctionResult::Error(error), PrintWriter::Disabled);
    let JsProgress::Complete { mut repl, value } = progress else {
        panic!("expected caught error");
    };
    assert_eq!(value.py_repr(), "'1,2,caught'");
    assert_no_leaks(&mut repl);
}

#[test]
fn array_map_bound_callback_can_suspend() {
    let repl = JsRepl::new("map.js", ResourceLimits::default()).unwrap();
    let progress = repl.feed(
        "[2].map(function(prefix, x) { return host(prefix + x); }.bind(null, 10));",
        &NamedValues::new(),
        PrintWriter::Disabled,
    );
    let JsProgress::Suspended(suspension) = progress else {
        panic!("expected bound callback suspension");
    };
    let progress =
        restore(&suspension).resume_call(ExtFunctionResult::Return(MontyObject::int(12)), PrintWriter::Disabled);
    let JsProgress::Complete { mut repl, value } = progress else {
        panic!("expected completion");
    };
    assert_eq!(value.py_repr(), "[12]");
    assert_no_leaks(&mut repl);
}

#[test]
fn array_map_abort_runs_finally_and_releases_partial_output() {
    let repl = JsRepl::new("map.js", ResourceLimits::default()).unwrap();
    let mut output = String::new();
    let progress = repl.feed(
        "[1, 2].map(x => { try { return host(x); } finally { console.log('cleanup'); } });",
        &NamedValues::new(),
        PrintWriter::collect_string(&mut output),
    );
    let JsProgress::Suspended(suspension) = progress else {
        panic!("expected callback suspension");
    };
    let error = MontyException::new(ExcType::RuntimeError, Some("cancelled".into()));
    let progress = restore(&suspension).abort(&error, PrintWriter::collect_string(&mut output));
    let JsProgress::Failed { mut repl, .. } = progress else {
        panic!("expected cancellation error");
    };
    assert_eq!(output, "cleanup\n");
    assert_no_leaks(&mut repl);
}

#[test]
fn array_map_async_callbacks_keep_pending_futures_across_dump() {
    let repl = JsRepl::new("map.js", ResourceLimits::default()).unwrap();
    let mut progress = repl.feed("let result; Promise.all([1, 2].map(async x => { try { return await host(x); } finally { console.log(x); } })).then(values => result = values);", &NamedValues::new(), PrintWriter::Disabled);
    let mut calls = Vec::new();
    for _ in 0..2 {
        let JsProgress::Suspended(suspension) = progress else {
            panic!("expected host call");
        };
        let mut suspension = restore(&suspension);
        let JsSuspensionKind::FunctionCall { call_id, .. } = suspension.kind() else {
            panic!("expected call");
        };
        calls.push(call_id);
        progress = suspension.resume_call(ExtFunctionResult::Future(call_id), PrintWriter::Disabled);
    }
    let JsProgress::Suspended(suspension) = progress else {
        panic!("expected pending futures");
    };
    let progress = restore(&suspension).resume_futures(
        calls
            .into_iter()
            .enumerate()
            .map(|(i, id)| {
                (
                    id,
                    ExtFunctionResult::Return(MontyObject::int(i64::try_from(i + 10).unwrap())),
                )
            })
            .collect(),
        PrintWriter::Disabled,
    );
    let JsProgress::Complete { repl, .. } = progress else {
        panic!("expected completion");
    };
    let JsProgress::Complete { mut repl, value } = repl.feed("result", &NamedValues::new(), PrintWriter::Disabled)
    else {
        panic!("expected results");
    };
    assert_eq!(value.py_repr(), "[10, 11]");
    assert_no_leaks(&mut repl);
}

#[test]
fn array_map_frame_limit_releases_callback_arguments() {
    let repl = JsRepl::new("map.js", ResourceLimits::default().max_recursion_depth(2)).unwrap();
    let progress = repl.feed("[{value: 1}].map(x => x);", &NamedValues::new(), PrintWriter::Disabled);
    let JsProgress::Failed { mut repl, error } = progress else {
        panic!("expected frame limit");
    };
    assert!(error.summary().contains("Maximum call stack"), "{}", error.summary());
    assert_no_leaks(&mut repl);
}
