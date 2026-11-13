# Adapted from RustPython; source and license in ../vendor/rustpython/.
ITERATIONS = 300
obj = []

# --- benchmark ---
for i in range(ITERATIONS):
    obj.append(i)
print(len(obj), sum(obj))
