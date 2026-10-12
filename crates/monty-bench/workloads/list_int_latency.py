# --- benchmark ---
# Latency samples in microseconds, with percentiles and an incident replay buffer.
samples = [100 + (index * 104729) % 20000 for index in range(150000)]
replay = samples[-25000:].copy()
replay.extend(samples[:25000])
for index in range(0, len(replay), 100):
    replay[index] = 50000
ordered = sorted(samples)
histogram = [0] * 21
for value in samples:
    histogram[value // 1000] += 1
assert sum(histogram) == len(samples)
assert len(replay) == 50000
print(ordered[len(ordered) // 2], ordered[len(ordered) * 99 // 100], sum(replay), histogram)
