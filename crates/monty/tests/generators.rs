use std::time::Duration;

use monty::{Dump, MontyRepl, MontyRun, Session, SessionRef, dump};
use monty_types::{CompileOptions, ExcType, MontyObject, PrintWriter, ResourceLimits, ResourceTracker};

fn run(code: &str) {
    MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default())
        .unwrap()
        .run_no_limits(vec![])
        .unwrap();
}

#[test]
fn lazy_creation_and_independent_locals() {
    run(r"
events = []
def generate(start):
    events.append(start)
    received = yield start
    assert received is None
    yield start + 1

a = generate(1)
b = generate(10)
assert events == []
assert next(a) == 1
assert next(b) == 10
assert next(a) == 2
assert next(b) == 11
assert events == [1, 10]
assert next(a, 'end') == 'end'
assert next(a, None) is None
try:
    next(b)
except StopIteration:
    pass
else:
    assert False
");
}

#[test]
fn loops_nested_generators_and_normal_calls() {
    run(r"
def normal(x):
    return x + 1
def inner():
    yield normal(1)
    yield 3
def outer():
    g = inner()
    yield next(g)
    for item in g:
        yield normal(item)
result = []
for item in outer():
    result.append(item)
assert result == [2, 4]
");
}

#[test]
fn exceptions_and_reentrant_execution() {
    run(r"
def generate():
    yield 1
    next(g)
g = generate()
assert next(g) == 1
try:
    next(g)
except ValueError as exc:
    assert str(exc) == 'generator already executing'
else:
    assert False
assert next(g, 'closed') == 'closed'

def failure():
    yield 1
    raise ValueError('failed')
g = failure()
assert next(g) == 1
try:
    next(g)
except ValueError:
    pass
else:
    assert False
assert next(g, 'closed') == 'closed'
");
}

#[test]
fn unreachable_yield_still_creates_generator() {
    run(r"
events = []
def generate():
    events.append(1)
    return
    yield 1
g = generate()
assert events == []
assert next(g, 'done') == 'done'
assert events == [1]
def ordinary():
    def nested():
        yield 1
    return 42
assert ordinary() == 42
");
}

#[test]
fn return_value_preserves_identity_and_only_first_exhaustion() {
    run(r"
value = []
def generate():
    yield 1
    return value
g = generate()
assert next(g) == 1
try:
    next(g)
except StopIteration as exc:
    assert exc.value is value
    assert exc.args == (value,)
else:
    assert False
try:
    next(g)
except StopIteration as exc:
    assert exc.value is None
    assert exc.args == ()
");
}

#[test]
fn suspended_exception_context_is_private_and_retained() {
    run(r"
def generate():
    try:
        raise ValueError('original')
    except ValueError:
        yield 1
        raise
g = generate()
assert next(g) == 1
try:
    raise TypeError('caller')
except TypeError:
    try:
        next(g)
    except ValueError as exc:
        assert str(exc) == 'original'
    else:
        assert False
    try:
        raise
    except TypeError as exc:
        assert str(exc) == 'caller'
assert next(g, None) is None
");
}

#[test]
fn suspended_generators_round_trip_with_shared_objects() {
    let mut repl = MontyRepl::new("test.py", ResourceTracker::default(), CompileOptions::default());
    repl.feed_run(
        r"
shared = []
def generate(value):
    yield value
    value.append(42)
    yield value
a = generate(shared)
b = generate(shared)
assert next(a) is shared
assert next(b) is shared
",
        vec![],
        PrintWriter::Stdout,
    )
    .unwrap();
    let bytes = dump("test.py", None, SessionRef::Idle(&repl)).unwrap();
    let Session::Idle(mut restored) = Dump::load(&bytes).unwrap().state else {
        panic!("expected idle session")
    };
    restored.feed_run("assert next(a) is shared\nassert next(b) is shared\nassert shared == [42, 42]\nassert next(a, None) is None", vec![], PrintWriter::Stdout).unwrap();
}

#[test]
fn running_nested_generators_round_trip_during_external_call() {
    for consumer in [
        "result = next(outer())",
        "result = None\nfor value in outer():\n    result = value",
    ] {
        let repl = MontyRepl::new("test.py", ResourceTracker::default(), CompileOptions::default());
        let code = format!(
            "def inner():\n    yield host_call()\ndef outer():\n    yield next(inner())\n{consumer}\nassert result == 42"
        );
        let progress = repl.feed_start(&code, vec![], PrintWriter::Stdout).unwrap();
        let bytes = dump("test.py", None, SessionRef::Suspended(&progress)).unwrap();
        progress
            .into_function_call()
            .unwrap()
            .resume(MontyObject::int(42), PrintWriter::Stdout)
            .unwrap()
            .into_complete()
            .unwrap();
        let Session::Suspended(restored) = Dump::load(&bytes).unwrap().state else {
            panic!("expected suspended session")
        };
        let call = restored.into_function_call().unwrap();
        assert_eq!(call.function_name, "host_call");
        call.resume(MontyObject::int(42), PrintWriter::Stdout)
            .unwrap()
            .into_complete()
            .unwrap();
    }
}

#[test]
#[cfg(feature = "ref-count-return")]
fn unreachable_generators_and_cycles_release_references() {
    let code = r"
def make_cycle():
    value = []
    def generate(arg):
        yield arg
        yield value
    g = generate(value)
    value.append(g)
    next(g)
for _ in range(100):
    make_cycle()
def temporary():
    value = [[] for _ in range(10)]
    def generate(arg):
        yield arg
    generate(value)
    g = generate(value)
    next(g)
for _ in range(100):
    temporary()
def exception_cycle():
    value = []
    def generate():
        yield 1
        return value
    g = generate()
    next(g)
    try:
        next(g)
    except StopIteration as exc:
        value.append(exc)
for _ in range(100):
    exception_cycle()
";
    let output = MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default())
        .unwrap()
        .run_ref_counts(vec![])
        .unwrap();
    assert!(output.unreachable.is_empty(), "{:?}", output.unreachable);
    assert!(output.heap_count < 50, "{}", output.heap_count);
}

#[test]
#[cfg(feature = "test-hooks")]
fn gc_collects_created_and_suspended_generator_cycles() {
    run(r"
import gc
gc.disable()
def make_cycle(started):
    values = []
    def generate(arg):
        yield arg
        yield values
    g = generate(values)
    values.append(g)
    if started:
        next(g)
for started in [False, True]:
    gc.collect()
    make_cycle(started)
    assert gc.collect() >= 2
    assert gc.collect() == 0
gc.enable()
");
}

#[test]
#[cfg(feature = "test-hooks")]
fn gc_preserves_reachable_suspended_and_running_generators() {
    run(r"
import gc
def generate():
    values = []
    values.append(values)
    yield 1
    gc.collect()
    assert values[0] is values
    yield values
g = generate()
assert next(g) == 1
gc.collect()
value = next(g)
assert value[0] is value
assert next(g, 'done') == 'done'
gc.collect()
assert value[0] is value
value = None
assert gc.collect() >= 1
");
}

#[test]
fn argument_errors_are_raised_before_generator_execution() {
    run(r"
events = []
def generate(a, *, b):
    events.append(a)
    yield a + b
for call in [lambda: generate(), lambda: generate(1), lambda: generate(1, 2), lambda: generate(1, b=2, c=3)]:
    try:
        call()
    except TypeError:
        pass
    else:
        assert False
assert events == []
assert next(generate(1, b=2)) == 3
");
}

#[test]
fn interleaved_generators_preserve_operand_stacks_and_loop_state() {
    run(r"
def generate(offset):
    for i in range(3):
        result = 100 + (yield offset + i)
        yield result
a = generate(10)
b = generate(20)
assert next(a) == 10
assert next(b) == 20
for g in [a, b]:
    try:
        next(g)
    except TypeError:
        pass
    else:
        assert False
    assert next(g, 'done') == 'done'
def counted(offset):
    for i in range(3):
        yield offset + i
a = counted(10)
b = counted(20)
assert [(next(a), next(b)) for _ in range(3)] == [(10, 20), (11, 21), (12, 22)]
");
}

#[test]
fn arguments_closures_lambdas_and_breaks() {
    run(r"
def factory(captured):
    def generate(start=1, *, step=2):
        yield captured + start
        yield captured + start + step
    return generate
g = factory(10)(step=3)
assert type(g).__name__ == 'generator'
assert iter(g) is g
advance = next
for item in g:
    assert item == 11
    break
assert advance(g) == 14
assert advance(g, []) == []
f = lambda x: (yield x)
assert next(f(42)) == 42
def ordinary():
    f = lambda: (yield 1)
    return 42
assert ordinary() == 42
");
}

#[test]
fn return_none_has_empty_exception_arguments() {
    run(r"
def generate():
    return
    yield
try:
    next(generate())
except StopIteration as exc:
    assert exc.value is None
    assert exc.args == ()
else:
    assert False
");
}

#[test]
fn generator_resumption_enforces_recursion_limits() {
    for expression in [
        "yield next(generate(depth - 1))",
        "for value in generate(depth - 1):\n            yield value",
    ] {
        let code =
            format!("def generate(depth):\n    if depth:\n        {expression}\n    yield 42\nnext(generate(100))");
        let mut runner = MontyRun::new(code, "test.py", vec![], CompileOptions::default()).unwrap();
        let limits = ResourceLimits::default().max_recursion_depth(10);
        let error = runner
            .run(vec![], ResourceTracker::new(limits), PrintWriter::Stdout)
            .unwrap_err();
        assert_eq!(error.exc_type(), ExcType::RecursionError);
    }
}

#[test]
fn repeated_resumption_releases_recursion_levels() {
    let code =
        "def generate():\n    for value in range(1000):\n        yield value\nfor value in generate():\n    pass";
    let mut runner = MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap();
    runner
        .run(
            vec![],
            ResourceTracker::new(ResourceLimits::default().max_recursion_depth(3)),
            PrintWriter::Stdout,
        )
        .unwrap();
}

#[test]
fn instruction_time_checks_run_inside_generator_frames() {
    let code = "def generate():\n    yield 1\n    while True:\n        pass\ng = generate()\nnext(g)\nnext(g)";
    let mut runner = MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap();
    let limits = ResourceLimits::default().max_feed_duration(Duration::from_millis(10));
    let error = runner
        .run(vec![], ResourceTracker::new(limits), PrintWriter::Stdout)
        .unwrap_err();
    assert_eq!(error.exc_type(), ExcType::TimeoutError);
}

#[test]
fn explicit_stop_iteration_becomes_runtime_error() {
    run(r"
def generate():
    yield 1
    next(iter([]))
g = generate()
assert next(g) == 1
try:
    next(g)
except RuntimeError as exc:
    assert str(exc) == 'generator raised StopIteration'
else:
    assert False
assert next(g, None) is None
");
}

#[test]
fn synchronous_callbacks_can_call_next() {
    run(r"
def generate():
    yield 42
g = generate()
assert list(map(next, [g])) == [42]
try:
    map(next, [g])
except StopIteration:
    pass
else:
    assert False
assert list(map(lambda g: next(g, 'done'), [g])) == ['done']
");
}

#[test]
fn abandoning_running_generators_releases_frames_and_completes_them() {
    let repl = MontyRepl::new("test.py", ResourceTracker::default(), CompileOptions::default());
    let progress = repl
        .feed_start(
            "def generate():\n    yield host_call()\ng = generate()\nnext(g)",
            vec![],
            PrintWriter::Stdout,
        )
        .unwrap();
    let mut repl = progress.into_repl();
    repl.feed_run("assert next(g, 'done') == 'done'", vec![], PrintWriter::Stdout)
        .unwrap();
}

#[test]
fn yield_in_finally_preserves_pending_return() {
    run(r"
def generate():
    try:
        yield 1
        return [42]
    finally:
        yield 2
g = generate()
assert next(g) == 1
assert next(g) == 2
try:
    next(g)
except StopIteration as exc:
    assert exc.value == [42]
    assert str(exc) == '[42]'
    assert repr(exc) == 'StopIteration([42])'
else:
    assert False
");
}

#[test]
fn saved_namespace_and_exception_context_survive_snapshot() {
    let mut repl = MontyRepl::new("test.py", ResourceTracker::default(), CompileOptions::default());
    repl.feed_run(r#"
namespace = {'token': []}
exec('def generate():\n    try:\n        raise ValueError("original")\n    except ValueError:\n        yield token\n        raise', namespace)
g = namespace['generate']()
assert next(g) is namespace['token']
"#, vec![], PrintWriter::Stdout).unwrap();
    let bytes = dump("test.py", None, SessionRef::Idle(&repl)).unwrap();
    let Session::Idle(mut restored) = Dump::load(&bytes).unwrap().state else {
        panic!("expected idle session")
    };
    restored.feed_run("try:\n    next(g)\nexcept ValueError as exc:\n    assert str(exc) == 'original'\nelse:\n    assert False\nassert next(g, None) is None", vec![], PrintWriter::Stdout).unwrap();
}

#[test]
fn generator_methods_bind_instances() {
    run(r#"
class Source:
    def __init__(self, value):
        self.value = value
    def __iter__(self):
        yield self.value
        yield self.value + 1
assert [value for value in Source(41)] == [41, 42]
class Invalid:
    def __init__(self):
        yield 42
try:
    Invalid()
except TypeError as exc:
    assert str(exc) == "__init__() should return None, not 'generator'"
else:
    assert False
"#);
}

#[test]
fn send_none_uses_the_same_resumption_boundary() {
    run(r"
def generate():
    assert (yield 1) is None
    yield 2
g = generate()
assert g.send(None) == 1
try:
    g.send(42)
except NotImplementedError:
    pass
else:
    assert False
assert g.send(None) == 2
try:
    g.send(None)
except StopIteration:
    pass
else:
    assert False
");
}

#[test]
fn invalid_yield_scopes_and_async_generators_are_rejected() {
    for code in [
        "yield 1",
        "class C:\n    value = (yield 1)",
        "def f():\n    return [(yield x) for x in [1]]",
    ] {
        let error = MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap_err();
        assert_eq!(error.exc_type(), ExcType::SyntaxError);
    }
    let error = MontyRun::new(
        "async def f():\n    yield 1".to_owned(),
        "test.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.exc_type(), ExcType::NotImplementedError);
}
