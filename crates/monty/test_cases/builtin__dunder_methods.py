from collections import Counter, defaultdict, deque, namedtuple

for value in [
    [],
    [1, 2],
    (),
    (1, 2),
    '',
    'aé🦀',
    b'',
    b'abc',
    {},
    {1: 2},
    set(),
    {1, 2},
    frozenset({1}),
    deque([1, 2]),
    Counter(a=2),
    defaultdict(int, a=1),
]:
    assert value.__len__() == len(value)
    assert type(value).__len__(value) == len(value)

for value, item in [
    ([1, 2], 1),
    ((1, 2), 3),
    ('aé🦀', 'é'),
    (b'abc', 97),
    ({1: 2}, 1),
    ({1, 2}, 3),
    (frozenset({1}), 1),
    (deque([1]), 1),
    (Counter(a=2), 'a'),
    (defaultdict(int, a=1), 'b'),
]:
    assert value.__contains__(item) == (item in value)
    assert type(value).__contains__(value, item) == (item in value)

for value in [[1, 2, 3], (1, 2, 3), 'aé🦀', b'abc', deque([1, 2, 3])]:
    assert value.__getitem__(-1) == value[-1]
    assert type(value).__getitem__(value, 0) == value[0]

for value in [[1, 2, 3], (1, 2, 3), 'aé🦀', b'abc']:
    assert value.__getitem__(slice(None, None, -1)) == value[::-1]

for value in [{1: 'one'}, Counter(a=2), defaultdict(int)]:
    key = next(iter(value), 'missing')
    assert value.__getitem__(key) == value[key]
    assert type(value).__getitem__(value, key) == value[key]

mapping = {'a': 1, 'b': 2}
for view, item in [(mapping.keys(), 'a'), (mapping.items(), ('a', 1))]:
    assert view.__len__() == 2
    assert view.__contains__(item)
    assert not view.__contains__('missing')

calls = []


def factory():
    calls.append(1)
    return []


d = defaultdict(factory)
result = d.__getitem__('new')
assert result is d['new']
assert calls == [1]
assert dict.__getitem__(d, 'new') is result
assert dict.__getitem__(d, 'other') is d['other']
assert calls == [1, 1]

Point = namedtuple('Point', ['x', 'y'])
p = Point(1, 2)
assert p.__len__() == tuple.__len__(p) == 2
assert p.__contains__(2) == tuple.__contains__(p, 2)
assert p.__getitem__(0) == tuple.__getitem__(p, 0) == 1
assert dict.__len__(Counter(a=2)) == 1
assert dict.__getitem__(defaultdict(int, a=1), 'a') == 1

for call in [
    lambda: [].__len__(1),
    lambda: list.__len__(),
    lambda: list.__len__('abc'),
    lambda: [].__contains__(),
    lambda: [].__getitem__(),
    lambda: [].__getitem__(0, 1),
    lambda: [].__len__(extra=1),
    lambda: [].__contains__(item=1),
    lambda: [].__getitem__(key=0),
]:
    try:
        call()
    except TypeError:
        pass
    else:
        raise AssertionError('expected TypeError')

for value in [set(), frozenset(), {}.keys(), {}.items()]:
    try:
        value.__getitem__(0)
    except AttributeError:
        pass
    else:
        raise AssertionError('unsupported protocol exposed')

try:
    [1].__getitem__(2)
except IndexError:
    pass
else:
    raise AssertionError('expected IndexError')

try:
    {}.__getitem__('missing')
except KeyError:
    pass
else:
    raise AssertionError('expected KeyError')

True
