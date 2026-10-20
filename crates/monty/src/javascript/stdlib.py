# Compiled once per session into ordinary Monty functions. No host execution.
import asyncio as _js_asyncio
import json as _js_json
import math as _js_math
import sys as _js_sys


def _js_string(value='', *ignored):
    if value is None:
        return 'null'
    if value is True:
        return 'true'
    if value is False:
        return 'false'
    if isinstance(value, str):
        return value
    if isinstance(value, (list, tuple)):
        return ','.join('' if x is None else _js_string(x) for x in value)
    if isinstance(value, dict):
        return '[object Object]'
    if isinstance(value, float) and (_js_math.isfinite(value) and value == int(value)):
        return str(int(value))
    return str(value)


def _js_number(value=0, *ignored):
    if value is None:
        return 0
    if isinstance(value, (list, tuple, dict)):
        value = _js_string(value)
    if isinstance(value, str):
        value = value.strip()
        if not value:
            return 0
        try:
            if value.lower().startswith(('0x', '0o', '0b')):
                return int(value, 0)
            number = float(value)
            return int(number) if _js_math.isfinite(number) and number == int(number) else number
        except ValueError:
            return float('nan')
    return value + 0


def _js_add(a, b):
    if isinstance(a, (list, tuple, dict)):
        a = _js_string(a)
    if isinstance(b, (list, tuple, dict)):
        b = _js_string(b)
    if isinstance(a, str) or isinstance(b, str):
        return _js_string(a) + _js_string(b)
    return _js_number(a) + _js_number(b)


def _js_truth(value=None, *ignored):
    if isinstance(value, (list, tuple, dict)):
        return True
    return bool(value) and value == value


def _js_get(obj, key):
    if obj is None:
        raise TypeError('Cannot read properties of null')
    if isinstance(obj, Exception):
        if key == 'message':
            return str(obj)
        if key == 'name':
            return type(obj).__name__
    if obj is _js_number:
        if key == 'isNaN':
            return lambda x: isinstance(x, float) and x != x
        if key == 'isFinite':
            return lambda x: type(x) in (int, float) and _js_math.isfinite(x)
        if key == 'isInteger':
            return lambda x: type(x) is int or (type(x) is float and _js_math.isfinite(x) and x == int(x))
    if isinstance(obj, (_js_Set, _js_Map)):
        if key == 'size':
            return len(obj.items)
        if key in ('has', 'get', 'set', 'add', 'delete', 'clear', 'keys', 'values', 'entries'):
            return getattr(obj, key, None)
        return None
    if isinstance(obj, dict):
        return obj.get(_js_string(key))
    if key == 'length' and isinstance(obj, (list, tuple, str)):
        return len(obj)
    if isinstance(obj, (list, tuple, str)):
        if isinstance(key, str) and 0 < len(key) <= 10 and not key.lstrip('0123456789'):
            index = int(key)
            if str(index) == key:
                key = index
        if type(key) in (int, float):
            if not _js_math.isfinite(key):
                return None
            i = int(key)
            return obj[i] if i == key and 0 <= i < len(obj) else None
    return getattr(obj, _js_string(key), None)


def _js_set(obj, key, value):
    if isinstance(obj, dict):
        obj[_js_string(key)] = value
    elif isinstance(obj, list):
        if key == 'length':
            length = int(value)
            if length != value or length < 0 or length >= 4294967296:
                raise ValueError('Invalid array length')
            if length < len(obj):
                obj[length:] = []
            else:
                obj.extend([None] * (length - len(obj)))
        else:
            index = int(key)
            if index < 0 or index >= 4294967295 or _js_string(index) != _js_string(key):
                raise TypeError('unsupported array property assignment')
            if index >= len(obj):
                obj.extend([None] * (index + 1 - len(obj)))
            obj[index] = value
    else:
        if obj is None:
            raise TypeError('Cannot set properties of null')
        setattr(obj, _js_string(key), value)
    return value


def _js_invoke(fn, *args):
    if fn is _js_string or fn is _js_number or fn is _js_truth or fn is _js_json.loads:
        return fn(args[0])
    return fn(*args)


