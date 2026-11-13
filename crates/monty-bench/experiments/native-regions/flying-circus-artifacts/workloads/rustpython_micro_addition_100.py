# Adapted from RustPython; source and license in ../vendor/rustpython/.
ITERATIONS = 100

# --- benchmark ---
total = 0
for i in range(ITERATIONS):
    total += i
print(total)
