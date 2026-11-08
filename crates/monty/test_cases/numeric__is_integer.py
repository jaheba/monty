"""Integral-value predicates on immediate and heap-allocated numbers."""

# === Integers, including inherited bool methods and both large-int forms ===
for value in (0, 1, -1, -9223372036854775808, 9223372036854775807, 2**1000, -(2**1000), True, False):
    assert value.is_integer() is True
assert (12345678901234567890123456789012345678901234567890).is_integer() is True

# === Finite floats, signed zeros and IEEE special values ===
for value in (0.0, -0.0, 1.0, -1.0, 1e300, -1e300, 9007199254740992.0):
    assert value.is_integer() is True
for value in (0.5, -0.5, 1.25, -1.25, 5e-324, -5e-324, float('inf'), float('-inf'), float('nan')):
    assert value.is_integer() is False

# === Zero-argument methods reject positional arguments and keywords ===
for value, name in ((1, 'int'), (2**1000, 'int'), (True, 'int'), (1.0, 'float')):
    try:
        value.is_integer([])
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == name + '.is_integer() takes no arguments (1 given)'

    try:
        value.is_integer(argument=[])
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == name + '.is_integer() takes no keyword arguments'

    try:
        value.is_integer([], argument=[])
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == name + '.is_integer() takes no keyword arguments'
