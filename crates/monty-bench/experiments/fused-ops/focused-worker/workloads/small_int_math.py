def calculate(n):
    a = 1
    for i in range(n):
        a = (a * 3 + 1) % 97
    return a

result = calculate(100000)
print(result)
