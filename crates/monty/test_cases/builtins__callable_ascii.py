import dataclasses
import functools
import math
import sys
import typing
from collections import namedtuple


def function():
    raise ValueError('callable must not execute the function')


class Plain:
    pass


class Called:
    def __call__(self):
        raise ValueError('callable must not invoke __call__')


class DisabledCall:
    __call__ = None


class NonFunctionCall:
    __call__ = 123


class HasMethod:
    def method(self):
        pass


assert callable(function) is True
assert callable(lambda x: x) is True
assert callable(HasMethod().method) is True
assert callable(Plain) is True
assert callable(Plain()) is False
assert callable(Called()) is True
assert callable(DisabledCall()) is True
assert callable(NonFunctionCall()) is True
assert callable(functools.partial(function)) is True
assert callable(functools.lru_cache()(function)) is True
assert callable(math.sqrt) is True
assert callable(print) is True
assert callable(callable) is True
assert callable(ascii) is True
for constructor in [int, str, bool, list, dict, set, tuple, float, complex, range, ValueError]:
    assert callable(constructor) is True
for value in [None, True, 1, 2**100, 1.5, 1j, '', b'', [], {}, (), set(), range(3), math]:
    assert callable(value) is False
assert callable(sys.stdout) is False
assert callable(dataclasses.MISSING) is False
assert callable(list[int]) is True
assert callable(int | None) is False

Pair = namedtuple('Pair', ['left', 'right'])
assert callable(Pair) is True
assert callable(Pair(1, 2)) is False

instance = Plain()
instance.__call__ = function
assert callable(instance) is False
Plain.__call__ = function
assert callable(instance) is True
Plain.__call__ = None
assert callable(instance) is True

for name in [
    'Any',
    'Optional',
    'List',
    'Dict',
    'Tuple',
    'Set',
    'FrozenSet',
    'Callable',
    'Type',
    'Sequence',
    'Mapping',
    'Iterable',
    'Iterator',
    'Generator',
    'ClassVar',
    'Final',
    'Literal',
    'TypeVar',
    'Generic',
    'Protocol',
    'Annotated',
    'Self',
    'Never',
    'NoReturn',
]:
    assert callable(getattr(typing, name)) is True

# ASCII output uses one byte, BMP, and astral escape widths without changing repr's quotes.
assert ascii('plain') == "'plain'"
assert ascii('café') == "'caf\\xe9'"
assert ascii('文') == "'\\u6587'"
assert ascii('😀') == "'\\U0001f600'"
assert ascii("l'été") == '"l\'\\xe9t\\xe9"'
assert ascii('\n\t\x00é') == "'\\n\\t\\x00\\xe9'"
assert ascii(['é', {'文': '😀'}]) == "['\\xe9', {'\\u6587': '\\U0001f600'}]"
assert ascii(123) == '123'
assert ascii(b'\xff') == "b'\\xff'"


class UnicodeRepr:
    def __repr__(self):
        return 'café 文 😀'


class BadRepr:
    def __repr__(self):
        return 1


class RaisingRepr:
    def __repr__(self):
        raise ValueError('repr failed')


assert ascii(UnicodeRepr()) == 'caf\\xe9 \\u6587 \\U0001f600'
try:
    ascii(BadRepr())
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == '__repr__ returned non-string (type int)'
try:
    ascii(RaisingRepr())
    assert False, 'expected ValueError'
except ValueError as exc:
    assert str(exc) == 'repr failed'


def check_arity_error(function, name):
    try:
        function()
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == f'{name}() takes exactly one argument (0 given)'
    try:
        function(1, 2)
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == f'{name}() takes exactly one argument (2 given)'
    try:
        function(obj=1)
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == f'{name}() takes no keyword arguments'


check_arity_error(callable, 'callable')
check_arity_error(ascii, 'ascii')
