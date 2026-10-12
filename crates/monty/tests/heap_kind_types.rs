use monty::MontyRun;
use monty_types::CompileOptions;

#[test]
fn tagged_types_preserve_payload_dependent_types() {
    let code = r#"
from collections import Counter, defaultdict, namedtuple
from functools import partial
from hashlib import sha256
from datetime import date, datetime

class Example:
    pass

Point = namedtuple('Point', ['x'])
values = [([], list), ({}, dict), (set(), set), (frozenset(), frozenset),
          (Counter(), Counter), (defaultdict(int), defaultdict),
          (Point(1), Point), (Example(), Example), (date(2026, 1, 1), date),
          (datetime(2026, 1, 1), datetime)]
for value, expected in values:
    assert type(value) is expected
assert str(type(ValueError())) == "<class 'ValueError'>"
assert str(type(TypeError())) == "<class 'TypeError'>"
assert str(type(sha256())) == "<class '_hashlib.HASH'>"
assert type(partial(Example)()) is Example
assert partial(lambda: 42)() == 42
try:
    partial([])
except TypeError:
    pass
else:
    assert False
"#;
    let mut run = MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap();
    run.run_no_limits(vec![]).unwrap();
}

#[test]
fn tag_fast_paths_preserve_iteration_formatting_and_conversions() {
    let code = r"
from base64 import a85encode
from binascii import hexlify
from datetime import date

assert next(iter([10, 20])) == 10
assert next(iter([]), 42) == 42
assert format([1, 2], '') == '[1, 2]'
assert format(date(2026, 1, 2), '%Y-%m-%d') == '2026-01-02'
assert (10, 20, 30)[1:] == (20, 30)
class Index:
    def __index__(self):
        return 1
assert (10, 20, 30)[Index()] == 20
assert a85encode(b'hello', wrapcol=10**30) == a85encode(b'hello')
assert hexlify(b'ab', '-'.encode()) == b'61-62'
class Example:
    pass
assert str(Example | None).endswith('Example | None')
assert str(list[int] | str) == 'list[int] | str'
try:
    hexlify(b'ab', [1])
except TypeError:
    pass
else:
    assert False
";
    let mut run = MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap();
    run.run_no_limits(vec![]).unwrap();
}
