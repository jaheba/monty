import string
from string import ascii_letters, capwords

assert string.ascii_lowercase == 'abcdefghijklmnopqrstuvwxyz'
assert string.ascii_uppercase == 'ABCDEFGHIJKLMNOPQRSTUVWXYZ'
assert ascii_letters == string.ascii_lowercase + string.ascii_uppercase
assert string.digits == '0123456789'
assert string.hexdigits == '0123456789abcdefABCDEF'
assert string.octdigits == '01234567'
assert string.punctuation == '!"#$%&\'()*+,-./:;<=>?@[\\]^_`{|}~'
assert string.whitespace == ' \t\n\r\x0b\x0c'
assert string.printable == string.digits + string.ascii_letters + string.punctuation + string.whitespace
assert len(string.printable) == 100

for text, expected in [
    ('', ''),
    ('  hello\tWORLD\n', 'Hello World'),
    ('hello world', 'Hello World'),
    ('élÈVE déjà VU', 'Élève Déjà Vu'),
    ('ßETA ǳUNGLE', 'Sseta ǲungle'),
    ('one\u2003two', 'One Two'),
]:
    assert capwords(text) == expected
    assert capwords(s=text, sep=None) == expected

assert capwords('hello-world', '-') == 'Hello-World'
assert capwords('a::b::::C', '::') == 'A::B::::C'
assert capwords('  hello  WORLD ', ' ') == '  Hello  World '


class Source:
    def split(self, sep):
        assert sep is None
        return ['one', 'TWO']


assert capwords(Source()) == 'One Two'

try:
    capwords('abc', '')
    assert False, 'expected ValueError'
except ValueError as exc:
    assert str(exc) == 'empty separator'
try:
    capwords()
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == "capwords() missing 1 required positional argument: 's'"
try:
    capwords('a', 'b', 'c')
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'capwords() takes from 1 to 2 positional arguments but 3 were given'
try:
    capwords('a', unexpected=1)
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == "capwords() got an unexpected keyword argument 'unexpected'"
