from collections import Counter, defaultdict, deque

for value in [[1, 2, 3], deque([1, 2, 3])]:
    alias = value
    assert value.__setitem__(-1, 4) is None
    assert list(value) == [1, 2, 4]
    assert type(value).__setitem__(value, True, 5) is None
    assert list(value) == [1, 5, 4]
    assert value.__delitem__(-1) is None
    assert list(value) == [1, 5]
    assert type(value).__delitem__(value, 0) is None
    assert list(value) == [5]
    assert value.__imul__(3) is alias
    assert list(value) == [5, 5, 5]
    assert value.__iadd__([6]) is alias
    assert list(value) == [5, 5, 5, 6]
    assert value.__imul__(0) is alias
    assert list(value) == []

for value in [{}, Counter(), defaultdict(int)]:
    assert value.__setitem__('a', [1]) is None
    assert value['a'] == [1]
    assert dict.__delitem__(value, 'a') is None
    assert 'a' not in value
    try:
        dict.__delitem__(value, 'a')
    except KeyError:
        pass
    else:
        raise AssertionError('missing key deletion succeeded')

value = [0, 1, 2, 3, 4, 5]
assert value.__setitem__(slice(1, 3), [8, 9, 10]) is None
assert value == [0, 8, 9, 10, 3, 4, 5]
assert value.__setitem__(slice(None, None, -2), [20, 21, 22, 23]) is None
assert value == [23, 8, 22, 10, 21, 4, 20]
assert value.__delitem__(slice(None, None, -2)) is None
assert value == [8, 10, 4]
assert value.__delitem__(slice(2, 1)) is None
assert value == [8, 10, 4]
assert value.__delitem__(slice(1, None)) is None
assert value == [8]

value = []
value.append(value)
assert value.__repr__() == '[[...]]'
assert value.__str__() == '[[...]]'
assert value.__imul__(2) is value
assert value[0] is value and value[1] is value
assert value.__delitem__(slice(None)) is None
assert value == []

value = {1, 2}
alias = value
assert value.__ior__({3}) is alias
assert value == {1, 2, 3}
assert value.__iand__(frozenset({2, 3})) is alias
assert value == {2, 3}
assert value.__ixor__({3, 4}) is alias
assert value == {2, 4}
assert value.__isub__({4}) is alias
assert value == {2}
assert value.__ior__([5]) is NotImplemented
assert value == {2}

value = {'a': 1}
assert value.__ior__([('b', 2)]) is value
assert value == {'a': 1, 'b': 2}

for value in [
    [1, 2],
    (1, 2),
    'ab',
    b'ab',
    range(2),
    {1: 2},
    {1, 2},
    frozenset({1, 2}),
    deque([1, 2]),
    {}.keys(),
    {}.items(),
    {}.values(),
]:
    iterator = value.__iter__()
    assert iterator.__iter__() is iterator
    expected = list(value)
    for item in expected:
        assert iterator.__next__() == item
    for _ in range(2):
        try:
            iterator.__next__()
        except StopIteration:
            pass
        else:
            raise AssertionError('exhausted iterator returned a value')

for value in [[1, 2], range(3), {1: 2, 3: 4}, deque([1, 2])]:
    iterator = value.__reversed__()
    assert iterator.__iter__() is iterator
    assert list(iterator) == list(reversed(value))

assert list.__len__([1, 2]) == 2
assert int.__repr__(True) == '1'
assert int.__and__(True, True) == 1
assert type(int.__and__(True, True)) is int
assert int.__add__(1, 2.0) is NotImplemented
assert int.__eq__(1, 1.0) is NotImplemented
assert float.__add__(1.0, 10**30) == 1.0 + 10**30
assert (1).__add__(10**30) == 1 + 10**30
assert (1).__mul__(10**30) == 10**30
assert int.__hash__(7) == hash(7)
assert 'ab'.__hash__() == hash('ab')
assert [1].__format__('') == str([1])
assert (3.5).__format__('.1f') == '3.5'
assert (3.5).__round__(1) == 3.5
assert (2).__pow__(5, 7) == 4
assert (2).__rpow__(5) == 25

for call in [
    lambda: [].__setitem__(0),
    lambda: [].__delitem__(),
    lambda: [].__iter__(1),
    lambda: (1).__add__(),
    lambda: list.__setitem__('abc', 0, 1),
]:
    try:
        call()
    except TypeError:
        pass
    else:
        raise AssertionError('invalid call succeeded')

# Base descriptors retain their defining type's semantics on native subtypes.
from collections import namedtuple

p = namedtuple('Point', ['x'])(1)
assert tuple.__repr__(p) == '(1,)'
assert tuple.__str__(p) == 'Point(x=1)'
c = Counter(a=2, b=0)
assert dict.__repr__(c) == "{'a': 2, 'b': 0}"
assert not dict.__eq__(c, Counter(a=2))
assert dict.__lt__(c, Counter(a=3)) is NotImplemented
assert dict.__or__(c, Counter(a=3)) == {'a': 3, 'b': 0}
assert type(dict.__or__(c, Counter(a=3))) is dict
assert dict.__ior__(c, {'a': -1}) is c
assert c['a'] == -1
assert c['b'] == 0
assert c.__delitem__('missing') is None

d = deque([1, 2])
i = iter(d)
assert d.__imul__(1) is d
assert next(i) == 1
assert i.__eq__(i) is True
assert i.__ne__(i) is False


left = defaultdict(list, a=1)
right = defaultdict(set, b=2)
merged = left.__ror__(right)
assert merged.default_factory is set
assert merged == {'b': 2, 'a': 1}
assert type(dict.__or__(left, right)) is dict
assert type(dict.__ror__(left, right)) is dict


for start in [None, -8, -1, 0, 3, 8]:
    for stop in [None, -8, -1, 0, 3, 8]:
        for step in [-3, -1, 1, 3, -(2**63)]:
            value = list(range(5))
            selection = slice(start, stop, step)
            removed = list(range(5))[selection]
            expected = [item for item in value if item not in removed]
            assert value.__delitem__(selection) is None
            assert value == expected

for call in [lambda: [1].__delitem__(slice(None, None, 0)), lambda: [1].__setitem__(slice(None, None, 0), [])]:
    try:
        call()
    except ValueError:
        pass
    else:
        raise AssertionError('zero slice step accepted')

value = [1]


class MutatingIndex:
    def __index__(self):
        value.clear()
        return 0


try:
    value.__delitem__(MutatingIndex())
except IndexError:
    pass
else:
    raise AssertionError('deletion ignored an index callback mutation')
assert value == []


True
