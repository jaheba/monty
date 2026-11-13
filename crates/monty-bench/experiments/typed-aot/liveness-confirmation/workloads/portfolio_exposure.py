# Deterministic exposure scenarios over one shared vector of daily returns.
def exposure(changes: list[float], scale: float, reserve: float) -> float:
    result = 0.0
    for change in changes:
        loss = -change * scale
        if loss > reserve:
            excess = loss - reserve
            result += excess * excess
        else:
            result += loss * 0.05
    return result

changes: list[float] = [float((i * 7919) % 2001 - 1000) / 10000.0 for i in range(2000)]
total = 0.0
for scenario in range(12):
    total += exposure(changes, 0.75 + float(scenario) * 0.1, 0.01)
print(f'{len(changes)} {total:.6f}')
