# Adapted from RustPython; source and license in ../vendor/rustpython/.
ITERATIONS = 300

# --- benchmark ---
total = 0
for i in range(ITERATIONS):
    total += i
print(total)
