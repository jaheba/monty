use std::time::Duration;

use monty::MontyRun;
use monty_types::{CompileOptions, MontyObject, PrintWriter, ResourceLimits, ResourceTracker};

fn run_assertions(code: &str) {
    let mut run = MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap();
    let limits = ResourceLimits::default().max_feed_duration(Duration::from_secs(2));
    assert_eq!(
        run.run(vec![], ResourceTracker::new(limits), PrintWriter::Stdout)
            .unwrap(),
        MontyObject::bool(true)
    );
}

#[test]
fn path_dunder_bytes_returns_encoded_path() {
    run_assertions(
        r"
from pathlib import Path
value = Path('/tmp/café').__bytes__()
assert type(value) is bytes
assert value == b'/tmp/caf\xc3\xa9'
True
",
    );
}

#[test]
fn sequence_concatenation_descriptors_reject_incompatible_operands() {
    run_assertions(
        r"
from collections import deque
for call in [lambda: [1].__add__((2,)), lambda: (1,).__add__([2]), lambda: 'a'.__add__(1), lambda: b'a'.__add__('b'), lambda: deque([1]).__add__([2])]:
    try:
        call()
    except TypeError:
        pass
    else:
        raise AssertionError('expected TypeError')
True
",
    );
}

#[test]
fn sequence_multiplication_descriptors_reject_non_index_operands() {
    run_assertions(
        r"
from collections import deque
for call in [lambda: [1].__mul__(2.0), lambda: [1].__rmul__(2.0), lambda: (1,).__mul__(2.0), lambda: 'a'.__mul__(2.0), lambda: b'a'.__mul__(2.0), lambda: deque([1]).__mul__(2.0)]:
    try:
        call()
    except TypeError:
        pass
    else:
        raise AssertionError('expected TypeError')
True
",
    );
}

#[test]
fn list_inplace_repetition_accepts_index_protocol() {
    run_assertions(
        r"
class Index:
    def __index__(self):
        return 2
value = [1]
assert value.__imul__(Index()) is value
assert value == [1, 1]
True
",
    );
}

#[test]
fn deque_inplace_repetition_accepts_index_protocol() {
    run_assertions(
        r"
from collections import deque
class Index:
    def __index__(self):
        return 2
value = deque([1])
assert value.__imul__(Index()) is value
assert list(value) == [1, 1]
True
",
    );
}

#[test]
fn sequence_multiplication_descriptors_accept_index_protocol() {
    run_assertions(
        r"
from collections import deque
class Index:
    def __index__(self):
        return 2
for value in [[1], (1,), 'a', b'a', deque([1])]:
    assert value.__mul__(Index()) == value * 2
    assert value.__rmul__(Index()) == value * 2
for value in [[], (), '', b'', deque()]:
    assert value.__mul__(2**62) == value
    assert value.__rmul__(2**62) == value
True
",
    );
}

#[test]
fn inplace_repetition_observes_index_callback_mutation() {
    run_assertions(
        r"
value = [1]
class Index:
    def __index__(self):
        value.append(2)
        return 2
assert value.__imul__(Index()) is value
assert value == [1, 2, 1, 2]
True
",
    );
}

#[test]
fn empty_list_inplace_repetition_checks_ssize_overflow() {
    run_assertions(
        r"
value = []
try:
    value.__imul__(2**63)
except OverflowError:
    pass
else:
    raise AssertionError('expected OverflowError')
assert value == []
True
",
    );
}

#[test]
fn negative_list_inplace_repetition_checks_ssize_overflow() {
    run_assertions(
        r"
value = [1]
try:
    value.__imul__(-(2**100))
except OverflowError:
    pass
else:
    raise AssertionError('expected OverflowError')
assert value == [1]
True
",
    );
}

#[test]
fn empty_deque_inplace_repetition_checks_ssize_overflow() {
    run_assertions(
        r"
from collections import deque
value = deque()
try:
    value.__imul__(2**63)
except OverflowError:
    pass
else:
    raise AssertionError('expected OverflowError')
assert list(value) == []
True
",
    );
}

#[test]
fn base_dict_repr_observes_mutations_to_native_subtype() {
    run_assertions(
        r#"
from collections import defaultdict
value = defaultdict(int)
class AddKey:
    def __repr__(self):
        value['added'] = 2
        return 'x'
value['first'] = AddKey()
assert dict.__repr__(value) == "{'first': x, 'added': 2}"
assert value['added'] == 2
True
"#,
    );
}
