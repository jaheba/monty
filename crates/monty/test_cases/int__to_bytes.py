# === Default size and byte order ===
assert (0).to_bytes() == b'\x00'
assert (65).to_bytes() == b'A'
assert (258).to_bytes(2) == b'\x01\x02'
assert (258).to_bytes(2, 'little') == b'\x02\x01'
assert (258).to_bytes(length=4, byteorder='big') == b'\x00\x00\x01\x02'
assert (258).to_bytes(4, 'little') == b'\x02\x01\x00\x00'
assert True.to_bytes() == b'\x01'
assert False.to_bytes() == b'\x00'

# === Signed boundaries, padding and arbitrary precision ===
assert (127).to_bytes(1, signed=True) == b'\x7f'
assert (-128).to_bytes(1, signed=True) == b'\x80'
assert (-129).to_bytes(2, signed=True) == b'\xff\x7f'
assert (-129).to_bytes(4, 'little', signed=True) == b'\x7f\xff\xff\xff'
assert (2**127).to_bytes(16) == b'\x80' + b'\x00' * 15
assert (2**127).to_bytes(17, signed=True) == b'\x00\x80' + b'\x00' * 15
assert (-(2**127)).to_bytes(16, signed=True) == b'\x80' + b'\x00' * 15
assert (2**100 + 1).to_bytes(13, 'little') == b'\x01' + b'\x00' * 11 + b'\x10'
assert (0).to_bytes(0) == b''
assert (0).to_bytes(0, signed=True) == b''
assert (255).to_bytes(1, signed=[]) == b'\xff'
assert (-1).to_bytes(1, signed=[1]) == b'\xff'


class Length:
    def __index__(self):
        return 2


assert (258).to_bytes(Length()) == b'\x01\x02'


def check_error(number, args, kwargs, error_type, message):
    try:
        number.to_bytes(*args, **kwargs)
        assert False, 'expected conversion error'
    except error_type as exc:
        assert str(exc) == message


for number in [0, 1, -1, 2**100, -(2**100)]:
    check_error(number, (-1,), {}, ValueError, 'length argument must be non-negative')
    check_error(number, (-1, 'bad'), {}, ValueError, "byteorder must be either 'little' or 'big'")
    check_error(number, (None,), {}, TypeError, "'NoneType' object cannot be interpreted as an integer")
    check_error(number, (1.5,), {}, TypeError, "'float' object cannot be interpreted as an integer")
    check_error(number, (2**100,), {}, OverflowError, 'Python int too large to convert to C ssize_t')
    check_error(number, (-(2**100),), {}, OverflowError, 'Python int too large to convert to C ssize_t')
    check_error(number, (1, None), {}, TypeError, "to_bytes() argument 'byteorder' must be str, not None")
    check_error(number, (1, 1), {}, TypeError, "to_bytes() argument 'byteorder' must be str, not int")
    check_error(
        number,
        (1, 'bad'),
        {'signed': NotImplemented},
        TypeError,
        'NotImplemented should not be used in a boolean context',
    )
    check_error(number, (1, 'big', True), {}, TypeError, 'to_bytes() takes at most 2 positional arguments (3 given)')
    check_error(number, (), {'unknown': 1}, TypeError, "to_bytes() got an unexpected keyword argument 'unknown'")
    check_error(
        number, (1,), {'length': 2}, TypeError, "argument for to_bytes() given by name ('length') and position (1)"
    )
    check_error(number, (1, 'big', True), {'unknown': 1}, TypeError, 'to_bytes() takes at most 3 arguments (4 given)')

for number in [1, -1, -2, 128, -129, 256, 2**100, -(2**100)]:
    check_error(number, (0,), {'signed': True}, OverflowError, 'int too big to convert')
for number in [-1, -128, -(2**100)]:
    check_error(number, (), {}, OverflowError, "can't convert negative int to unsigned")
for number in [128, 255, -129]:
    check_error(number, (1,), {'signed': True}, OverflowError, 'int too big to convert')


class RaisingLength:
    def __index__(self):
        raise ValueError('length callback failed')


check_error(1, (RaisingLength(), 'bad'), {}, ValueError, 'length callback failed')
