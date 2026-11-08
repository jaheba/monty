# === Integer, replacement-string and deletion mappings ===
assert 'abracadabra'.translate({ord('a'): 'A', ord('b'): None, ord('c'): ord('C')}) == 'ArACAdArA'
assert 'é文😀'.translate({ord('é'): 'long', ord('文'): ord('語'), ord('😀'): None}) == 'long語'
assert 'ab'.translate({ord('a'): ''}) == 'b'
assert 'ab'.translate({ord('a'): True}) == '\x01b'
assert 'abc'.translate({}) == 'abc'
assert ''.translate(None) == ''
assert 'abc'.translate('short') == 'abc'
assert '\x00\x01'.translate('ab') == 'ab'
assert '\x00\x01'.translate(b'AB') == 'AB'
assert ('a' * 40).translate({ord('a'): '文'}) == '文' * 40


class Mapping:
    def __getitem__(self, key):
        if key == ord('文'):
            return '語'
        if key == ord('é'):
            raise IndexError('missing')
        raise LookupError('missing')


assert '文é😀'.translate(Mapping()) == '語é😀'
table = Mapping()
table.__getitem__ = lambda key: 'ignored'
assert '文'.translate(table) == '語'


class RaisingMapping:
    def __getitem__(self, key):
        raise ValueError('lookup failed')


try:
    '文'.translate(RaisingMapping())
    assert False, 'expected ValueError'
except ValueError as exc:
    assert str(exc) == 'lookup failed'

for value in [-1, 0x110000, 2**100]:
    try:
        'a'.translate({ord('a'): value})
        assert False, 'expected ValueError'
    except ValueError as exc:
        assert str(exc) == 'character mapping must be in range(0x110000)'
for value in [1.5, b'a', [], {}]:
    try:
        'a'.translate({ord('a'): value})
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == 'character mapping must return integer, None or str'

try:
    'abc'.translate()
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'str.translate() takes exactly one argument (0 given)'
try:
    'abc'.translate({}, {})
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'str.translate() takes exactly one argument (2 given)'
try:
    'abc'.translate(table={})
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'str.translate() takes no keyword arguments'

# === Both staticmethod entry points ===
assert str.maketrans('abc', 'XYZ') == {97: 88, 98: 89, 99: 90}
assert ''.maketrans('abc', 'XYZ', 'bc') == {97: 88, 98: None, 99: None}
assert ('x' * 40).maketrans('é文😀', 'ABC') == {233: 65, 25991: 66, 128512: 67}
assert str.maketrans('aa', 'XY') == {97: 89}
assert str.maketrans('', '', 'ab') == {97: None, 98: None}
assert str.maketrans({}) == {}
assert str.maketrans({'a': 'XYZ', '文': None, 0: 65, -1: 66, 2**100: []}) == {
    97: 'XYZ',
    25991: None,
    0: 65,
    -1: 66,
    2**100: [],
}
assert 'abcé'.translate(str.maketrans('aé', 'AÉ', 'b')) == 'AcÉ'
replacement = []
assert str.maketrans({'a': replacement})[97] is replacement

for mapping in [{'ab': 1}, {'': 1}]:
    try:
        str.maketrans(mapping)
        assert False, 'expected ValueError'
    except ValueError as exc:
        assert str(exc) == 'string keys in translatetable must be of length 1'
for mapping in [{None: 1}, {1.5: 1}, {b'a': 1}]:
    try:
        str.maketrans(mapping)
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == 'keys in translate table mustbe strings or integers'
for value in [None, 'abc', [], 123]:
    try:
        str.maketrans(value)
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == 'if you give only one argument to maketrans it must be a dict'
try:
    str.maketrans('abc', 'xy')
    assert False, 'expected ValueError'
except ValueError as exc:
    assert str(exc) == 'the first two maketrans arguments must have equal length'
try:
    str.maketrans([], '')
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'first maketrans argument must be a string if there is a second argument'
try:
    str.maketrans('a', None)
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'maketrans() argument 2 must be str, not None'
try:
    str.maketrans('a', 'b', None)
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'maketrans() argument 3 must be str, not None'
try:
    str.maketrans()
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'maketrans expected at least 1 argument, got 0'
try:
    str.maketrans('', '', '', '')
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'maketrans expected at most 3 arguments, got 4'
try:
    str.maketrans(x={})
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'str.maketrans() takes no keyword arguments'
