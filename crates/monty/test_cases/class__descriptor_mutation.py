class Replacing:
    def __get__(self, obj, owner):
        owner.field = 99
        self.alive = [1, 2, 3]
        return self.alive


class C:
    field = Replacing()


assert C.field == [1, 2, 3]
assert C.field == 99


class Setter:
    def __set__(self, obj, value):
        C.field = None
        self.alive = value
        obj.result = self.alive


C.field = Setter()
x = C()
x.field = [1, 2]
assert x.result == [1, 2]

# __get__ itself is called raw, rather than invoking another descriptor.
class Hook:
    def __get__(self, descriptor, owner):
        raise AssertionError('must not bind the __get__ hook')


class Nested:
    __get__ = Hook()


C.field = Nested()
try:
    x.field
except TypeError:
    pass
else:
    assert False

# Existing dataclass-generated field reads must also invoke descriptors.
import dataclasses


@dataclasses.dataclass
class D:
    value: int


a = D(2)
b = D(2)


class Field:
    def __get__(self, obj, owner):
        return obj.storage

    def __set__(self, obj, value):
        obj.storage = value


D.value = Field()
a.value = 4
b.value = 4
assert a == b
assert repr(a) == 'D(value=4)'
c = D(6)
assert c.value == 6

# Implicit special methods bind through the same descriptor implementation.
class Repr:
    def __get__(self, obj, owner):
        return lambda: 'descriptor repr'


class Init:
    def __get__(self, obj, owner):
        def initialize(value):
            obj.value = value
        return initialize


class Special:
    __init__ = Init()
    __repr__ = Repr()


special = Special(7)
assert special.value == 7
assert repr(special) == 'descriptor repr'
