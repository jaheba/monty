# Adapted from RustPython; source and license in ../vendor/rustpython/.
ITERATIONS = 300

# --- benchmark ---
obj = {i for i in range(ITERATIONS)}
print(len(obj), sum(obj))
