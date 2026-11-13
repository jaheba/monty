#![cfg(feature = "experimental-threaded")]

use std::{fmt::Write, thread};

use monty::MontyRun;
use monty_types::CompileOptions;

fn check(source: &str) {
    let mut runner = MontyRun::new(source.to_owned(), "dispatch.py", vec![], CompileOptions::default()).unwrap();
    runner.run_no_limits(vec![]).unwrap();
    #[cfg(feature = "ref-count-return")]
    {
        let result = runner.run_ref_counts(vec![]).unwrap();
        assert!(result.unreachable.is_empty(), "{:?}", result.unreachable);
    }
    #[cfg(not(feature = "ref-count-return"))]
    runner.run_no_limits(vec![]).unwrap();
}

#[test]
fn local_runs_preserve_frame_changes_errors_and_reference_ownership() {
    check(
        r"
def copy(value):
    alias = value
    a = alias
    b = a
    a = b
    return a

def recurse(n, value):
    if n:
        return recurse(n - 1, copy(value))
    return copy(value)

for i in range(400):
    value = [i]
    assert recurse(4, value) is value

def unbound():
    a = 1
    b = a
    try:
        b = missing
    except UnboundLocalError:
        return b
    missing = 2
assert unbound() == 1

try:
    recurse(1, [1])[5]
except IndexError:
    pass
else:
    assert False

def large():
    value = 1234567890123456789012345678901234567890
    alias = value
    return alias
assert large() == 1234567890123456789012345678901234567890

",
    );
}

#[test]
fn wide_locals_and_checkpoint_long_runs_preserve_values() {
    let mut source = "def wide():\n".to_owned();
    for slot in 0..300 {
        writeln!(source, "    a{slot} = [{slot}]").unwrap();
    }
    for _ in 0..3 {
        for slot in 0..300 {
            writeln!(source, "    a{slot} = a{slot}").unwrap();
        }
    }
    source.push_str("    return a299\nassert wide() == [299]\n");
    check(&source);
}

#[cfg(feature = "test-hooks")]
#[test]
fn native_runs_keep_deadline_checkpoints() {
    use std::time::{Duration, Instant};

    use monty_types::{ExcType, PrintWriter, ResourceLimits, ResourceTracker};

    let mut runner = MontyRun::opcode_cost_benchmark(10_000_000);
    let tracker = ResourceTracker::new(ResourceLimits::default().max_feed_duration(Duration::from_millis(1)));
    let start = Instant::now();
    let error = runner.run(vec![], tracker, PrintWriter::Disabled).unwrap_err();
    assert_eq!(error.exc_type(), ExcType::TimeoutError);
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn threaded_subscription_keeps_callbacks_mutations_and_caught_errors() {
    check(
        r"
def read(d):
    value = d['x']
    return value

for i in range(300):
    a = {'x': [i], 'y': [2]}
    b = {'y': [3], 'x': [i + 1]}
    assert read(a) is a['x']
    assert read(b) is b['x']
    a.pop('x')
    a['x'] = [i + 2]
    assert read(a) == [i + 2]
    a.clear()
    try:
        read(a)
    except KeyError:
        pass
    else:
        assert False

calls = []
class Key:
    def __hash__(self):
        return hash('x')
    def __eq__(self, other):
        calls.append(other)
        return other == 'x'
k = Key()
d = {k: [5]}
assert read(d) == [5]
assert len(calls) > 0

class Record:
    pass
try:
    read(Record())
except TypeError:
    pass
else:
    assert False

assert [1, 2][1] == 2
assert (3, 4)[1] == 4
assert 'abc'[1] == 'b'
from collections import defaultdict, Counter
missing = defaultdict(list)
assert missing['new'] == []
assert missing['new'] is missing['new']
assert Counter()['missing'] == 0

try:
    read({})
except KeyError:
    assert read({'x': [9]}) == [9]
else:
    assert False
",
    );
}

#[test]
fn threaded_subscription_preserves_traceback_and_task_errors() {
    use monty_types::ExcType;

    let source = "def read(d):\n    value = d['missing']\n    return value\nread({})";
    let mut runner = MontyRun::new(source.to_owned(), "subscript.py", vec![], CompileOptions::default()).unwrap();
    let error = runner.run_no_limits(vec![]).unwrap_err();
    assert_eq!(error.exc_type(), ExcType::KeyError);
    let frame = error.traceback().last().unwrap();
    assert_eq!(frame.frame_name.as_deref(), Some("read"));
    assert_eq!(frame.start.line, 2);

    check(
        r"
import asyncio
async def caught(i):
    try:
        value = {}['missing']
    except KeyError:
        return i
assert await asyncio.gather(caught(1), caught(2)) == [1, 2]

async def failed():
    return {}['missing']
async def sibling():
    return 7
try:
    await asyncio.gather(failed(), sibling())
except KeyError:
    assert {'x': [3]}['x'] == [3]
else:
    assert False
",
    );
}

#[test]
fn threaded_subscription_observes_deadlines_and_recovers() {
    use std::time::Duration;

    use monty::MontyRepl;
    use monty_types::{ExcType, MontyObject, PrintWriter, ResourceLimits, ResourceTracker};

    let tracker = ResourceTracker::new(ResourceLimits::default().max_feed_duration(Duration::from_millis(20)));
    let mut repl = MontyRepl::new("deadline.py", tracker, CompileOptions::default());
    let error = repl
        .feed_run(
            "def hot(d):\n    while True:\n        value = d['x']\nhot({'x': [1]})",
            vec![],
            PrintWriter::Disabled,
        )
        .unwrap_err();
    assert_eq!(error.exc_type(), ExcType::TimeoutError);
    assert_eq!(
        repl.feed_run("1 + 1", vec![], PrintWriter::Disabled).unwrap(),
        MontyObject::int(2)
    );
}

#[cfg(feature = "test-hooks")]
#[test]
fn native_batches_end_with_the_same_stack_and_counter_state() {
    use monty_types::MontyObject;

    for count in [254, 255, 256, 257, 511, 512, 513, 100_000] {
        let mut runner = MontyRun::opcode_cost_benchmark(count);
        assert_eq!(runner.run_no_limits(vec![]).unwrap(), MontyObject::int(0));
    }
}

#[test]
fn callback_reentry_with_long_dispatch_batches_fits_small_stack() {
    thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let mut source = "class Recursive:\n    def __eq__(self, other):\n        a = 0\n".to_owned();
            for _ in 0..120 {
                source.push_str("        a = a\n");
            }
            source.push_str("        return self == other\ntry:\n    Recursive() == Recursive()\nexcept RecursionError:\n    pass\nelse:\n    assert False\n");
            check(&source);
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn direct_branches_preserve_short_circuit_values_and_heap_truth() {
    check(
        r"
class Marker:
    pass
marker = [9]
for _ in range(300):
    for value in [None, False, True, 0, 1, -1, 0.0, 1.0, [], [1], '', 'x', Marker(), 123456789012345678901234567890]:
        truth = bool(value)
        if truth:
            assert (value or marker) is value
            assert (value and marker) is marker
        else:
            assert (value or marker) is marker
            assert (value and marker) is value
",
    );
}
