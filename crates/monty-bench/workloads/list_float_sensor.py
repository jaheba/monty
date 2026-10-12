# --- benchmark ---
# Calibrated sensor readings, block statistics and a downsampled history.
raw = [18.0 + ((index * 37) % 1000) / 100.0 for index in range(160000)]
readings = [value * 1.025 - 0.75 for value in raw]
history = readings[::4].copy()
means = []
spreads = []
for start in range(0, len(readings), 256):
    block = readings[start : start + 256]
    mean = sum(block) / len(block)
    means.append(mean)
    spreads.append(sum([(value - mean) ** 2 for value in block]) / len(block))
assert len(means) == 625
assert len(history) == 40000
assert all(value > 0.0 for value in spreads)
print(len(readings), round(sum(means), 6), round(sum(spreads), 6), round(sum(history), 6))
