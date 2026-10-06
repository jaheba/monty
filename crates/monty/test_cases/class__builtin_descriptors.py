class Counter:
    def __init__(self, value=0):
        self._value = value

    @property
    def value(self):
        return self._value

    @value.setter
    def value(self, new_value):
        self._value = new_value

    @classmethod
    def create(cls, value):
        return cls(value)

    @staticmethod
    def add(a, b=1):
        return a + b

counter = Counter.create(3)
assert counter.value == 3
counter.value = 7
assert counter.value == 7
assert Counter.value.fset is not None
assert Counter.add(2) == counter.add(2) == 3
assert counter.create(9).value == 9
saved = counter.create
assert saved(11).value == 11

class ReadOnly:
    @property
    def value(self):
        return 42

try:
    ReadOnly().value = 0
except AttributeError:
    pass
else:
    raise AssertionError('read-only property accepted assignment')

# Constructors and accessor decorators preserve the original property.
def read(obj):
    return obj._value


def write(obj, value):
    obj._value = value
    return 'ignored'


def remove(obj):
    obj._value = None


original = property(read, doc='value documentation')
updated = original.setter(write).deleter(remove)
assert original.fset is None
assert updated.fget is read
assert updated.fset is write
assert updated.fdel is remove
assert updated.__doc__ == 'value documentation'

class Manual:
    field = updated

m = Manual()
m.field = 12
assert m.field == 12
assert Manual.field is updated
assert isinstance(updated, property)
assert type(updated) is property

# Static and class methods are non-data descriptors and can be shadowed.
class Methods:
    @classmethod
    def owner(cls):
        return cls

    @staticmethod
    def double(value):
        return value * 2

method_instance = Methods()
assert Methods.owner() is Methods
assert method_instance.owner() is Methods
assert Methods.double(value=4) == 8
assert method_instance.double(*(5,)) == 10
method_instance.owner = lambda: 'instance'
method_instance.double = lambda value: value + 1
assert method_instance.owner() == 'instance'
assert method_instance.double(2) == 3
assert Methods.owner() is Methods
assert Methods.double(2) == 4

wrapped_static = staticmethod(read)
wrapped_class = classmethod(read)
assert wrapped_static.__func__ is read
assert wrapped_static.__wrapped__ is read
assert wrapped_class.__func__ is read
assert wrapped_class.__wrapped__ is read
assert type(wrapped_static) is staticmethod
assert type(wrapped_class) is classmethod
assert wrapped_static(m) == 12
for non_callable in (wrapped_class, updated):
    try:
        non_callable(m)
    except TypeError:
        pass
    else:
        raise AssertionError('non-callable descriptor accepted a call')

# User decorators follow the same class-body path.
def increment_result(function):
    def wrapped(self, value):
        return function(self, value) + 1
    return wrapped

class Decorated:
    @increment_result
    def method(self, value):
        return value * 2

assert Decorated().method(3) == 7
assert Decorated.method(Decorated(), 3) == 7

# A setter-only property rejects reads but accepts writes.
class WriteOnly:
    value = property(fset=write)

w = WriteOnly()
w.value = 22
assert w._value == 22
try:
    w.value
except AttributeError:
    pass
else:
    raise AssertionError('property without a getter accepted a read')

# Malformed constructor calls release their arguments.
for constructor in (classmethod, staticmethod):
    for args in ((), (read, write)):
        try:
            constructor(*args)
        except TypeError:
            pass
        else:
            raise AssertionError('invalid wrapper constructor accepted arguments')
    try:
        constructor(function=read)
    except TypeError:
        pass
    else:
        raise AssertionError('wrapper constructor accepted keyword arguments')

# Property objects are shared by both copy operations.
import copy

assert copy.copy(updated) is updated
assert copy.deepcopy(updated) is updated
assert staticmethod(*(read,))(m) == 12
assert property(**{'fget': read, 'fset': write}).fset is write
