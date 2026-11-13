# Adapted from RustPython; source and license in ../vendor/rustpython/.
ITERATIONS = 900
fs = frozenset(range(0, ITERATIONS))

# --- benchmark ---
result = hash(fs)
print(isinstance(result, int))
