# Visit every position once, using both positive and negative indices.
sequence = 'éàöñ' * 2500
total = 0
for i in range(10_000):
    index = (i * 7919) % 10_000
    total += ord(sequence[index]) + ord(sequence[index - 10_000])
assert total == 4720000
print(len(sequence), total)
