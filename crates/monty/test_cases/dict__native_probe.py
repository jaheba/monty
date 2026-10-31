# Native lookups must still call a colliding custom key's equality exactly once.
events = []
needle = 'ACGTACGTACGT'


class Key:
    def __hash__(self):
        return hash(needle)

    def __eq__(self, other):
        events.append(other)
        return other == needle


key = Key()
counts = {key: 7}
assert counts.get(needle) == 7
assert events == [needle]
counts[needle] = 8
assert events == [needle, needle]
assert list(counts.keys())[0] is key
assert counts[key] == 8
values = {key}
assert needle in values
assert events == [needle, needle, needle]

# A callback may replace the table; the slow probe must restart against it.
events = []


class ReplacingKey:
    def __hash__(self):
        return hash(needle)

    def __eq__(self, other):
        events.append(other)
        replaced.clear()
        replaced[needle] = 99
        return False


replacement = ReplacingKey()
replaced = {replacement: 1}
assert replaced.get(needle) == 99
assert events == [needle]
assert replaced == {needle: 99}

# Reverse comparisons also need the callback path when the lookup key is custom.
events = []
assert {needle: 42}.get(key) == 42
assert events == [needle]
assert key in {needle}
assert events == [needle, needle]
