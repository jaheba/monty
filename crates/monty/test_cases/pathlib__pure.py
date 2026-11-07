# /// monty
# skip-cpython-windows = true  # pathlib uses POSIX paths in Monty's sandbox, Windows CPython resolves differently
# ///
# === Path constructor ===
from pathlib import Path

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

# === POSIX anchor properties ===
for text in ['', '.', '..', 'folder/file', 'C:/folder', 'é/文']:
    path = Path(text)
    assert path.anchor == ''
    assert path.root == ''
    assert path.drive == ''

for text in ['/', '/folder/file', '/./folder', '///folder', '/é/文']:
    path = Path(text)
    assert path.anchor == '/'
    assert path.root == '/'
    assert path.drive == ''

# Dynamically built attribute names also reach the property dispatcher.
for attribute, expected in [('anchor', '/'), ('root', '/'), ('drive', '')]:
    dynamic_name = ''.join(list(attribute))
    assert getattr(Path('/folder'), dynamic_name) == expected

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

# === Lexical relative paths ===
for left, right, expected in [
    ('a/b', 'a', 'b'),
    ('a', 'a', '.'),
    ('a/b', '.', 'a/b'),
    ('a/b', '', 'a/b'),
    ('.', '.', '.'),
    ('/a/b', '/a', 'b'),
    ('/a', '/', 'a'),
    ('/', '/', '.'),
    ('a/../b', 'a', '../b'),
    ('../a', '..', 'a'),
    ('../a/../b', '../a', '../b'),
    ('a/./b', './a/', 'b'),
    ('a/é/文', 'a/é', '文'),
]:
    path = Path(left)
    assert path.is_relative_to(right) is True
    assert path.is_relative_to(other=Path(right)) is True
    result = path.relative_to(right)
    assert isinstance(result, Path)
    assert str(result) == expected
    assert str(path.relative_to(other=Path(right), walk_up=True)) == expected

for left, right, expected in [
    ('.', 'a', '..'),
    ('a', 'a/b', '..'),
    ('a/b', 'a/c', '../b'),
    ('a/b', 'c/d', '../../a/b'),
    ('/a/b', '/ab', '../a/b'),
    ('/a', '/a/b/c', '../..'),
    ('/', '/a/b', '../..'),
    ('../a', '../b', '../a'),
]:
    path = Path(left)
    assert path.is_relative_to(right) is False
    assert str(path.relative_to(right, walk_up=True)) == expected
    try:
        path.relative_to(right)
        assert False, 'expected ValueError'
    except ValueError as e:
        assert str(e) == f'{str(path)!r} is not in the subpath of {str(Path(right))!r}'

for left, right in [('a', '/a'), ('/a', 'a'), ('.', '/'), ('/', '.')]:
    assert Path(left).is_relative_to(right) is False
    try:
        Path(left).relative_to(right, walk_up=True)
        assert False, 'expected ValueError'
    except ValueError as e:
        assert str(e) == f'{left!r} and {right!r} have different anchors'

for left, right in [('a', '..'), ('a/b', 'a/..'), ('a/b', 'a/../c'), ('/a', '../b')]:
    try:
        Path(left).relative_to(right, walk_up=True)
        assert False, 'expected ValueError'
    except ValueError as e:
        assert str(e) == f"'..' segment in {right!r} cannot be walked"

for method in ['relative_to', 'is_relative_to']:
    for operand in [1, b'a', None, ['a']]:
        try:
            if method == 'relative_to':
                Path('a/b').relative_to(operand)
            else:
                Path('a/b').is_relative_to(operand)
            assert False, 'expected TypeError'
        except TypeError as e:
            assert str(e) == (
                'argument should be a str or an os.PathLike object where __fspath__ returns a str, '
                f"not '{type(operand).__name__}'"
            )

# Missing, excess, duplicate and unexpected arguments use the pure-Python signature.
assert str(Path('a/b').relative_to('a', walk_up=NotImplemented)) == 'b'
try:
    Path('a/b').relative_to('c', walk_up=NotImplemented)
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == 'NotImplemented should not be used in a boolean context'

