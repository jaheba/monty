# Search boundaries use Python character positions, including empty needles.
for text in ('', 'abcdef', 'äbc中文🐍def', 'aé中🐍' * 200):
    characters = list(text)
    for needle in ('', 'a', '中', '中🐍', 'not-present'):
        for start in (-10000, -10, -1, 0, 1, 6, 10000):
            for stop in (-10000, -1, 0, 1, 8, 10000):
                low = max(len(text) + start, 0) if start < 0 else start
                high = max(len(text) + stop, 0) if stop < 0 else min(stop, len(text))
                matches = []
                if low <= high:
                    for pos in range(low, high + 1):
                        if pos + len(needle) <= high and ''.join(characters[pos : pos + len(needle)]) == needle:
                            matches.append(pos)
                assert text.find(needle, start, stop) == (matches[0] if matches else -1)
                assert text.rfind(needle, start, stop) == (matches[-1] if matches else -1)
                if not needle:
                    assert text.count(needle, start, stop) == len(matches)
    assert text.find('not-present') == -1
    assert text.count('not-present') == 0
    assert text.find('') == 0
    assert text.rfind('') == len(text)
    assert text.count('') == len(text) + 1


class Boundary:
    def __index__(self):
        # Re-enter the same string while its search arguments are converted.
        assert text.find('中') == 2
        assert text[-2] == '中'
        return 4


text = 'aé中🐍' * 1000
assert text.find('中', Boundary()) == 6
assert text.rfind('中', 0, Boundary()) == 2
