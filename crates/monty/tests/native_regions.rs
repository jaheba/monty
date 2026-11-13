#![cfg(all(
    feature = "experimental-native",
    not(feature = "experimental-fusion"),
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use std::time::Duration;

use monty::{FunctionCall, MontyRun, RunProgress, experimental_native};
use monty_types::{CompileOptions, MontyObject, PrintWriter, ResourceLimits, ResourceTracker};

fn run(source: &str, enabled: bool) -> Result<MontyObject, String> {
    experimental_native::set_enabled(enabled);
    MontyRun::new(source.to_owned(), "native.py", vec![], CompileOptions::default())
        .unwrap()
        .run_no_limits(vec![])
        .map_err(|error| error.to_string())
}

fn compare(source: &str) {
    let expected = run(source, false);
    experimental_native::reset_stats();
    assert_eq!(run(source, true), expected, "{source}");
    assert!(
        experimental_native::stats().instructions > 0,
        "no native execution: {source}"
    );
}

#[test]
fn varied_control_flow_and_arithmetic() {
    for update in ["i + 3", "i * 7", "i // 3", "i % -3", "i ^ 12", "-i", "i & 7", "i | 8"] {
        compare(&format!(
            "def calculate(n):\n    i = -20\n    total = 0\n    while i < n:\n        if i % 2 == 0:\n            total += {update}\n        else:\n            total -= {update}\n        i += 1\n    return total\ncalculate(200)"
        ));
    }
    compare(
        "def helper(x):\n    return x * 3 + 1\ni = 0\ntotal = 0\nwhile i < 100:\n    total += helper(i)\n    i += 1\ntotal",
    );
    compare("total = 0\nfor i in range(100):\n    total += i * 2\ntotal");
}

#[test]
fn guards_preserve_results_and_exception_locations() {
    for source in [
        "x = 9223372036854775807\nx += 1\nx * 2",
        "x = -9223372036854775807 - 1\n-x",
        "x = -9223372036854775807 - 1\nx // -1",
        "x = 12\nx // 0",
        "x = 12\nx % 0",
        "x = 1\nx += 0.5\nx * 2",
        "x = True\nx + 3",
        "x = 'abc'\nx + 'def'",
        "def f():\n    x = 1\n    return missing + x\nf()",
        "x = 1\ntry:\n    x += 2\n    y = x // 0\nexcept ZeroDivisionError:\n    x += 10\nx",
        "a = []\nb = a\ni = 0\nwhile i < 10:\n    a, b = b, a\n    i += 1\n(a is b, a)",
    ] {
        compare(source);
    }
}

#[test]
fn dictionary_reads_and_protocol_fallbacks() {
    compare("row = {'price': 7}\ni = 0\ntotal = 0\nwhile i < 200:\n    total += row['price'] * 3\n    i += 1\ntotal");
    assert!(experimental_native::stats().dictionary_reads >= 200);
    for source in [
        "d = {'price': 7}\nk = ''.join(['pr', 'ice'])\nx = 1\nx + d[k]",
        "d = {'price': []}\nx = 1\nd['price']",
        "d = {}\nx = 1\nd['missing']",
        "from collections import defaultdict\nd = defaultdict(int)\nx = 1\nd['missing'] + x",
        "from collections import Counter\nd = Counter()\nx = 1\nd['missing'] + x",
        "events = []\nclass Key:\n    def __hash__(self):\n        return hash('price')\n    def __eq__(self, other):\n        events.append(other)\n        return True\nd = {Key(): 7}\nx = 1\nresult = d['price'] + x\n(result, events)",
    ] {
        compare(source);
    }
}

#[test]
fn suspension_dump_and_resume_rebuild_native_artifacts() {
    experimental_native::set_enabled(true);
    experimental_native::reset_stats();
    let runner = MontyRun::new(
        "def work():\n    x = 7\n    y = external(x * 3)\n    return y * 2 + x\nwork()".to_owned(),
        "suspend.py",
        vec!["external".to_owned()],
        CompileOptions::default(),
    )
    .unwrap();
    let RunProgress::FunctionCall(call) = runner
        .start(
            vec![MontyObject::function("external".to_owned(), None)],
            ResourceTracker::default(),
            PrintWriter::Disabled,
        )
        .unwrap()
    else {
        panic!("expected suspension")
    };
    assert_eq!(call.args.arg(0).unwrap(), MontyObject::int(21));
    assert!(experimental_native::stats().instructions > 0);
    let bytes = serde_json::to_vec(&call).unwrap();
    let RunProgress::Complete(original) = call.resume(MontyObject::int(10), PrintWriter::Disabled).unwrap() else {
        panic!("expected original completion")
    };
    assert_eq!(original, MontyObject::int(27));
    experimental_native::reset_stats();
    let restored: FunctionCall = serde_json::from_slice(&bytes).unwrap();
    let RunProgress::Complete(result) = restored.resume(MontyObject::int(10), PrintWriter::Disabled).unwrap() else {
        panic!("expected completion")
    };
    assert_eq!(result, MontyObject::int(27));
    assert!(experimental_native::stats().compilations > 0);
    assert!(experimental_native::stats().instructions > 0);
}

