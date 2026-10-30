def calculate(a, b, n):
    total = 0
    for i in range(n):
        total += a + b
    return total

result = calculate(3, 7, 100000)
print(result)
