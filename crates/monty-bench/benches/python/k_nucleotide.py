# Generate a fixed, non-periodic DNA sequence without filesystem input.
seed = 42
bases = []
for _ in range(10_000):
    seed = (seed * 3877 + 29573) % 139968
    r = seed / 139968
    if r < 0.3029549426680:
        bases.append('a')
    elif r < 0.5009432431601:
        bases.append('c')
    elif r < 0.6984905497992:
        bases.append('g')
    else:
        bases.append('t')
sequence = ''.join(bases).upper()


def frequencies(sequence, k):
    counts = {}
    for i in range(len(sequence) - k + 1):
        fragment = sequence[i : i + k]
        counts[fragment] = counts.get(fragment, 0) + 1
    return counts


# Fold the ranked one- and two-base tables and the five query counts into one result.
checksum = 0
for k in (1, 2):
    counts = frequencies(sequence, k)
    ranked = sorted(counts, key=lambda fragment: (-counts[fragment], fragment))
    for rank, fragment in enumerate(ranked):
        checksum += (rank + 1) * counts[fragment]

for fragment in ('GGT', 'GGTA', 'GGTATT', 'GGTATTTTAATT', 'GGTATTTTAATTTATAGT'):
    counts = frequencies(sequence, len(fragment))
    checksum = checksum * 31 + counts.get(fragment, 0)

assert checksum == 2_692_365_025_979
