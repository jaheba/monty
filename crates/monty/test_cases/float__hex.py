cases = [
    (0.0, '0x0.0p+0'),
    (-0.0, '-0x0.0p+0'),
    (1.0, '0x1.0000000000000p+0'),
    (-1.0, '-0x1.0000000000000p+0'),
    (1.5, '0x1.8000000000000p+0'),
    (0.1, '0x1.999999999999ap-4'),
    (5e-324, '0x0.0000000000001p-1022'),
    (-5e-324, '-0x0.0000000000001p-1022'),
    (2.225073858507201e-308, '0x0.fffffffffffffp-1022'),
    (2.2250738585072014e-308, '0x1.0000000000000p-1022'),
    (1.7976931348623157e308, '0x1.fffffffffffffp+1023'),
    (float('inf'), 'inf'),
    (float('-inf'), '-inf'),
    (float('nan'), 'nan'),
    (float('-nan'), 'nan'),
]
for value, expected in cases:
    assert value.hex() == expected

for args, kwargs, message in [
    (([1],), {}, 'float.hex() takes no arguments (1 given)'),
    ((), {'other': [1]}, 'float.hex() takes no keyword arguments'),
    (([1],), {'other': [1]}, 'float.hex() takes no keyword arguments'),
]:
    try:
        (1.0).hex(*args, **kwargs)
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == message
