# Adapted from RustPython; source and license in ../vendor/rustpython/.
ITERATIONS = 500
l = [i for i in range(ITERATIONS)]

# --- benchmark ---
print(l.count(1))
