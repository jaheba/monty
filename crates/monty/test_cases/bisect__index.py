import bisect


class Index:
    def __init__(self, value):
        self.value = value

    def __index__(self):
        return self.value


class Boom:
    def __index__(self):
        raise ValueError('index failed')


class Bad:
    def __index__(self):
        return 'bad'


for f in [bisect.bisect_left, bisect.bisect_right]:
    assert f([1, 2, 3], 2, Index(0), Index(3)) in [1, 2]
    assert f([1, 2, 3], 2, hi=Index(-1)) in [1, 2]
    for kwargs, error, message in [
        ({'lo': Boom()}, ValueError, 'index failed'),
        ({'hi': Boom()}, ValueError, 'index failed'),
        ({'lo': Bad()}, TypeError, '__index__ returned non-int (type str)'),
        ({'hi': Bad()}, TypeError, '__index__ returned non-int (type str)'),
        ({'lo': Index(10**30)}, OverflowError, 'Python int too large to convert to C ssize_t'),
        ({'hi': Index(10**30)}, OverflowError, "cannot fit 'Index' into an index-sized integer"),
        ({'lo': Index(-1)}, ValueError, 'lo must be non-negative'),
    ]:
        try:
            f([1, 2], 1, **kwargs)
            assert False, 'expected failure'
        except error as exc:
            assert str(exc) == message
