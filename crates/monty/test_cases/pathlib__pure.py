# /// monty
# skip-cpython-windows = true  # pathlib uses POSIX paths in Monty's sandbox, Windows CPython resolves differently
# ///
# === Path constructor ===
from pathlib import Path

# === Virtual POSIX file URIs ===
for text, expected in [
    ('/', 'file:///'),
    ('/a b/é文😀', 'file:///a%20b/%C3%A9%E6%96%87%F0%9F%98%80'),
    ('/a?b#c%/d:e', 'file:///a%3Fb%23c%25/d%3Ae'),
    ('/~user/_.-A0', 'file:///~user/_.-A0'),
    ('/a\x00b', 'file:///a%00b'),
]:
    assert Path(text).as_uri() == expected
for text in ['', '.', 'relative', 'C:/file']:
    try:
        Path(text).as_uri()
        assert False, 'expected ValueError'
    except ValueError as exc:
        assert str(exc) == "relative paths can't be expressed as file URIs"
try:
    Path('/').as_uri(1)
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == 'Path.as_uri() takes 1 positional argument but 2 were given'
try:
    Path('/').as_uri(self=None)
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == "Path.as_uri() got multiple values for argument 'self'"
try:
    Path('/').as_uri(unexpected=1)
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == "Path.as_uri() got an unexpected keyword argument 'unexpected'"
try:
    Path.cwd(cls=None)
    assert False, 'expected TypeError'
except TypeError as exc:
    assert str(exc) == "Path.cwd() got multiple values for argument 'cls'"

p = Path('/usr/local/bin/python')
assert str(p) == '/usr/local/bin/python'

# Constructor with multiple arguments
assert str(Path('folder', 'file.txt')) == 'folder/file.txt'
assert str(Path('/usr', 'local', 'bin')) == '/usr/local/bin'
assert str(Path('start', '/absolute', 'end')) == '/absolute/end'

# Constructor with no arguments
assert str(Path()) == '.'

# === name property ===
assert p.name == 'python'
assert Path('/usr/local/bin/').name == 'bin'
assert Path('/').name == ''
assert Path('file.txt').name == 'file.txt'

# === parent property ===
assert str(p.parent) == '/usr/local/bin'
assert str(Path('/usr').parent) == '/'
assert str(Path('/').parent) == '/'
assert str(Path('file.txt').parent) == '.'

# === stem property ===
assert Path('/path/file.tar.gz').stem == 'file.tar'
assert Path('/path/file.txt').stem == 'file'
assert Path('/path/.bashrc').stem == '.bashrc'
assert Path('/path/file').stem == 'file'

# === suffix property ===
assert Path('/path/file.tar.gz').suffix == '.gz'
assert Path('/path/file.txt').suffix == '.txt'
assert Path('/path/.bashrc').suffix == ''
assert Path('/path/file').suffix == ''

# === suffixes property ===
assert Path('/path/file.tar.gz').suffixes == ['.tar', '.gz']
assert Path('/path/file.txt').suffixes == ['.txt']
assert Path('/path/.bashrc').suffixes == []

# === parts property ===
assert Path('/usr/local/bin').parts == ('/', 'usr', 'local', 'bin')
assert Path('usr/local').parts == ('usr', 'local')
assert Path('/').parts == ('/',)

# === is_absolute method ===
assert Path('/usr/bin').is_absolute() == True
assert Path('usr/bin').is_absolute() == False
assert Path('').is_absolute() == False

# === joinpath method ===
assert str(Path('/usr').joinpath('local')) == '/usr/local'
assert str(Path('/usr').joinpath('local', 'bin')) == '/usr/local/bin'
assert str(Path('/usr').joinpath('/etc')) == '/etc'
assert str(Path('.').joinpath('file')) == 'file'

# === with_name method ===
assert str(Path('/path/file.txt').with_name('other.py')) == '/path/other.py'
assert str(Path('file.txt').with_name('other.py')) == 'other.py'

