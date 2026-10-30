def calculate(n):
    items, total = list(range(64)), 0
    for i in range(n):
        total += items[i % 64]
    return total
result = calculate(100000)
print(result)
