# Adapted from RustPython; source and license in ../vendor/rustpython/.
ITERATIONS = 700
fs = frozenset(range(0, ITERATIONS))

# --- benchmark ---
result = hash(fs)
print(isinstance(result, int))
