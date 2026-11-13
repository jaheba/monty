# Adapted from RustPython; source and license in ../vendor/rustpython/.
ITERATIONS = 500
obj = []

# --- benchmark ---
for i in range(ITERATIONS):
    obj.append(i)
print(len(obj), sum(obj))
