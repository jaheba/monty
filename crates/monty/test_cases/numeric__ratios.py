"""Exact rational components of integers and binary64 floats."""

# === Integers retain their numerator, while bool returns ordinary integers ===
for value in (0, 1, -1, -9223372036854775808, 9223372036854775807, 2**1000, -(2**1000)):
    ratio = value.as_integer_ratio()
    assert ratio == (value, 1)
    assert ratio[0] is value
    assert type(ratio[0]) is int
    assert type(ratio[1]) is int
assert (12345678901234567890123456789012345678901234567890).as_integer_ratio() == (
    12345678901234567890123456789012345678901234567890,
    1,
)
assert True.as_integer_ratio() == (1, 1)
assert False.as_integer_ratio() == (0, 1)
assert type(True.as_integer_ratio()[0]) is int
assert type(False.as_integer_ratio()[0]) is int

# === Lowest terms, negative zeros, subnormal values and both exponent extremes ===
for value, expected in (
    (0.0, (0, 1)),
    (-0.0, (0, 1)),
    (0.5, (1, 2)),
    (-1.5, (-3, 2)),
    (0.1, (3602879701896397, 36028797018963968)),
    (5e-324, (1, 2**1074)),
    (-5e-324, (-1, 2**1074)),
    (2.2250738585072014e-308, (1, 2**1022)),
    (1.7976931348623157e308, (2**1024 - 2**971, 1)),
):
    ratio = value.as_integer_ratio()
    assert ratio == expected
    assert type(ratio[0]) is int
    assert type(ratio[1]) is int

# === Non-finite values and invalid method arguments ===
for value in (float('inf'), float('-inf')):
    try:
        value.as_integer_ratio()
        assert False, 'expected OverflowError'
    except OverflowError as exc:
        assert str(exc) == 'cannot convert Infinity to integer ratio'
try:
    float('nan').as_integer_ratio()
    assert False, 'expected ValueError'
except ValueError as exc:
    assert str(exc) == 'cannot convert NaN to integer ratio'

for value, name in ((1, 'int'), (2**1000, 'int'), (True, 'int'), (1.0, 'float')):
    try:
        value.as_integer_ratio([])
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == name + '.as_integer_ratio() takes no arguments (1 given)'
    try:
        value.as_integer_ratio(argument=[])
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == name + '.as_integer_ratio() takes no keyword arguments'
