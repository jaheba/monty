def calculate(n):
    a, b, total = 0, 7, 0
    for i in range(n):
        if a < b:
            total += 1
        if a == b:
            total += 2
        a = (a + 1) % 17
    return total
result = calculate(100000)
print(result)