def _js_method(obj, key, *args):
    if obj is _js_number:
        return _js_get(obj, key)(*args)
    if isinstance(obj, (_js_Set, _js_Map)) and key in (
        'has',
        'get',
        'set',
        'add',
        'delete',
        'clear',
        'keys',
        'values',
        'entries',
    ):
        return getattr(obj, key)(*args)
    if isinstance(obj, dict):
        return obj[key](*args)
    if isinstance(obj, set):
        if key == 'has':
            return args[0] in obj
        if key == 'add':
            obj.add(args[0])
            return obj
        if key == 'delete':
            present = args[0] in obj
            obj.discard(args[0])
            return present
        if key in ('values', 'keys'):
            return list(obj)
        if key == 'clear':
            obj.clear()
            return None
    if key == 'toString':
        return _js_string(obj)
    if key == 'toFixed' and type(obj) in (int, float):
        digits = int(args[0]) if args else 0
        if digits < 0 or digits > 100:
            raise ValueError('toFixed digits must be between 0 and 100')
        return format(obj, '.' + str(digits) + 'f')
    if isinstance(obj, (list, tuple)):
        if key in ('map', 'flatMap', 'filter', 'find', 'findIndex', 'some', 'every', 'forEach'):
            fn = args[0]
            result = []
            for i in range(len(obj)):
                item = _js_get(obj, i)
                value = _js_invoke(fn, item, i, obj)
                if key == 'map':
                    result.append(value)
                elif key == 'flatMap':
                    result.extend(value if isinstance(value, list) else [value])
                elif key == 'filter' and _js_truth(value):
                    result.append(item)
                elif key == 'find' and _js_truth(value):
                    return item
                elif key == 'findIndex' and _js_truth(value):
                    return i
                elif key == 'some' and _js_truth(value):
                    return True
                elif key == 'every' and not _js_truth(value):
                    return False
            if key in ('map', 'flatMap', 'filter'):
                return result
            if key == 'findIndex':
                return -1
            if key == 'some':
                return False
            if key == 'every':
                return True
            return None
        if key == 'reduce':
            fn = args[0]
            if len(args) > 1:
                value, start = args[1], 0
            elif obj:
                value, start = obj[0], 1
            else:
                raise TypeError('Reduce of empty array with no initial value')
            for i in range(start, len(obj)):
                value = _js_invoke(fn, value, _js_get(obj, i), i, obj)
            return value
        if key == 'push':
            obj.extend(args)
            return len(obj)
        if key == 'pop':
            return obj.pop() if obj else None
        if key == 'shift':
            return obj.pop(0) if obj else None
        if key == 'unshift':
            obj[0:0] = list(args)
            return len(obj)
        if key == 'concat':
            result = list(obj)
            for value in args:
                result.extend(value if isinstance(value, (list, tuple)) else [value])
            return result
        if key == 'join':
            separator = _js_string(args[0]) if args else ','
            return separator.join('' if v is None else _js_string(v) for v in obj)
        if key in ('includes', 'indexOf'):
            start = int(args[1]) if len(args) > 1 else 0
            start = max(len(obj) + start, 0) if start < 0 else start
            for i in range(start, len(obj)):
                if _js_same(obj[i], args[0]) if key == 'includes' else _js_eq(obj[i], args[0]):
                    return True if key == 'includes' else i
            return False if key == 'includes' else -1
        if key == 'reverse':
            obj.reverse()
            return obj
        if key == 'sort':
            if not args:
                obj.sort(key=_js_string)
            else:
                # Insertion sort keeps the comparator in interpreted, suspendable frames.
                fn = args[0]
                for i in range(1, len(obj)):
                    j = i
                    while j > 0 and fn(obj[j - 1], obj[j]) > 0:
                        obj[j - 1], obj[j] = obj[j], obj[j - 1]
                        j -= 1
            return obj
        if key == 'flat':
            depth = int(args[0]) if args else 1
            result = list(obj)
            for _ in range(max(depth, 0)):
                result = [y for x in result for y in (x if isinstance(x, list) else [x])]
            return result
    if key == 'at' and isinstance(obj, (list, tuple, str)):
        index = int(args[0]) if args else 0
        index = len(obj) + index if index < 0 else index
        return obj[index] if 0 <= index < len(obj) else None
    if key == 'slice' and isinstance(obj, (list, tuple, str)):
        start = int(args[0]) if args and args[0] is not None else 0
        end = int(args[1]) if len(args) > 1 and args[1] is not None else len(obj)
        return obj[start:end]
    if isinstance(obj, str):
        if key in ('trim', 'trimStart', 'trimEnd', 'toLowerCase', 'toUpperCase'):
            if key == 'trim':
                return obj.strip()
            if key == 'trimStart':
                return obj.lstrip()
            if key == 'trimEnd':
                return obj.rstrip()
            return obj.lower() if key == 'toLowerCase' else obj.upper()
        if key == 'split':
            if not args or args[0] is None:
                return [obj]
            delimiter = _js_string(args[0])
            parts = obj.split(delimiter) if delimiter else list(obj)
            return parts[: int(args[1])] if len(args) > 1 else parts
        if key in ('includes', 'startsWith', 'endsWith', 'indexOf'):
            value = _js_string(args[0])
            start = int(args[1]) if len(args) > 1 else 0
            if key == 'includes':
                return value in obj[max(start, 0) :]
            if key == 'startsWith':
                return obj.startswith(value, max(start, 0))
            if key == 'endsWith':
                return obj[: start if len(args) > 1 else len(obj)].endswith(value)
            return obj.find(value, max(start, 0))
        if key in ('replace', 'replaceAll'):
            return obj.replace(_js_string(args[0]), _js_string(args[1]), 1 if key == 'replace' else -1)
    fn = _js_get(obj, key)
    if callable(fn):
        return fn(*args)
    raise TypeError('unsupported JavaScript method: ' + str(key))


