# === Byte order and signed values ===
assert int.from_bytes(b'') == 0
assert int.from_bytes(b'\x01\x02') == 258
assert int.from_bytes(b'\x01\x02', 'little') == 513
assert int.from_bytes(bytes=b'\x01\x02', byteorder='big', signed=False) == 258
assert int.from_bytes(b'\xff') == 255
assert int.from_bytes(b'\xff', signed=True) == -1
assert int.from_bytes(b'\x80\x00', signed=True) == -32768
assert int.from_bytes(b'\x00\x80', 'little', signed=True) == -32768
assert int.from_bytes(b'\x7f\xff', signed=True) == 32767
assert int.from_bytes(b'\xff' * 9) == 2**72 - 1
assert int.from_bytes(b'\xff' * 9, signed=True) == -1
assert int.from_bytes(b'\x80' + b'\x00' * 15) == 2**127
assert int.from_bytes(b'\x80' + b'\x00' * 15, signed=True) == -(2**127)
assert int.from_bytes(b'\x00' * 15 + b'\x80', 'little', signed=True) == -(2**127)
assert int.from_bytes(b'\x00' * 40 + b'\x01') == 1
assert int.from_bytes(b'\xff', signed=[]) == 255
assert int.from_bytes(b'\xff', signed=[1]) == -1

# === Iterables of integer bytes ===
assert int.from_bytes([]) == 0
assert int.from_bytes([65, 66]) == 16706
assert int.from_bytes((65, 66), 'little') == 16961
assert int.from_bytes(iter([1, 2, 3])) == 66051
assert int.from_bytes(x for x in [1, 2, 3]) == 66051
assert int.from_bytes([True, False]) == 256
assert int.from_bytes(range(256), 'little') == sum(n << (n * 8) for n in range(256))


class ByteValue:
    def __index__(self):
        return 65


class BytesValue:
    def __bytes__(self):
        return b'\x01\x02'

    def __iter__(self):
        raise ValueError('__bytes__ must win')


assert int.from_bytes([ByteValue(), 66]) == 16706
assert int.from_bytes(BytesValue()) == 258

# The same classmethod works through instances and the inherited bool type.
assert (123).from_bytes(b'\x01\x02') == 258
large = 2**100
assert large.from_bytes(b'\x01\x02') == 258
assert bool.from_bytes(b'\x01') is True
assert bool.from_bytes(b'\x00') is False
assert True.from_bytes(b'\xff', signed=True) is True


def check_error(args, kwargs, error_type, message):
    try:
        int.from_bytes(*args, **kwargs)
        assert False, 'expected conversion error'
    except error_type as exc:
        assert str(exc) == message


check_error((), {}, TypeError, "from_bytes() missing required argument 'bytes' (pos 1)")
check_error((b'a', 'big', True), {}, TypeError, 'from_bytes() takes at most 2 positional arguments (3 given)')
check_error((b'a',), {'unknown': 1}, TypeError, "from_bytes() got an unexpected keyword argument 'unknown'")
check_error((b'a',), {'bytes': b'b'}, TypeError, "argument for from_bytes() given by name ('bytes') and position (1)")
check_error((b'a', None), {}, TypeError, "from_bytes() argument 'byteorder' must be str, not None")
check_error((b'a', 1), {}, TypeError, "from_bytes() argument 'byteorder' must be str, not int")
check_error((b'a', None), {'unknown': 1}, TypeError, "from_bytes() got an unexpected keyword argument 'unknown'")
check_error(
    (b'a', 1), {'byteorder': 'big'}, TypeError, "argument for from_bytes() given by name ('byteorder') and position (2)"
)
check_error((b'a', 'big', True), {'unknown': 1}, TypeError, 'from_bytes() takes at most 3 arguments (4 given)')
check_error((b'a', 'bad'), {}, ValueError, "byteorder must be either 'little' or 'big'")
check_error((3, 'bad'), {}, ValueError, "byteorder must be either 'little' or 'big'")
check_error(
    (b'a', 'bad'), {'signed': NotImplemented}, TypeError, 'NotImplemented should not be used in a boolean context'
)
check_error(
    (b'a', None), {'signed': NotImplemented}, TypeError, "from_bytes() argument 'byteorder' must be str, not None"
)
for value in [3, True, 'ab', None, 1.5]:
    check_error((value,), {}, TypeError, f"cannot convert '{type(value).__name__}' object to bytes")
for value in [-1, 256, 2**100, -(2**100)]:
    check_error(([value],), {}, ValueError, 'bytes must be in range(0, 256)')
check_error(([1.2],), {}, TypeError, "'float' object cannot be interpreted as an integer")
check_error((['a'],), {}, TypeError, "'str' object cannot be interpreted as an integer")


class BadBytes:
    def __bytes__(self):
        return 'abc'


class RaisingBytes:
    def __bytes__(self):
        raise ValueError('bytes conversion failed')


class RaisingIndex:
    def __index__(self):
        raise ValueError('index conversion failed')


class RaisingIterator:
    def __iter__(self):
        raise ValueError('iteration failed')


check_error((BadBytes(),), {}, TypeError, '__bytes__ returned non-bytes (type str)')
check_error((RaisingBytes(),), {}, ValueError, 'bytes conversion failed')
check_error(([RaisingIndex()],), {}, ValueError, 'index conversion failed')
check_error((RaisingIterator(),), {}, ValueError, 'iteration failed')
