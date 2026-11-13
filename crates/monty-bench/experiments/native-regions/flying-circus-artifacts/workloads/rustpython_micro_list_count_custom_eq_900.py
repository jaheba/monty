# Adapted from RustPython; source and license in ../vendor/rustpython/.
ITERATIONS = 900
class A:
    def __eq__(self, other):
        return True

l = [A()] * ITERATIONS

# --- benchmark ---
print(l.count(1))