try:
    Path('a').relative_to()
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == "PurePath.relative_to() missing 1 required positional argument: 'other'"
try:
    Path('a').relative_to('a', True)
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == 'PurePath.relative_to() takes 2 positional arguments but 3 were given'
try:
    Path('a').relative_to('a', other='b')
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == "PurePath.relative_to() got multiple values for argument 'other'"
try:
    Path('a').is_relative_to()
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == "PurePath.is_relative_to() missing 1 required positional argument: 'other'"
try:
    Path('a').is_relative_to('a', 'b')
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == 'PurePath.is_relative_to() takes 2 positional arguments but 3 were given'
try:
    Path('a').is_relative_to('a', walk_up=True)
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == "PurePath.is_relative_to() got an unexpected keyword argument 'walk_up'"

# === Path.match ===
for name, pattern, expected in [
    ('a/b.py', '*.py', True),
    ('a/b.py', 'a/*.py', True),
    ('a/b.py', 'b/*.py', False),
    ('a/b.py', '*.txt', False),
    ('a/b.py', '**/*.py', True),
    ('a/b/c.py', 'a/**/*.py', True),
    ('a/b/c/d.py', 'a/**/*.py', False),
    ('a.py', '**/*.py', False),
    ('/a/b.py', 'a/*.py', True),
    ('/a/b.py', '/a/*.py', True),
    ('/a/b.py', '/*.py', False),
    ('/a', '*/*', False),
    ('/', '*', False),
    ('/', '/', True),
    ('.', '*', False),
    ('a', './a/', True),
    ('.hidden', '*', True),
    ('a/.hidden', 'a/*', True),
    ('a/é.py', 'a/?.py', True),
    ('a/文.py', '*.py', True),
    ('a\nb', '*', True),
    ('a3', 'a[0-9]', True),
    ('aZ', 'a[!0-9]', True),
    ('a-', 'a[z-a-]', True),
    ('a[', 'a[', True),
]:
    assert Path(name).match(pattern) is expected
    assert Path(name).match(path_pattern=Path(pattern)) is expected

assert Path('A/B.PY').match('a/*.py') is False
assert Path('A/B.PY').match('a/*.py', case_sensitive=False) is True
assert Path('A/B.PY').match('a/*.py', case_sensitive=True) is False
assert Path('a/é.py').match('A/É.PY', case_sensitive=0) is True
assert Path('a/é.py').match('A/É.PY', case_sensitive=1) is False
for letter in ['İ', 'ı', 'ſ', 'K']:
    assert Path(letter).match('[a-z]', case_sensitive=False) is True

for pattern in ['', '.', Path('')]:
    try:
        Path('a').match(pattern)
        assert False, 'expected ValueError'
    except ValueError as e:
        assert str(e) == 'empty pattern'

try:
    Path('a').match()
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == "PurePath.match() missing 1 required positional argument: 'path_pattern'"
try:
    Path('a').match('a', False)
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == 'PurePath.match() takes 2 positional arguments but 3 were given'
try:
    Path('a').match(pattern='a')
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == "PurePath.match() got an unexpected keyword argument 'pattern'"
try:
    Path('a').match(b'a')
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == ("argument should be a str or an os.PathLike object where __fspath__ returns a str, not 'bytes'")
try:
    Path('a').match('*', case_sensitive=[])
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == "unhashable type: 'list'"
try:
    Path('a').match('*', case_sensitive=NotImplemented)
    assert False, 'expected TypeError'
except TypeError as e:
    assert str(e) == 'NotImplemented should not be used in a boolean context'
assert Path('a').match('a/b', case_sensitive=NotImplemented) is False

# Negated classes can match the root separator even though * and ? cannot consume it.
for pattern in ['[!a]', '[!z-a]', '*[!a]*', '[!a]*', '*[!a]', '/']:
    assert Path('/').match(pattern) is True
for pattern in ['*', '?', '**', '?[!a]', '/[!a]', '/[!a]*', '*/[!a]', '/*', '/?']:
    assert Path('/').match(pattern) is False