#[test]
fn generators_can_resume_between_native_regions() {
    compare("def values():\n    i = 0\n    while i < 30:\n        i += 1\n        yield i * 2\nlist(values())");
}

#[test]
fn range_cursors_preserve_aliases_and_partial_progress() {
    for source in [
        "s = 0\nfor i in range(200):\n    s += i * 2\n(s, i)",
        "s = 0\nfor i in range(20, -30, -3):\n    if i % 2:\n        s += i\n(s, i)",
        "it = iter(range(10))\nvalues = []\nfor i in it:\n    values.append((i, next(it, None)))\nvalues",
        "s = 0\nfor i in range(10):\n    s += 0.5\n(s, i)",
        "s = 0\nfor i in range(10):\n    if i == 4:\n        break\n    s += i\n(s, i)",
        "s = 0\nfor i in range(0):\n    s += i\ns",
    ] {
        compare(source);
        assert!(experimental_native::stats().range_steps > 0);
    }
}

#[test]
fn advanced_range_cursor_survives_suspension_and_serialization() {
    experimental_native::set_enabled(true);
    experimental_native::reset_stats();
    let runner = MontyRun::new(
        "s = 0\nfor i in range(6):\n    s += i\n    if i == 2:\n        s += external(s)\n(s, i)".to_owned(),
        "range_suspend.py",
        vec!["external".to_owned()],
        CompileOptions::default(),
    )
    .unwrap();
    let RunProgress::FunctionCall(call) = runner
        .start(
            vec![MontyObject::function("external".to_owned(), None)],
            ResourceTracker::default(),
            PrintWriter::Disabled,
        )
        .unwrap()
    else {
        panic!("expected suspension")
    };
    assert_eq!(call.args.arg(0).unwrap(), MontyObject::int(3));
    assert!(experimental_native::stats().range_steps >= 3);
    let bytes = serde_json::to_vec(&call).unwrap();
    let RunProgress::Complete(expected) = call.resume(MontyObject::int(10), PrintWriter::Disabled).unwrap() else {
        panic!("expected original completion")
    };
    experimental_native::reset_stats();
    let restored: FunctionCall = serde_json::from_slice(&bytes).unwrap();
    let RunProgress::Complete(result) = restored.resume(MontyObject::int(10), PrintWriter::Disabled).unwrap() else {
        panic!("expected restored completion")
    };
    assert_eq!(result, expected);
    assert!(experimental_native::stats().range_steps >= 3);
}

#[test]
fn native_loops_keep_deadline_checkpoints() {
    experimental_native::set_enabled(true);
    experimental_native::reset_stats();
    let mut runner = MontyRun::new(
        "x = 0\nwhile True:\n    x += 1".to_owned(),
        "timeout.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    let error = runner
        .run(
            vec![],
            ResourceTracker::new(ResourceLimits::default().max_turn_duration(Duration::from_millis(10))),
            PrintWriter::Disabled,
        )
        .unwrap_err();
    assert!(error.to_string().contains("time limit"), "{error}");
    assert!(experimental_native::stats().instructions > 0);
}

#[test]
fn configured_memory_limits_use_the_interpreter() {
    experimental_native::set_enabled(true);
    experimental_native::reset_stats();
    let mut runner = MontyRun::new(
        "x = 1\nx + 2".to_owned(),
        "memory.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    assert_eq!(
        runner
            .run(
                vec![],
                ResourceTracker::new(ResourceLimits::default().max_memory(1024 * 1024)),
                PrintWriter::Disabled,
            )
            .unwrap(),
        MontyObject::int(3)
    );
    assert_eq!(experimental_native::stats().instructions, 0);
}
