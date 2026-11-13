# Adapted from RustPython; source and license in ../vendor/rustpython/.
ITERATIONS = 500
from random import random, seed
seed(0)

unsorted_list = [random() for _ in range(5 * ITERATIONS)]

# --- benchmark ---
# Setup code only runs once so do not modify in-place
result = sorted(unsorted_list)
assert all(result[i] <= result[i + 1] for i in range(len(result) - 1))
print(len(result))
