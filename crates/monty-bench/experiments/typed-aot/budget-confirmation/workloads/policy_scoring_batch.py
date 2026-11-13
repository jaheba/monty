# Deterministic credit-policy scoring over ordinary Python dictionaries.
def score(row: dict[str, float]) -> float:
    income = row['income']
    limit = row['limit']
    balance = row['balance']
    late = row['late_payments']
    age = row['account_age']
    utilization = balance / limit
    result = 600.0 + income * 0.0008 + age * 0.15
    result -= utilization * 110.0 + late * 22.0
    if utilization > 0.8:
        result -= 35.0
    if late > 2.0:
        result -= 40.0
    if income < 30000.0:
        result -= 20.0
    if result > 850.0:
        result = 850.0
    return result

rows: list[dict[str, float]] = [
    {
        'income': float(20000 + (i * 7919) % 130000),
        'limit': float(2000 + (i * 13) % 18000),
        'balance': float((i * 97) % 20000),
        'late_payments': float(i % 5),
        'account_age': float(12 + i % 240),
    }
    for i in range(5000)
]
total = 0.0
accepted = 0
for row in rows:
    result = score(row)
    total += result
    if result >= 650.0:
        accepted += 1
print(f'{len(rows)} {accepted} {total:.3f}')
