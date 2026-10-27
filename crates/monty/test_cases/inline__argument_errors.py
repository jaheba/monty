import random
import re


def raises(error_type, call):
    try:
        call()
    except error_type:
        return
    assert False, 'expected an argument error'


inline = 'abc!'[:-1]
heap = 'abcdefghijklmnop!'[:-1]
for invalid in [inline, heap, [], {}]:
    raises(TypeError, lambda: b'abc'.startswith((b'zz', invalid)))
    raises(TypeError, lambda: b'abc'.endswith((invalid, b'c')))
    raises(TypeError, lambda: b'abc'.startswith(invalid))
    raises(TypeError, lambda: b'abc'.find(invalid))
    raises(TypeError, lambda: b'abc'.count(invalid))

raises(TypeError, lambda: b'abc'.startswith(42))

for separator in ['', inline, heap, 'é!'[:-1], b'', b'::', b'\xff']:
    raises(ValueError, lambda: b'ab'.hex(separator))
raises(TypeError, lambda: b'ab'.hex(42))
raises(TypeError, lambda: bytes.fromhex(42))
assert b'abc'.startswith((b'zz', b'ab'))
assert b'abc'.endswith((b'zz', b'bc'))
assert b'ab'.hex(b':') == '61:62'
assert bytes.fromhex(('61 ' * 8)[:-1]) == b'a' * 8

pattern = re.compile('a')
for invalid in [None, 42, [], {}]:
    for operation in [
        lambda: pattern.search(invalid),
        lambda: pattern.match(invalid),
        lambda: pattern.fullmatch(invalid),
        lambda: pattern.findall(invalid),
        lambda: pattern.finditer(invalid),
    ]:
        raises(TypeError, operation)
    raises(TypeError, lambda: pattern.sub(inline, invalid))
    raises(TypeError, lambda: pattern.sub(inline, invalid, -1))
    raises(TypeError, lambda: pattern.split(invalid))
raises(TypeError, lambda: pattern.sub(42, inline))

rng = random.Random(0)
state = rng.getstate()
raises(ValueError, lambda: rng.setstate((inline,) + state[1:]))
assert rng.getstate() == state

for path in [None, [], {}]:
    raises(TypeError, lambda: open(path))
raises(TypeError, lambda: open(inline, 42))
raises(ValueError, lambda: open(inline, 'rr!'[:-1]))
raises(ValueError, lambda: format(inline, '+!'[:-1]))

for operation in [
    lambda: inline.find(42),
    lambda: inline.replace(inline, 42),
    lambda: inline.split(42),
    lambda: inline.removeprefix(42),
]:
    raises(TypeError, operation)


class BadString:
    def __str__(self):
        return 42

    def __repr__(self):
        return 42


for operation in [
    lambda: f'{BadString()}',
    lambda: f'{BadString()!s}',
    lambda: f'{BadString()!r}',
    lambda: f'{BadString()!a}',
]:
    raises(TypeError, operation)


import csv
import unicodedata

match = re.search('(a)', 'a')
for invalid in [None, 1.5, b'a', []]:
    raises(IndexError, lambda: match[invalid])
for invalid in [None, 42, []]:
    raises(TypeError, lambda: unicodedata.lookup(invalid))


class Sink:
    def write(self, text):
        return len(text)


for invalid in [None, 42, [], {}]:
    raises(AttributeError, lambda: csv.DictWriter(Sink(), ['column'], extrasaction=invalid))
