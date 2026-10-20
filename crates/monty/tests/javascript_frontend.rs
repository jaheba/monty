#![cfg(feature = "javascript")]

use std::time::Duration;

use monty::{Dump, MontyRepl, ReplProgress, Session, SessionRef, dump};
use monty_types::{
    CompileOptions, ExtFunctionResult, MontyObject, NamedValues, PrintWriter, ResourceLimits, ResourceTracker,
};

fn session() -> MontyRepl {
    MontyRepl::new("mixed.py", ResourceTracker::default(), CompileOptions::default())
}

fn python(repl: &mut MontyRepl, code: &str) -> MontyObject {
    repl.feed_run(code, NamedValues::new(), PrintWriter::Disabled).unwrap()
}

fn javascript(repl: &mut MontyRepl, code: &str) -> MontyObject {
    repl.feed_javascript(code, NamedValues::new(), PrintWriter::Disabled)
        .unwrap()
}

#[test]
fn python_calls_a_js_closure_which_calls_python() {
    let mut repl = session();
    python(&mut repl, "def py_add(x, y):\n    return x + y");
    javascript(
        &mut repl,
        "function makeAdder(x) { return (y) => py_add(x, y); } const add = makeAdder(10);",
    );
    assert_eq!(python(&mut repl, "add(32)"), MontyObject::int(42));
    assert_eq!(javascript(&mut repl, "add(5)"), MontyObject::int(15));
}

#[test]
fn heap_objects_are_shared_without_conversion() {
    let mut repl = session();
    python(&mut repl, "values = []\nalias = values");
    javascript(&mut repl, "values.push(42);");
    assert_eq!(python(&mut repl, "values is alias"), MontyObject::bool(true));
    assert_eq!(python(&mut repl, "alias[0]"), MontyObject::int(42));
    python(&mut repl, "alias.append(7)");
    assert_eq!(javascript(&mut repl, "values[1]"), MontyObject::int(7));
}

#[test]
fn unicode_and_truth_follow_the_shared_value_model() {
    let mut repl = session();
    assert_eq!(javascript(&mut repl, "'😀'.length"), MontyObject::int(1));
    assert_eq!(javascript(&mut repl, "([] || 42).length"), MontyObject::int(0));
    assert_eq!(javascript(&mut repl, "-7 % 3"), MontyObject::int(2));
}

#[test]
fn unsupported_source_is_rejected_before_running_anything() {
    let mut repl = session();
    python(&mut repl, "value = 1");
    assert!(
        repl.feed_javascript("value = 2; new Date();", NamedValues::new(), PrintWriter::Disabled)
            .is_err()
    );
    assert_eq!(python(&mut repl, "value"), MontyObject::int(1));
    assert!(
        repl.feed_javascript("'\\uD800'", NamedValues::new(), PrintWriter::Disabled)
            .is_err()
    );
    assert_eq!(javascript(&mut repl, "undefined"), MontyObject::none());
}

