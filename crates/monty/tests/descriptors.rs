use monty::MontyRun;
#[cfg(not(feature = "memory-model-checks"))]
use monty::{Dump, MontyRepl, ReplProgress, Session, SessionRef, dump};
use monty_types::{CompileOptions, ExcType};
#[cfg(not(feature = "memory-model-checks"))]
use monty_types::{ExtFunctionResult, MontyObject, PrintWriter, ResourceTracker};

fn run(source: &str) {
    let mut run = MontyRun::new(source.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap();
    run.run_no_limits(vec![]).unwrap();
}

#[test]
fn builtin_descriptor_decorators() {
    run(include_str!("../test_cases/class__builtin_descriptors.py"));
}

#[test]
fn descriptor_lookup_and_assignment() {
    run(include_str!("../test_cases/class__descriptors.py"));
}

#[test]
fn descriptor_calls_resolve_before_arguments() {
    run(include_str!("../test_cases/class__descriptor_calls.py"));
}

#[test]
fn mutation_nested_hooks_and_dataclass_fields() {
    run(include_str!("../test_cases/class__descriptor_mutation.py"));
}

#[test]
fn recursive_getters_raise_instead_of_overflowing() {
    let mut run = MontyRun::new(
        "class D:\n def __get__(self, obj, owner): return obj.field\nclass C:\n field = D()\nC().field".to_owned(),
        "test.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    let error = run.run_no_limits(vec![]).unwrap_err();
    assert_eq!(error.exc_type(), ExcType::RecursionError);
}

#[test]
#[cfg(not(feature = "memory-model-checks"))]
fn prepared_descriptor_callable_survives_suspended_argument_snapshot() {
    let mut repl = MontyRepl::new("test.py", ResourceTracker::default(), CompileOptions::default());
    repl.feed_run(
        "events = []\ndef target(value): return value + 1\nclass D:\n def __get__(self, obj, owner):\n  events.append('get')\n  return target\nclass C:\n method = D()\nx = C()",
        vec![], PrintWriter::Disabled,
    ).unwrap();
    let progress = repl
        .feed_start("x.method(host_call())", vec![], PrintWriter::Disabled)
        .unwrap();
    let bytes = dump("test.py", None, SessionRef::Suspended(&progress)).unwrap();
    let Session::Suspended(progress) = Dump::load(&bytes).unwrap().state else {
        panic!("expected suspended argument call");
    };
    let ReplProgress::FunctionCall(call) = *progress else {
        panic!("expected function call")
    };
    let ReplProgress::Complete {
        repl: mut restored,
        value: output,
    } = call
        .resume(ExtFunctionResult::Return(MontyObject::int(4)), PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("expected completion")
    };
    assert_eq!(output, MontyObject::int(5));
    restored
        .feed_run("assert events == ['get']", vec![], PrintWriter::Disabled)
        .unwrap();
}

#[test]
fn external_calls_in_getters_raise_a_catchable_error() {
    run(
        "class D:\n def __get__(self, obj, owner):\n  try: host_call()\n  except NotImplementedError: return 7\nclass C:\n field = D()\nassert C().field == 7",
    );
}

#[test]
#[cfg(feature = "ref-count-return")]
fn failed_hooks_and_argument_unpacking_release_references() {
    let source = r"
def exercise():
    class D:
        def __get__(self, obj, owner):
            return obj.target
        def __set__(self, obj, value):
            raise ValueError('setter')
    class C:
        field = D()
        def target(self, value): return value
    x = C()
    x.cycle = x
    try: x.field = [x]
    except ValueError: pass
    try: x.field(*[x], **{1: x})
    except TypeError: pass
    try: x.field(*[x], **{'value': x}, **{'value': x})
    except TypeError: pass
    class Raising:
        def __get__(self, obj, owner): raise ValueError('getter')
    class Broken:
        __init__ = Raising()
    try: Broken([x])
    except ValueError: pass
    C.field = Raising()
    try: x.field
    except ValueError: pass
for _ in range(10): exercise()
";
    let output = MontyRun::new(source.to_owned(), "test.py", vec![], CompileOptions::default())
        .unwrap()
        .run_ref_counts(vec![])
        .unwrap();
    assert!(output.unreachable.is_empty(), "{:?}", output.unreachable);
}

#[test]
fn nested_staticmethod_wrappers_respect_recursion_limits() {
    let mut run = MontyRun::new(
        "target = lambda: 1\nfor _ in range(2000): target = staticmethod(target)\ntarget()".to_owned(),
        "test.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    let error = run.run_no_limits(vec![]).unwrap_err();
    assert_eq!(error.exc_type(), ExcType::RecursionError);
}

#[test]
#[cfg(feature = "ref-count-return")]
fn builtin_descriptor_cycles_release_references() {
    let source = r"
def exercise():
    state = []
    def getter(obj): return state
    class C:
        field = property(getter, doc=state)
        @classmethod
        def owner(cls): return cls
        @staticmethod
        def items(): return state
    x = C()
    state.append(x)
    assert x.field is state
    assert C.items() is state
    assert x.owner() is C
    try: x.field = x
    except AttributeError: pass
for _ in range(10): exercise()
";
    let output = MontyRun::new(source.to_owned(), "test.py", vec![], CompileOptions::default())
        .unwrap()
        .run_ref_counts(vec![])
        .unwrap();
    assert!(output.unreachable.is_empty(), "{:?}", output.unreachable);
}

#[test]
#[cfg(not(feature = "memory-model-checks"))]
fn builtin_descriptors_survive_suspended_call_snapshot() {
    let mut repl = MontyRepl::new("test.py", ResourceTracker::default(), CompileOptions::default());
    repl.feed_run(
        r"
class C:
    def __init__(self, value): self._value = value
    @property
    def value(self): return self._value
    @value.setter
    def value(self, value): self._value = value
    @classmethod
    def create(cls, value): return cls(value)
    @staticmethod
    def double(value): return value * 2
saved = C.create
",
        vec![],
        PrintWriter::Disabled,
    )
    .unwrap();
    let progress = repl
        .feed_start("saved(host_call()).value", vec![], PrintWriter::Disabled)
        .unwrap();
    let bytes = dump("test.py", None, SessionRef::Suspended(&progress)).unwrap();
    let Session::Suspended(progress) = Dump::load(&bytes).unwrap().state else {
        panic!("expected suspended call");
    };
    let ReplProgress::FunctionCall(call) = *progress else {
        panic!("expected function call");
    };
    let ReplProgress::Complete {
        repl: mut restored,
        value: output,
    } = call
        .resume(ExtFunctionResult::Return(MontyObject::int(9)), PrintWriter::Disabled)
        .unwrap()
    else {
        panic!("expected completion");
    };
    assert_eq!(output, MontyObject::int(9));
    restored
        .feed_run(
            "x = saved(3)\nx.value = C.double(x.value)\nassert x.value == 6",
            vec![],
            PrintWriter::Disabled,
        )
        .unwrap();
}

#[test]
fn nested_classmethod_bindings_respect_recursion_limits() {
    for source in [
        "class C:\n method = classmethod(lambda *args: None)\nfor _ in range(2000): C.method = classmethod(C.method)\nC.method()",
        "class C:\n __init__ = classmethod(lambda *args: None)\nfor _ in range(2000): C.__init__ = classmethod(C.__init__)\nC()",
    ] {
        let mut run = MontyRun::new(source.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap();
        let error = run.run_no_limits(vec![]).unwrap_err();
        assert_eq!(error.exc_type(), ExcType::RecursionError);
    }
}

#[test]
#[cfg(not(feature = "memory-model-checks"))]
fn prepared_native_callables_survive_suspended_argument_snapshots() {
    for (source, expected) in [
        ("xs.append(host_call())", MontyObject::none()),
        ("'{value}'.format(value=host_call())", MontyObject::string("9")),
        ("xs.append(*(host_call(),))", MontyObject::none()),
        ("'{value}'.format(**{'value': host_call()})", MontyObject::string("9")),
        (
            "'{a}-{b}'.format(**{'a': host_call()}, **{'b': 1})",
            MontyObject::string("9-1"),
        ),
    ] {
        let mut repl = MontyRepl::new("test.py", ResourceTracker::default(), CompileOptions::default());
        repl.feed_run("xs = []", vec![], PrintWriter::Disabled).unwrap();
        let progress = repl.feed_start(source, vec![], PrintWriter::Disabled).unwrap();
        let bytes = dump("test.py", None, SessionRef::Suspended(&progress)).unwrap();
        let Session::Suspended(progress) = Dump::load(&bytes).unwrap().state else {
            panic!("expected suspended argument");
        };
        let ReplProgress::FunctionCall(call) = *progress else {
            panic!("expected function call");
        };
        let ReplProgress::Complete {
            repl: mut restored,
            value,
        } = call
            .resume(ExtFunctionResult::Return(MontyObject::int(9)), PrintWriter::Disabled)
            .unwrap()
        else {
            panic!("expected completion");
        };
        assert_eq!(value, expected, "{source}");
        let assertion = if source.starts_with("xs.") {
            "assert xs == [9]"
        } else {
            "assert xs == []"
        };
        restored.feed_run(assertion, vec![], PrintWriter::Disabled).unwrap();
    }
}

#[test]
fn native_bound_calls_preserve_arguments_and_cleanup() {
    run(r"
for _ in range(30):
    xs = []
    xs.append([1])
    xs.extend([2, 3])
    assert xs.pop() == 3
    assert xs == [[1], 2]
    assert '{a}-{b}'.format(a=1, b=2) == '1-2'
    assert b'ab'.replace(b'a', b'c') == b'cb'
    assert dict.fromkeys(['a', 'b'], 4) == {'a': 4, 'b': 4}
    try:
        xs.append([1], [2])
    except TypeError:
        pass
    else:
        assert False
    try:
        xs.append(**{'unexpected': [1]})
    except TypeError:
        pass
    else:
        assert False
");
}
