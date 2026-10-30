def calculate(n):
    a, b, c, total = 3, 7, 11, 0
    for i in range(n):
        total += (a + b) * c
        a = (a + 1) % 97
        b = (b * 3) % 89
    return total
result = calculate(100000)
print(result)