def _js_json_value(value):
    if isinstance(value, float):
        if not _js_math.isfinite(value):
            return None
        if _js_math.isfinite(value) and value == int(value):
            return int(value)
    if isinstance(value, (_js_Set, _js_Map)):
        return {}
    if isinstance(value, (list, tuple)):
        return [_js_json_value(x) for x in value]
    if isinstance(value, dict):
        return {k: _js_json_value(v) for k, v in value.items()}
    return value


def _js_stringify(value, replacer=None, space=None):
    if replacer is not None:
        raise TypeError('JSON replacers are not supported')
    indent = min(max(int(space), 0), 10) if isinstance(space, (int, float)) else space
    return _js_json.dumps(
        _js_json_value(value), ensure_ascii=False, separators=(',', ':') if not indent else (',', ': '), indent=indent
    )


def _js_typeof(value):
    if value is None:
        return 'object'
    if isinstance(value, bool):
        return 'boolean'
    if isinstance(value, (int, float)):
        return 'number'
    if isinstance(value, str):
        return 'string'
    return 'function' if callable(value) else 'object'


def _js_array(*args):
    if len(args) == 1 and type(args[0]) in (int, float):
        length = args[0]
        if length < 0 or length != int(length):
            raise ValueError('Invalid array length')
        return [None] * int(length)
    return list(args)


def _js_array_from(value, mapper=None):
    result = list(value)
    return [_js_invoke(mapper, x, i, result) for i, x in enumerate(result)] if mapper is not None else result


def _js_rest(obj, keys):
    return {k: v for k, v in obj.items() if k not in keys}


def _js_assign(target, *sources):
    for source in sources:
        if source is not None:
            target.update(_js_entries(source))
    return target


def _js_parse_int(value=None, radix=0, *ignored):
    value = _js_string(value).lstrip()
    sign = -1 if value.startswith('-') else 1
    value = value.lstrip('+-')
    radix = int(radix) if radix else (16 if value.lower().startswith('0x') else 10)
    if radix < 2 or radix > 36:
        return float('nan')
    if radix == 16 and value.lower().startswith('0x'):
        value = value[2:]
    digits = ''
    for ch in value.lower():
        if ch not in '0123456789abcdefghijklmnopqrstuvwxyz'[:radix]:
            break
        digits += ch
    return sign * int(digits, radix) if digits else float('nan')


def _js_eq(a, b):
    # The frontend replaces this body with the shared strict-equality opcode.
    raise NotImplementedError('compiler intrinsic')


def _js_same(a, b):
    return _js_eq(a, b) or (type(a) is float and type(b) is float and a != a and b != b)


