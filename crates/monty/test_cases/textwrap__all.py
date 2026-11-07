from textwrap import dedent, indent

# === Shared literal indentation and whitespace-only lines ===
for text, expected in [
    ('', ''),
    ('   ', ''),
    (' \t\n\t \n', '\n\n'),
    ('  one\n  two\n', 'one\ntwo\n'),
    ('    one\n  two\n', '  one\ntwo\n'),
    ('\tone\n\t\ttwo', 'one\n\ttwo'),
    ('\tone\n two', '\tone\n two'),
    ('  one\n\n  two', 'one\n\ntwo'),
    ('  one\r\n  two\r\n', 'one\r\ntwo\r\n'),
    ('  one\v  two', 'one\v  two'),
    ('  é\n  文\n  😀', 'é\n文\n😀'),
    ('\u2003\n  one\n\x1c', '\none\n'),
    ('one\n  two', 'one\n  two'),
]:
    assert dedent(text) == expected
    assert dedent(text=text) == expected

# Indentation preserves each splitlines() separator, including CRLF and Unicode separators.
assert indent('', '> ') == ''
assert indent('one\ntwo\n', '> ') == '> one\n> two\n'
assert indent('one\n \t\ntwo', '> ') == '> one\n \t\n> two'
assert indent('one\r\ntwo\r', '> ') == '> one\r\n> two\r'
assert indent('one\v文\u2028😀', '> ') == '> one\v> 文\u2028> 😀'
assert indent('one\ntwo', '') == 'one\ntwo'
assert indent(text='one', prefix='> ', predicate=None) == '> one'
assert indent('\n \n', None) == '\n \n'
assert indent('', '> ', 123) == ''

seen = []


def choose(line):
    seen.append(line)
    return line.startswith('yes')


assert indent('yes\nno\nyes\n', '> ', choose) == '> yes\nno\n> yes\n'
assert seen == ['yes\n', 'no\n', 'yes\n']
assert indent('\n \n', '> ', lambda line: True) == '> \n>  \n'
assert indent('one\ntwo', None, lambda line: False) == 'one\ntwo'


class Source:
    def split(self, separator):
        assert separator == '\n'
        return ['  one', '    two']

    def splitlines(self, keepends):
        assert keepends is True
        return ['one\n', 'two']


assert dedent(Source()) == 'one\n  two'
assert indent(Source(), '> ') == '> one\n> two'

# === Type and signature errors ===
for value, name in [(None, 'NoneType'), (123, 'int'), (b'one', 'bytes'), ([], 'list')]:
    try:
        dedent(value)
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == f"expected str object, not '{name}'"

try:
    indent('one', None)
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'sequence item 0: expected str instance, NoneType found'
try:
    indent('one', '> ', 123)
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == "'int' object is not callable"
try:
    dedent()
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == "dedent() missing 1 required positional argument: 'text'"
try:
    indent('one')
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == "indent() missing 1 required positional argument: 'prefix'"
try:
    dedent('one', 'two')
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'dedent() takes 1 positional argument but 2 were given'
try:
    indent('one', '> ', None, None)
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'indent() takes from 2 to 3 positional arguments but 4 were given'
