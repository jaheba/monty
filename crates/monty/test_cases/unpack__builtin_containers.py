def pair():
    return [1], [2]


a, b = pair()
assert a == [1] and b == [2]
shared = []
for values in [(shared, shared), [shared, shared]]:
    a, b = values
    assert a is shared and b is shared
    a.append(42)
    assert b is values[0]

for values in [(), [], (1,), [1], (1, 2, 3), [1, 2, 3]]:
    a, b = 'old-a', 'old-b'
    try:
        a, b = values
    except ValueError as exc:
        if len(values) < 2:
            assert str(exc) == f'not enough values to unpack (expected 2, got {len(values)})'
        else:
            assert str(exc) == 'too many values to unpack (expected 2, got 3)'
    else:
        assert False
    assert a == 'old-a' and b == 'old-b'

() = ()
() = []
a, = [shared]
assert a is shared
a, (b, c) = (shared, ([1], [2]))
assert a is shared and b == [1] and c == [2]
store = [None]
try:
    store[0], store[5] = pair()
except IndexError:
    pass
assert store == [[1]]

# Generic iterators still stop at the first surplus item.
events = []


class Source:
    def __iter__(self):
        return self

    def __next__(self):
        events.append(len(events))
        return events[-1]


try:
    a, b = Source()
except ValueError as exc:
    assert str(exc) == 'too many values to unpack (expected 2)'
assert events == [0, 1, 2]
