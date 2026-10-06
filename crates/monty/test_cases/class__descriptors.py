events = []


class Descriptor:
    def __get__(self, obj, owner):
        events.append((obj, owner))
        if obj is None:
            return self
        return obj.stored


class C:
    field = Descriptor()


x = C()
x.stored = 7
assert x.field == 7
assert events[-1] == (x, C)
assert C.field is C.field
assert events[-1] == (None, C)
assert getattr(x, 'field') == 7
assert hasattr(x, 'field')

# A non-data descriptor is shadowed by instance attributes.
x.field = 9
before = len(events)
assert x.field == 9
assert len(events) == before

# Descriptor objects in instance storage are returned without invoking __get__.
x.local = Descriptor()
assert type(x.local) is Descriptor

# The descriptor's protocol methods are looked up on its class, not its instance.
raw = C.field
raw.__get__ = lambda obj, owner: 100
assert C.field is raw


class Data:
    def __get__(self, obj, owner):
        if obj is None:
            return self
        return obj.stored * 2

    def __set__(self, obj, value):
        obj.stored = value
        return ['ignored setter result']


# Install a data descriptor after instance storage already holds the same name.
C.field = Data()
assert x.field == 14
x.field = 11
assert x.stored == 11 and x.field == 22
setattr(x, 'field', 12)
assert x.stored == 12
object.__setattr__(x, 'field', 13)
assert x.stored == 13

# Assigning to the class replaces the descriptor without calling its setter.
C.field = 20
assert x.field == 9  # The previous instance attribute was never overwritten.


class SetterOnly:
    def __set__(self, obj, value):
        obj.stored = value


C.field = SetterOnly()
assert x.field == 9  # No __get__, so the instance value remains visible.
x.field = 15
assert x.stored == 15


class DeleteOnly:
    def __get__(self, obj, owner):
        return 30

    def __delete__(self, obj):
        pass


C.field = DeleteOnly()
assert x.field == 30
try:
    x.field = 5
except AttributeError:
    pass
else:
    assert False, 'a delete-only descriptor must reject assignment'


class Missing:
    def __set__(self, obj, value):
        pass

    def __get__(self, obj, owner):
        raise AttributeError('unavailable')


C.field = Missing()
assert getattr(x, 'field', 42) == 42
assert not hasattr(x, 'field')


class BadGetter:
    __get__ = None

    def __set__(self, obj, value):
        pass


C.field = BadGetter()
try:
    x.field
except TypeError:
    pass
else:
    assert False


class Methods:
    def foo(self, value):
        return value + 1


m = Methods()
assert m.foo(2) == 3
saved = m.foo
assert saved(2) == 3
assert Methods.foo(m, 2) == 3
m.foo = lambda value: value + 10
assert m.foo(2) == 12
