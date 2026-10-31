# /// monty
# call-external = true
# ///

# Resume with a receiver and partially evaluated bounds still on the stack.
text = 'aé中🐍' * 20
assert text[add_ints(0, 1) : add_ints(2, 2)] == 'é中🐍'
assert text[add_ints(0, 1) : add_ints(2, 2) : add_ints(0, 1)] == 'é中🐍'
assert [0, 1, 2, 3][add_ints(0, 1) : add_ints(1, 2)] == [1, 2]
assert text[1:4] == 'é中🐍'
