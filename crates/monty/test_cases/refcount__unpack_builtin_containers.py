first = [1]
second = [2]
pair = (first, second)
values = [first, second]
a, b = pair
x, y = values
assert a is x is first
assert b is y is second
try:
    a, b, c = pair
except ValueError:
    pass
try:
    values[0], values[99] = pair
except IndexError:
    pass
a = b = x = y = pair = values = None
first
# ref-counts={'first': 2, 'second': 1}