#[test]
fn control_flow_and_string_escaping() {
    let mut repl = session();
    assert_eq!(
        javascript(
            &mut repl,
            "let n = 0; while (n < 5) { n += 1; } if (n === 5) { n += 37; } n"
        ),
        MontyObject::int(42)
    );
    assert_eq!(
        javascript(&mut repl, r#""hello\n'world\\""#),
        MontyObject::string("hello\n'world\\")
    );
}

#[test]
fn mixed_closures_use_the_existing_snapshot_format() {
    let mut repl = session();
    python(&mut repl, "def double(x):\n    return x * 2");
    javascript(
        &mut repl,
        "function capture(x) { return (y) => double(x + y); } const f = capture(10);",
    );

    let bytes = dump("mixed.py", None, &[], SessionRef::Idle(&repl)).unwrap();
    let Session::Idle(mut restored) = Dump::load(&bytes).unwrap().state else {
        panic!("idle session")
    };
    assert_eq!(python(&mut restored, "f(11)"), MontyObject::int(42));
    assert_eq!(javascript(&mut restored, "f(1)"), MontyObject::int(22));
}

#[test]
fn exceptions_unwind_through_both_languages() {
    let mut repl = session();
    python(&mut repl, "def fail():\n    return 1 / 0");
    javascript(&mut repl, "function bridge() { return fail(); }");
    let error = repl
        .feed_run("bridge()", NamedValues::new(), PrintWriter::Disabled)
        .unwrap_err();
    let traceback = error.to_string();
    assert!(traceback.contains("in bridge"), "{traceback}");
    assert!(traceback.contains("in fail"), "{traceback}");
    assert!(traceback.contains("ZeroDivisionError"), "{traceback}");
}

#[test]
fn js_names_do_not_pass_through_a_python_parser() {
    let mut repl = session();
    assert_eq!(
        javascript(&mut repl, "let $value = 40; let lambda = 2; $value + lambda"),
        MontyObject::int(42)
    );
    assert_eq!(
        javascript(&mut repl, "function $add(from) { return from + $value; } $add(2)"),
        MontyObject::int(42)
    );
}

#[test]
fn strict_equality_is_a_shared_vm_operation() {
    let mut repl = session();
    python(
        &mut repl,
        "items = []\nalias = items\nnumber = 42.0\ntext = ''.join(['a', 'b'])\nnan = float('nan')",
    );
    for code in [
        "items === alias",
        "42 === number",
        "'ab' === text",
        "null === null",
        "true === true",
    ] {
        assert_eq!(javascript(&mut repl, code), MontyObject::bool(true), "{code}");
    }
    for code in [
        "[] === []",
        "true === 1",
        "false === null",
        "'42' === 42",
        "nan === nan",
    ] {
        assert_eq!(javascript(&mut repl, code), MontyObject::bool(false), "{code}");
    }
    javascript(&mut repl, "function same(a, b) { return a === b; }");
    assert_eq!(python(&mut repl, "same(True, 1)"), MontyObject::bool(false));
    assert_eq!(python(&mut repl, "same(items, alias)"), MontyObject::bool(true));
    assert_eq!(python(&mut repl, "True == 1"), MontyObject::bool(true));

    let bytes = dump("mixed.py", None, &[], SessionRef::Idle(&repl)).unwrap();
    let Session::Idle(mut restored) = Dump::load(&bytes).unwrap().state else {
        panic!("idle session")
    };
    assert_eq!(python(&mut restored, "same([], [])"), MontyObject::bool(false));
    assert_eq!(javascript(&mut restored, "same(1, true)"), MontyObject::bool(false));
}

#[test]
fn traceback_contains_original_javascript_source() {
    let mut repl = session();
    javascript(&mut repl, "function broken(x) {\n    return x / 0;\n}");
    let error = repl
        .feed_run("broken(1)", NamedValues::new(), PrintWriter::Disabled)
        .unwrap_err();
    let traceback = error.to_string();
    assert!(traceback.contains("return x / 0;"), "{traceback}");
    assert!(traceback.contains("line 2"), "{traceback}");
}

#[test]
fn break_and_continue_use_shared_loop_control() {
    let mut repl = session();
    assert_eq!(
        javascript(
            &mut repl,
            "let n = 0; let total = 0; while (true) { n += 1; if (n < 3) { continue; } if (n > 5) { break; } total += n; } total"
        ),
        MontyObject::int(12)
    );
}

#[test]
fn javascript_uses_the_existing_execution_budget() {
    let mut repl = session();
    *repl.tracker_mut() = ResourceTracker::new(ResourceLimits::default().max_feed_duration(Duration::from_millis(10)));
    let error = repl
        .feed_javascript("while (true) {}", NamedValues::new(), PrintWriter::Disabled)
        .unwrap_err();
    assert!(error.to_string().contains("TimeoutError"), "{error}");
    assert_eq!(python(&mut repl, "40 + 2"), MontyObject::int(42));
}

#[test]
fn closures_write_through_intermediate_scopes() {
    let mut repl = session();
    javascript(
        &mut repl,
        "function outer() { let n = 1; function middle() { return () => { n++; return n; }; } return middle(); } const next = outer();",
    );
    assert_eq!(javascript(&mut repl, "next()"), MontyObject::int(2));
    assert_eq!(python(&mut repl, "next()"), MontyObject::int(3));
    let bytes = dump("mixed.py", None, &[], SessionRef::Idle(&repl)).unwrap();
    let Session::Idle(mut restored) = Dump::load(&bytes).unwrap().state else {
        panic!("idle")
    };
    assert_eq!(javascript(&mut restored, "next()"), MontyObject::int(4));
}

#[test]
fn optional_access_and_nullish_coalescing_short_circuit() {
    let mut repl = session();
    javascript(
        &mut repl,
        "let hits = 0; function side() { hits++; return 'x'; } const nil = null;",
    );
    assert_eq!(javascript(&mut repl, "nil?.[side()]"), MontyObject::none());
    assert_eq!(javascript(&mut repl, "nil?.push(side())"), MontyObject::none());
    assert_eq!(javascript(&mut repl, "0 ?? side()"), MontyObject::int(0));
    assert_eq!(javascript(&mut repl, "false && side()"), MontyObject::bool(false));
    assert_eq!(javascript(&mut repl, "hits"), MontyObject::int(0));
}

#[test]
fn augmented_property_assignment_evaluates_receiver_and_key_once() {
    let mut repl = session();
    javascript(
        &mut repl,
        "const obj = {n: 1}; const calls = []; function receiver() { calls.push('object'); return obj; } function key() { calls.push('key'); return 'n'; } function rhs() { calls.push('rhs'); return 2; }",
    );
    javascript(&mut repl, "receiver()[key()] += rhs();");
    assert_eq!(javascript(&mut repl, "obj.n"), MontyObject::int(3));
    assert_eq!(
        javascript(&mut repl, "calls.join(',')"),
        MontyObject::string("object,key,rhs")
    );
}

#[test]
fn source_guards_allow_batches_and_reject_recursive_forms() {
    let mut repl = session();
    let source = format!("let n = 0; {} n", "n += 1;".repeat(256));
    assert_eq!(javascript(&mut repl, &source), MontyObject::int(256));
    for source in [
        format!("{}0{}", "(".repeat(256), ")".repeat(256)),
        format!("{};", "for (;;)".repeat(256)),
        format!("{};", "if (true) ; else ".repeat(256)),
    ] {
        assert!(
            repl.feed_javascript(&source, NamedValues::new(), PrintWriter::Disabled)
                .is_err()
        );
    }
    assert_eq!(javascript(&mut repl, "n"), MontyObject::int(256));
}

#[test]
fn rejected_initial_feed_does_not_install_helpers_or_publish_names() {
    let mut repl = session();
    assert!(
        repl.feed_javascript(
            "let introduced = 42; new Date();",
            NamedValues::new(),
            PrintWriter::Disabled
        )
        .is_err()
    );
    assert!(
        repl.feed_run("_js_method", NamedValues::new(), PrintWriter::Disabled)
            .is_err()
    );
    assert!(
        repl.feed_run("introduced", NamedValues::new(), PrintWriter::Disabled)
            .is_err()
    );
    assert!(
        repl.feed_javascript("let _js_method = 42;", NamedValues::new(), PrintWriter::Disabled)
            .is_err()
    );
    assert_eq!(
        javascript(&mut repl, "[1, 2].map(x => x * 2).join(',')"),
        MontyObject::string("2,4")
    );
}

#[test]
fn library_callbacks_share_the_execution_budget() {
    let mut repl = session();
    javascript(&mut repl, "let ready = true;");
    *repl.tracker_mut() = ResourceTracker::new(ResourceLimits::default().max_feed_duration(Duration::from_millis(10)));
    let error = repl
        .feed_javascript(
            "[1].map(x => { while (true) {} });",
            NamedValues::new(),
            PrintWriter::Disabled,
        )
        .unwrap_err();
    assert!(error.to_string().contains("TimeoutError"), "{error}");
    assert_eq!(python(&mut repl, "40 + 2"), MontyObject::int(42));
}

#[test]
fn callback_host_suspensions_restore_shared_language_frames() {
    let mut repl = session();
    python(&mut repl, "def bridge(x):\n    return remote(x)");
    let mut progress = repl
        .feed_javascript_start(
            "JSON.stringify([1, 2].map(x => bridge(x)))",
            NamedValues::new(),
            PrintWriter::Disabled,
        )
        .unwrap();
    for (input, output) in [(1, 10), (2, 20)] {
        let bytes = dump("mixed.py", None, &[], SessionRef::Suspended(&progress)).unwrap();
        drop(progress.into_repl());
        let Session::Suspended(restored) = Dump::load(&bytes).unwrap().state else {
            panic!("suspended")
        };
        let ReplProgress::FunctionCall(call) = *restored else {
            panic!("function call")
        };
        assert_eq!(call.function_name, "remote");
        assert_eq!(call.args.args().collect::<Vec<_>>(), vec![MontyObject::int(input)]);
        progress = call.resume(MontyObject::int(output), PrintWriter::Disabled).unwrap();
    }
    let (mut repl, result) = progress.into_complete().expect("complete");
    assert_eq!(result, MontyObject::string("[10,20]"));
    assert_eq!(javascript(&mut repl, "[3].map(x => x + 1)[0]"), MontyObject::int(4));
}

#[test]
fn async_js_callbacks_use_shared_futures_and_snapshots() {
    let repl = session();
    let mut progress = repl
        .feed_javascript_start(
            "await Promise.all([1, 2].map(async x => await remote(x)))",
            NamedValues::new(),
            PrintWriter::Disabled,
        )
        .unwrap();
    let mut ids = Vec::new();
    for expected in [1, 2] {
        let ReplProgress::FunctionCall(call) = progress else {
            panic!("host call: {progress:?}")
        };
        assert_eq!(call.args.args().collect::<Vec<_>>(), vec![MontyObject::int(expected)]);
        ids.push(call.call_id);
        progress = call.resume_pending(PrintWriter::Disabled).unwrap();
    }
    let bytes = dump("mixed.py", None, &[], SessionRef::Suspended(&progress)).unwrap();
    drop(progress.into_repl());
    let Session::Suspended(restored) = Dump::load(&bytes).unwrap().state else {
        panic!("suspended")
    };
    let ReplProgress::ResolveFutures(pending) = *restored else {
        panic!("futures")
    };
    let progress = pending
        .resume(
            ids.into_iter()
                .map(|id| (id, ExtFunctionResult::Return(MontyObject::int(21))))
                .collect(),
            PrintWriter::Disabled,
        )
        .unwrap();
    let (mut repl, result) = progress.into_complete().expect("complete");
    assert_eq!(result, MontyObject::list([MontyObject::int(21), MontyObject::int(21)]));
    assert_eq!(python(&mut repl, "40 + 2"), MontyObject::int(42));
}

#[test]
fn js_property_writes_coerce_keys_and_resize_shared_arrays() {
    let mut repl = session();
    javascript(
        &mut repl,
        "const obj = {}; obj[7] = 'ok'; const data = []; data[2] = 42;",
    );
    assert_eq!(
        javascript(&mut repl, "JSON.stringify(obj)"),
        MontyObject::string("{\"7\":\"ok\"}")
    );
    assert_eq!(javascript(&mut repl, "data['2']"), MontyObject::int(42));
    assert_eq!(python(&mut repl, "len(data)"), MontyObject::int(3));
    javascript(&mut repl, "data.length = 1;");
    assert_eq!(python(&mut repl, "data"), MontyObject::list([MontyObject::none()]));
    javascript(&mut repl, "data.length = 3;");
    assert_eq!(
        javascript(&mut repl, "JSON.stringify(data)"),
        MontyObject::string("[null,null,null]")
    );
    assert_eq!(
        javascript(&mut repl, "JSON.stringify(Object.entries([1, 2]))"),
        MontyObject::string("[[\"0\",1],[\"1\",2]]")
    );
}

#[test]
fn library_collections_restore_identity_keys() {
    let mut repl = session();
    javascript(
        &mut repl,
        "const key = {}; const table = new Map([[key, 42]]); const members = new Set([key]);",
    );
    let bytes = dump("mixed.py", None, &[], SessionRef::Idle(&repl)).unwrap();
    let Session::Idle(mut repl) = Dump::load(&bytes).unwrap().state else {
        panic!("idle")
    };
    assert_eq!(javascript(&mut repl, "table.get(key)"), MontyObject::int(42));
    assert_eq!(javascript(&mut repl, "members.has(key)"), MontyObject::bool(true));
    assert_eq!(javascript(&mut repl, "table.has({})"), MontyObject::bool(false));
    assert_eq!(
        javascript(&mut repl, "JSON.stringify([[1], [2, 3]].flatMap(x => x))"),
        MontyObject::string("[1,2,3]")
    );
    assert_eq!(javascript(&mut repl, "[1, 2, 3].at(-1)"), MontyObject::int(3));
    assert_eq!(
        javascript(&mut repl, "console.error('diagnostic'); console.warn('warning'); 42"),
        MontyObject::int(42)
    );
}
