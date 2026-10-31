# /// monty
# [bench]
# data = 'data/rebar/en-sampled.txt'
# ///

# Rebar's literal-search corpus and pattern, using Python string operations.
# Excerpts add character-index conversion and slicing around each match.
text = DATA
needle = 'Sherlock Holmes'
position = 0
count = 0
checksum = 0
while True:
    position = text.find(needle, position)
    if position < 0:
        break
    count += 1
    checksum += position + len(text[position : position + 48])
    position += len(needle)

assert count == 513
assert checksum == 236800171
assert text.count(needle) == 513
assert text.rfind(needle) == 896565
