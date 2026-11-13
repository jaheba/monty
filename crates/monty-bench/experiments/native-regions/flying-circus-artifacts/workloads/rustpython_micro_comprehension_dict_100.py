# Adapted from RustPython; source and license in ../vendor/rustpython/.
ITERATIONS = 100

# --- benchmark ---
obj = {i: i for i in range(ITERATIONS)}
print(len(obj), sum(obj.values()))
