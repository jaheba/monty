# --- benchmark ---
# Daily position valuation and periodic rebalancing across 4,096 assets.
positions = [1000.0 + index / 8.0 for index in range(4096)]
returns = [((index * 17) % 101 - 50) / 10000.0 for index in range(4096)]
valuations = []
turnover = []
for day in range(120):
    positions = [positions[index] * (1.0 + returns[(index + day) % 4096]) for index in range(4096)]
    valuations.append(sum(positions))
    if day % 20 == 19:
        before = positions.copy()
        target = sum(positions) / len(positions)
        positions = [target] * len(positions)
        turnover.append(sum([abs(positions[index] - before[index]) for index in range(4096)]))
assert len(valuations) == 120
assert len(turnover) == 6
assert all(value > 0.0 for value in turnover)
print(round(valuations[-1], 2), round(sum(turnover), 2), round(sum(valuations), 2))
