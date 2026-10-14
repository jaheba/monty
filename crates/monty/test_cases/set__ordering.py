def cmp(left, op, right):
    if op == '<':
        return left < right
    if op == '<=':
        return left <= right
    if op == '>':
        return left > right
    if op == '>=':
        return left >= right
    raise ValueError('unknown comparison: ' + op)


for left_type in (set, frozenset):
    for right_type in (set, frozenset):
        for left, right, expected in (
            ([], [], {'<': False, '<=': True, '>': False, '>=': True}),
            ([], [1], {'<': True, '<=': True, '>': False, '>=': False}),
            ([1], [], {'<': False, '<=': False, '>': True, '>=': True}),
            ([1, 2], [2, 1], {'<': False, '<=': True, '>': False, '>=': True}),
            ([1], [1, 2], {'<': True, '<=': True, '>': False, '>=': False}),
            ([1, 2], [1], {'<': False, '<=': False, '>': True, '>=': True}),
            ([1], [2], {'<': False, '<=': False, '>': False, '>=': False}),
            ([1, 2], [2, 3, 4], {'<': False, '<=': False, '>': False, '>=': False}),
            ([1, 2, 3], [2, 4], {'<': False, '<=': False, '>': False, '>=': False}),
            ([frozenset([1])], [frozenset([1]), frozenset([2])], {'<': True, '<=': True, '>': False, '>=': False}),
        ):
            a = left_type(left)
            b = right_type(right)

            for op, result in expected.items():
                assert cmp(a, op, b) == result

    a = left_type([1])
    assert not a < a
    assert a <= a
    assert not a > a
    assert a >= a
    assert a <= {1: 'a'}.keys()
    assert a < {1: 'a', 2: 'b'}.keys()
    assert a >= {1: 'a'}.keys()
    assert a > {}.keys()
    assert left_type([(1, 'a')]) <= {1: 'a'}.items()

    for other in ([1], (1,), {1: 'a'}, '1', 1, None):
        for first, second in ((a, other), (other, a)):
            for op in ('<', '<=', '>', '>='):
                try:
                    cmp(first, op, second)
                except TypeError as exc:
                    expected = (
                        "'"
                        + op
                        + "' not supported between instances of '"
                        + type(first).__name__
                        + "' and '"
                        + type(second).__name__
                        + "'"
                    )
                    assert str(exc) == expected
                else:
                    assert False, 'expected TypeError'


class Key:
    def __init__(self, value):
        self.value = value

    def __hash__(self):
        assert not reject_hash
        return self.value

    def __eq__(self, other):
        return self.value == other.value


reject_hash = False
for left_type in (set, frozenset):
    for right_type in (set, frozenset):
        a = left_type([Key(1)])
        b = right_type([Key(1), Key(2)])
        reject_hash = True
        assert a < b and a <= b and b > a and b >= a
        reject_hash = False

for collection in (set, frozenset):
    a = collection([1])
    assert {1: 'a'}.keys() <= a
    assert {1: 'a', 2: 'b'}.keys() > a
    assert {}.keys() < a

    a = collection([(1, 'a')])
    assert {1: 'a'}.items() <= a
    assert {}.items() < a
    assert {1: 'a'}.items() >= a
    assert {1: 'a', 2: 'b'}.items() > a

    items = {1: []}.items()
    assert not collection([(1, 2)]) <= items
    assert not collection([(1, 2)]) < items
    assert not collection([(1, 2)]) > items
    assert not collection() >= items


class ExplodingEquality:
    def __hash__(self):
        return 1

    def __eq__(self, other):
        raise ValueError('comparison failed')


for collection in (set, frozenset):
    a = collection([ExplodingEquality()])
    b = collection([ExplodingEquality()])
    assert not a < b
    assert not a > b
    for op in ('<=', '>='):
        try:
            cmp(a, op, b)
        except ValueError as exc:
            assert str(exc) == 'comparison failed'
        else:
            assert False, 'expected ValueError'
