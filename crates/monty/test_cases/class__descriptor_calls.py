events = []


def target(*args, **kwargs):
    events.append('call')
    return args, kwargs


class CallableDescriptor:
    def __get__(self, obj, owner):
        events.append('get')
        return target


class C:
    method = CallableDescriptor()


x = C()


def argument():
    events.append('arg')
    return 1


assert x.method(argument()) == ((1,), {})
assert events == ['get', 'arg', 'call']
events.clear()
assert x.method(value=argument()) == ((), {'value': 1})
assert events == ['get', 'arg', 'call']
events.clear()
assert x.method(*[argument()], **{'value': argument()}) == ((1,), {'value': 1})
assert events == ['get', 'arg', 'arg', 'call']
events.clear()
assert x.method(*[argument()], *[argument()], **{'a': 2}, **{'b': 3}) == ((1, 1), {'a': 2, 'b': 3})
assert events == ['get', 'arg', 'arg', 'call']
events.clear()
assert C.method(argument()) == ((1,), {})
assert events == ['get', 'arg', 'call']

# Retrieval produces a callable once; later calls do not rerun __get__.
events.clear()
f = x.method
assert f(argument()) == ((1,), {})
assert events == ['get', 'arg', 'call']

# A getter failure precedes argument evaluation.
class Raising:
    def __get__(self, obj, owner):
        raise ValueError('getter failed')


C.method = Raising()
events.clear()
try:
    x.method(argument())
except ValueError as exc:
    assert str(exc) == 'getter failed'
else:
    assert False
assert events == []

# Resolve the member before an argument replaces it.
C.method = CallableDescriptor()


def replace():
    C.method = lambda self, value: 'replacement'
    return 5


assert x.method(replace()) == ((5,), {})
assert x.method(5) == 'replacement'

# Native immediate calls retain their existing path, including unpacking errors.
xs = []
xs.append(1)
xs.extend(*[[2, 3]])
xs.sort(**{'reverse': True})
assert xs == [3, 2, 1]
try:
    xs.sort(**{'reverse': True}, **{'reverse': False})
except TypeError:
    pass
else:
    assert False
