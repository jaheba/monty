def calculate(n):
    a, b, c, d = 1, 2, 3, 4
    for i in range(n):
        a, b, c, d = b, c, d, a
    return a + b + c + d
result = calculate(100000)
print(result)
