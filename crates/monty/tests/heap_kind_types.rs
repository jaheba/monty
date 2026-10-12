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
