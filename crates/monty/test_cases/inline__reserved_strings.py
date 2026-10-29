import os
from dataclasses import dataclass

empty = ''
assert str() is empty
assert f'{empty}' is empty
assert 'abc'[:0] is empty
assert id(str()) == id(empty)

for literal in ['\0', '\x7f', 'a', '/', ':', '.', '\n']:
    character = chr(ord(literal))
    assert character is literal
    assert ('!' + literal)[1:] is literal
    assert next(iter(literal)) is literal
    assert id(character) == id(literal)
    assert hash(character) == hash(literal)
    assert {character: 42}[literal] == 42

separator = '/'
assert os.sep is separator
assert os.path.sep is separator

for code in range(128):
    text = chr(code)
    assert str(text) is text
    assert ''.join([text]) is text
    assert {text: code}[('!' + text)[1:]] == code
    assert text.encode().decode() is text


@dataclass
class Point:
    x: int
    y: int


assert Point(1, 2).x == 1
assert list(Point.__dataclass_fields__) == ['x', 'y']
name = 'x'
assert Point.__dataclass_fields__['x'].name is name
setattr(Point, 'z', 3)
assert getattr(Point, chr(122)) == 3

Dynamic = type('Dynamic', (), {'__module__': __name__, '__annotations__': {'x': int}})
Dynamic = dataclass(Dynamic)
assert Dynamic(42).x == 42