class _js_Set:
    def __init__(self, values=None):
        self.items = []
        for value in values if values is not None else []:
            self.add(value)

    def __iter__(self):
        return iter(self.items)

    def has(self, value):
        return any(_js_same(x, value) for x in self.items)

    def add(self, value):
        if not self.has(value):
            self.items.append(value)
        return self

    def delete(self, value):
        for i, item in enumerate(self.items):
            if _js_same(item, value):
                self.items.pop(i)
                return True
        return False

    def clear(self):
        self.items.clear()

    def values(self):
        return list(self.items)

    def keys(self):
        return list(self.items)

    def entries(self):
        return [[x, x] for x in self.items]


class _js_Map:
    def __init__(self, entries=None):
        self.items = []
        for key, value in entries if entries is not None else []:
            self.set(key, value)

    def __iter__(self):
        return iter(self.entries())

    def has(self, key):
        return any(_js_same(k, key) for k, _ in self.items)

    def get(self, key):
        for k, value in self.items:
            if _js_same(k, key):
                return value
        return None

    def set(self, key, value):
        for item in self.items:
            if _js_same(item[0], key):
                item[1] = value
                return self
        self.items.append([key, value])
        return self

    def delete(self, key):
        for i, item in enumerate(self.items):
            if _js_same(item[0], key):
                self.items.pop(i)
                return True
        return False

    def clear(self):
        self.items.clear()

    def keys(self):
        return [k for k, _ in self.items]

    def values(self):
        return [v for _, v in self.items]

    def entries(self):
        return [list(item) for item in self.items]


def _js_entries(obj):
    if isinstance(obj, dict):
        return [list(pair) for pair in obj.items()]
    if isinstance(obj, (list, tuple, str)):
        return [[str(i), value] for i, value in enumerate(obj)]
    return []


def parseFloat(value=None, *ignored):
    text = _js_string(value).lstrip()
    i = 1 if text[:1] in ('+', '-') else 0
    if text[i:].startswith('Infinity'):
        return -float('inf') if text.startswith('-') else float('inf')
    first = i
    while i < len(text) and '0' <= text[i] <= '9':
        i += 1
    digits = i - first
    if i < len(text) and text[i] == '.':
        i += 1
        first = i
        while i < len(text) and '0' <= text[i] <= '9':
            i += 1
        digits += i - first
    if not digits:
        return float('nan')
    if i < len(text) and text[i] in 'eE':
        exponent = i
        i += 1
        if i < len(text) and text[i] in '+-':
            i += 1
        first = i
        while i < len(text) and '0' <= text[i] <= '9':
            i += 1
        if i == first:
            i = exponent
    return float(text[:i])


def isNaN(value=None, *ignored):
    number = _js_number(value)
    return number != number


def isFinite(value=None, *ignored):
    return _js_math.isfinite(_js_number(value))


Object = {
    'keys': lambda o: [k for k, _ in _js_entries(o)],
    'values': lambda o: [v for _, v in _js_entries(o)],
    'entries': _js_entries,
    'fromEntries': lambda xs: {_js_string(k): v for k, v in xs},
    'assign': _js_assign,
    'hasOwn': lambda o, k: any(_js_string(k) == key for key, _ in _js_entries(o)),
}
JSON = {'parse': _js_json.loads, 'stringify': _js_stringify}
Math = {
    'floor': _js_math.floor,
    'ceil': _js_math.ceil,
    'round': lambda x: _js_math.floor(x + 0.5),
    'abs': abs,
    'min': lambda *xs: min(xs) if xs else float('inf'),
    'max': lambda *xs: max(xs) if xs else -float('inf'),
    'pow': pow,
    'sqrt': _js_math.sqrt,
    'trunc': _js_math.trunc,
    'PI': _js_math.pi,
    'E': _js_math.e,
}
Array = {'isArray': lambda x: isinstance(x, list), 'from': _js_array_from}


def _js_error(*values):
    print(*values, file=_js_sys.stderr)


console = {'log': print, 'warn': _js_error, 'error': _js_error}
String = _js_string
Number = _js_number
Boolean = _js_truth
parseInt = _js_parse_int

Set = _js_Set
Map = _js_Map
Error = Exception
RangeError = ValueError


async def _js_resolve(value=None):
    return value


async def _js_reject(value):
    raise value if isinstance(value, Exception) else Exception(_js_string(value))


Promise = {'all': lambda values: _js_asyncio.gather(*values), 'resolve': _js_resolve, 'reject': _js_reject}
