import random
from collections import deque, namedtuple
from datetime import date

# === missing methods fail before any arguments ===


class Empty:
    pass


seen = []


def missing_arg():
    seen.append(1)
    return 1


def star():
    seen.append(2)
    return [1]


def kw():
    seen.append(3)
    return {'x': 1}


for obj in [Empty(), Empty, [], {}, 'text', b'text', set()]:
    try:
        obj.missing(missing_arg())
    except AttributeError:
        pass
    try:
        obj.missing(x=missing_arg())
    except AttributeError:
        pass
    try:
        obj.missing(*star(), **kw())
    except AttributeError:
        pass
    try:
        obj.missing(missing_arg(), *star(), x=missing_arg(), **kw())
    except AttributeError:
        pass
assert seen == []

# === method selection precedes argument side effects ===


class Mutable:
    def method(self, value=0):
        return 'original'


obj = Mutable()


def replace():
    obj.method = lambda value=0: 'replacement'
    return 1


assert obj.method(replace()) == 'original'
assert obj.method() == 'replacement'
obj = Mutable()
assert obj.method(value=replace()) == 'original'
obj = Mutable()
assert obj.method(*[replace()]) == 'original'
obj = Mutable()
assert obj.method(*[], *[replace()], **{}) == 'original'


class D:
    def method(self, value):
        return 'old class method'


def replace_class():
    D.method = lambda self, value: 'new class method'
    return 1


assert D().method(replace_class()) == 'old class method'
assert D().method(1) == 'new class method'

# === noncallable attributes evaluate arguments before type error ===


class NonCallable:
    method = 42


seen = []


def noncallable_arg():
    seen.append(1)
    return 0


for obj in [NonCallable(), NonCallable]:
    try:
        obj.method(noncallable_arg())
    except TypeError:
        pass
    else:
        raise AssertionError('noncallable attribute was called')
assert seen == [1, 1]

# === native methods can be saved and called bound or unbound ===

items = []
append = items.append
append(1)
getattr(items, 'append')(2)
unbound = list.append
unbound(items, 3)
assert items == [1, 2, 3]
assert str.upper('hello') == 'HELLO'
assert getattr('hello', ''.join(['up', 'per']))() == 'HELLO'
assert list.__class_getitem__(int) == list[int]
assert dict.fromkeys(['a'], 1) == {'a': 1}
fromkeys = {}.fromkeys
assert fromkeys(['a'], 2) == {'a': 2}
parse = date.fromisoformat
assert parse('2024-01-02') == date(2024, 1, 2)
assert date(2020, 1, 1).fromisoformat('2024-01-02') == date(2024, 1, 2)
assert b'ab'.upper() == b'AB'
assert bytes.upper(b'ab') == b'AB'
assert bytes.fromhex('6162') == b'ab'
assert b''.fromhex('6162') == b'ab'
assert random.Random(1).random() >= 0
q = deque()
q.append(1)
assert q.pop() == 1
Point = namedtuple('Point', ['x', 'y'])
p = Point._make([1, 2])
assert p._asdict() == {'x': 1, 'y': 2}
assert p.count(1) == 1

# === rejected native receivers release arguments ===

for args in [(), (42,), (42, [1])]:
    try:
        list.append(*args)
    except TypeError:
        pass
    else:
        raise AssertionError('invalid receiver accepted')

True
