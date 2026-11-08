def check(number, length, count):
    assert number.bit_length() == length
    assert number.bit_count() == count
    assert type(number.bit_length()) is int
    assert type(number.bit_count()) is int


for number, length, count in [
    (0, 0, 0),
    (1, 1, 1),
    (2, 2, 1),
    (3, 2, 2),
    (255, 8, 8),
    (256, 9, 1),
    (2**63 - 1, 63, 63),
    (2**63, 64, 1),
    (2**64 - 1, 64, 64),
    (2**100 + 1, 101, 2),
    (2**128 - 1, 128, 128),
    (2**10000, 10001, 1),
    (2**10000 - 1, 10000, 10000),
]:
    check(number, length, count)
    check(-number, length, count)

check(False, 0, 0)
check(True, 1, 1)

# Keep both the interned and runtime-allocated big-integer paths covered.
literal = 1267650600228229401496703205377
check(literal, 101, 2)
check(literal + 1, 101, 2)

for number in [0, False, True, -(2**63), 2**100, -(2**100)]:
    owner = 'bool' if isinstance(number, bool) else 'int'
    for args, kwargs, message in [
        ([1], {}, 'takes no arguments (1 given)'),
        ([[], 2], {}, 'takes no arguments (2 given)'),
        ([], {'x': []}, 'takes no keyword arguments'),
        ([1], {'x': 2}, 'takes no keyword arguments'),
    ]:
        try:
            number.bit_length(*args, **kwargs)
            assert False, 'expected TypeError'
        except TypeError as exc:
            assert str(exc) == f'{owner}.bit_length() {message}'
        try:
            number.bit_count(*args, **kwargs)
            assert False, 'expected TypeError'
        except TypeError as exc:
            assert str(exc) == f'{owner}.bit_count() {message}'