# === with_suffix method ===
assert str(Path('/path/file.txt').with_suffix('.py')) == '/path/file.py'
assert str(Path('/path/file.txt').with_suffix('')) == '/path/file'
assert str(Path('/path/file').with_suffix('.txt')) == '/path/file.txt'

# === / operator ===
assert str(Path('/usr') / 'local') == '/usr/local'
assert str(Path('/usr') / 'local' / 'bin') == '/usr/local/bin'

# Path / Path
assert str(Path('/usr') / Path('bin')) == '/usr/bin'
assert str(Path('/usr') / Path('/etc')) == '/etc'

# Path / heap-allocated (non-interned) str
heap_str = 'lo' + 'cal'
assert str(Path('/usr') / heap_str) == '/usr/local'

# str / Path (__rtruediv__)
assert str('/usr' / Path('bin')) == '/usr/bin'
assert str(heap_str / Path('bin')) == 'local/bin'
assert str('/usr' / Path('/etc')) == '/etc'

# absolute rhs replaces lhs
assert str(Path('/usr') / '/etc') == '/etc'

# empty and dot components
assert str(Path('.') / 'a') == 'a'
assert str(Path('/usr') / '') == '/usr'
assert str(Path('/usr') / '.') == '/usr'
assert str('' / Path('a')) == 'a'

# augmented assignment
aug = Path('/a')
aug /= 'b'
assert str(aug) == '/a/b'
aug /= Path('c')
assert str(aug) == '/a/b/c'

# unsupported operand types raise TypeError matching CPython
try:
    Path('/usr') / 1
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == "unsupported operand type(s) for /: 'PosixPath' and 'int'"
try:
    1 / Path('/usr')
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == "unsupported operand type(s) for /: 'int' and 'PosixPath'"
try:
    Path('/usr') / b'bin'
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == "unsupported operand type(s) for /: 'PosixPath' and 'bytes'"
try:
    b'/usr' / Path('bin')
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == "unsupported operand type(s) for /: 'bytes' and 'PosixPath'"

# === as_posix method ===
assert Path('/usr/bin').as_posix() == '/usr/bin'

# === __fspath__ method (os.PathLike protocol) ===
assert Path('/usr/bin').__fspath__() == '/usr/bin'

# === dot normalization ===
# CPython normalizes '.' components but keeps '..'
assert str(Path('/a/./b')) == '/a/b'
assert str(Path('/a/b/.')) == '/a/b'
assert str(Path('./a')) == 'a'
assert str(Path('.')) == '.'
assert str(Path('/a/b/..')) == '/a/b/..'
assert str(Path('/a/./b/../c')) == '/a/b/../c'
assert str(Path('/a///b')) == '/a/b'

# === Home expansion validates arguments before contacting the host ===
plain = Path('folder/~/file')
assert plain.expanduser() is plain
assert str(Path('').expanduser()) == '.'


def call_class_home(*args, **kwargs):
    return Path.home(*args, **kwargs)


def call_instance_home(*args, **kwargs):
    return Path('folder').home(*args, **kwargs)


def call_expanduser(*args, **kwargs):
    return Path('~').expanduser(*args, **kwargs)


for method, name, receiver in [
    (call_class_home, 'Path.home', 'cls'),
    (call_instance_home, 'Path.home', 'cls'),
    (call_expanduser, 'Path.expanduser', 'self'),
]:
    try:
        method(1)
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == f'{name}() takes 1 positional argument but 2 were given'
    try:
        method(**{receiver: None})
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == f"{name}() got multiple values for argument '{receiver}'"
    try:
        method(unexpected=1)
        assert False, 'expected TypeError'
    except TypeError as exc:
        assert str(exc) == f"{name}() got an unexpected keyword argument 'unexpected'"

# === repr ===
r = repr(Path('/usr/bin'))
assert r == "PosixPath('/usr/bin')", f'repr should be PosixPath, got {r}'
