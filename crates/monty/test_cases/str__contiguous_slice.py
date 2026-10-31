# Compare fused bounds with an explicit slice object across string storage kinds.
texts = ['', 'a', 'short ASCII', 'abcdef' * 20, 'aé中🐍', 'Straße 中文 🐍 ' * 20]
bounds = [None, False, True, -(10**30), -100, -3, 0, 1, 3, 100, 10**30]
for original in texts:
    for text in [original, ''.join([original, ''])]:
        for start in bounds:
            for stop in bounds:
                expected = text[slice(start, stop)]
                assert text[start:stop] == expected
                assert text[start:stop:1] == expected
        assert text[::-1] == text[slice(None, None, -1)]
        assert text[::2] == text[slice(None, None, 2)]

# Evaluate the receiver and bound expressions before coercing the bounds.
events = []


class Index:
    def __init__(self, label, number):
        self.label = label
        self.number = number

    def __index__(self):
        events.append(self.label)
        return self.number


def receiver():
    events.append('receiver')
    return ''.join(['aé中🐍', 'bcdefghijklmnop'])


def bound(label, number):
    events.append(label)
    return Index(label + ' index', number)


assert receiver()[bound('start', 1) : bound('stop', 4)] == 'é中🐍'
assert events == ['receiver', 'start', 'stop', 'start index', 'stop index']


# Failure must release temporary receivers and leave exception handlers usable.
class BadIndex:
    def __index__(self):
        raise ValueError('bad bound')


try:
    ('abcdef' * 20)[BadIndex() :]
    assert False
except ValueError as exc:
    assert str(exc) == 'bad bound'

for step in [0, 0.5]:
    try:
        ('abcdef' * 20)[1:4:step]
        assert False
    except (ValueError, TypeError):
        pass

assert ('abcdef' * 20)[1:4] == 'bcd'


# Custom receivers still receive a real slice through generic dispatch.
class Receiver:
    def __getitem__(self, key):
        return (key.start, key.stop, key.step)


assert Receiver()[1:4] == (1, 4, None)
assert [0, 1, 2, 3][1:3] == [1, 2]
assert (0, 1, 2, 3)[1:3] == (1, 2)
assert b'abcd'[1:3] == b'bc'


class KeepSlice:
    def __getitem__(self, key):
        return key


kept = KeepSlice()[1:4]
assert isinstance(kept, slice)
assert (kept.start, kept.stop, kept.step) == (1, 4, None)


class RaisingReceiver:
    def __getitem__(self, key):
        raise ValueError('getitem')


try:
    RaisingReceiver()[1:4]
    assert False
except ValueError as exc:
    assert str(exc) == 'getitem'
