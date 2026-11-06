use std::time::Duration;

use monty::MontyRun;
use monty_types::{CompileOptions, MontyObject, PrintWriter, ResourceLimits, ResourceTracker};

fn run_assertions(code: &str) {
    let mut run = MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap();
    // Empty-sequence repetition must validate large counts rather than loop over them.
    // Bound execution so a regression in that path fails the test instead of hanging the suite.
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

#[test]
fn date_descriptors_preserve_datetime_subtype_rules() {
    run_assertions(
        r"
from datetime import date, datetime, timedelta, timezone
for value in [datetime(2024, 1, 2, 12, 34), datetime(2024, 1, 2, 12, 34, tzinfo=timezone.utc)]:
    assert date.__repr__(value) == 'datetime.datetime(2024, 1, 2)'
    assert date.__str__(value) == value.isoformat()
    assert date.__format__(value, '') == str(value)
    assert date.__format__(value, '%H:%M') == '12:34'
    assert date.__add__(value, timedelta(days=1)) is NotImplemented
    assert date.__radd__(value, timedelta(days=1)) is NotImplemented
    assert date.__sub__(value, date(2024, 1, 1)) is NotImplemented
    assert date.__rsub__(value, date(2024, 1, 1)) is NotImplemented
    assert date.__eq__(value, date(2024, 1, 2)) is True
    assert date.__eq__(value, datetime(2024, 1, 2)) is NotImplemented
    assert date.__lt__(value, datetime(2024, 1, 3)) is NotImplemented
assert date.__repr__(date(2024, 1, 2)) == 'datetime.date(2024, 1, 2)'
assert date.__add__(date(2024, 1, 2), timedelta(days=1)) == date(2024, 1, 3)
for call in [lambda: date.__repr__(value, 1), lambda: date.__add__(value), lambda: date.__str__(value, 1), lambda: date.__format__(value)]:
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
fn slice_deletion_compacts_without_losing_owned_references() {
    run_assertions(
        r"
for selection, expected in [(slice(1, 7), [0, 7, 8, 9]), (slice(None, None, 2), [1, 3, 5, 7, 9]), (slice(None, None, -2), [0, 2, 4, 6, 8]), (slice(8, 1, -3), [0, 1, 3, 4, 6, 7, 9]), (slice(None, None, -(2**63)), list(range(9)))]:
    values = [[i] for i in range(10)]
    alias = values
    assert values.__delitem__(selection) is None
    assert alias is values
    assert values == [[i] for i in expected]
    assert values.__delitem__(slice(None)) is None
    assert alias == []
values = []
values.extend([values, [1], values, [2]])
values.__delitem__(slice(None, None, 2))
assert values == [[1], [2]]
values.append(values)
values.__delitem__(slice(0, 2))
assert values[0] is values
values.__delitem__(slice(None))
assert values == []
True
",
    );
}

#[test]
fn large_stepped_slice_deletion_finishes_within_feed_limit() {
    // Repeated Vec::remove made this quadratic; the shared two-second feed limit
    // leaves ample time for linear construction and compaction of these values.
    run_assertions(
        r"
values = list(range(500000))
values.__delitem__(slice(None, None, 2))
assert len(values) == 250000
assert values[0] == 1
assert values[-1] == 499999
True
",
    );
}
