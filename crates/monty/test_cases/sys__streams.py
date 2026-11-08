import sys

assert sys.stdout.write('plain') == 5
# Embedded CPython on Windows can use cp1252; Monty's stream transport accepts Unicode on every host.
if sys.platform != 'win32':
    assert sys.stdout.write(' é文😀\n') == 5
assert sys.stderr.write('error\n') == 6
assert sys.stdout.write('') == 0
assert sys.stderr.write('') == 0
assert sys.stdout.flush() is None
assert sys.stderr.flush() is None
print('after writes')
