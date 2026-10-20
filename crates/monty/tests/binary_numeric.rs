use monty::MontyRun;
use monty_types::CompileOptions;

#[test]
fn numeric_fast_paths_and_fallbacks() {
    let code = r"
def check(a, b, add, sub, mul):
    assert a + b == add
    assert a - b == sub
    assert a * b == mul
    assert type(a + b) == type(add)
    assert type(a - b) == type(sub)
    assert type(a * b) == type(mul)

check(7, 2, 9, 5, 14)
check(7.0, 2.0, 9.0, 5.0, 14.0)
check(7, 2.0, 9.0, 5.0, 14.0)
check(7.0, 2, 9.0, 5.0, 14.0)
check(-7, 2, -5, -9, -14)
check(True, 2, 3, -1, 2)
check(7.0, True, 8.0, 6.0, 7.0)
check(9223372036854775807, 2, 9223372036854775809, 9223372036854775805, 18446744073709551614)
check(-9223372036854775808, -1, -9223372036854775809, -9223372036854775807, 9223372036854775808)

big = 1 << 100
check(big, 2, big + 2, big - 2, big * 2)
check(2, big, big + 2, 2 - big, big * 2)
check(big, big, 1 << 101, 0, 1 << 200)
check(big, 2.0, float(big), float(big), float(1 << 101))
check(2.0, big, float(big), -float(big), float(1 << 101))

promoted = 9223372036854775807
promoted = promoted + 1
check(promoted, 2, 9223372036854775810, 9223372036854775806, 18446744073709551616)


def fallback(a, b):
    return a + b, a * 2

assert fallback('a', 'b') == ('ab', 'aa')
assert fallback([1], [2]) == ([1, 2], [1, 1])

def signed_zero(a, b):
    import math
    assert math.copysign(1.0, a * b) == -1.0

signed_zero(-0.0, 2)
signed_zero(2, -0.0)
";
    let mut runner = MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap();
    runner.run_no_limits(vec![]).unwrap();
}

#[test]
fn numeric_division_fast_paths_and_fallbacks() {
    let code = r#"
import math

def check(a, b, quotient, floor, remainder):
    assert a / b == quotient
    assert a // b == floor
    assert a % b == remainder
    assert type(a / b) == type(quotient)
    assert type(a // b) == type(floor)
    assert type(a % b) == type(remainder)

for a, b, floor, remainder in [(7, 2, 3, 1), (-7, 2, -4, 1), (7, -2, -4, -1), (-7, -2, 3, -1)]:
    check(a, b, float(a) / float(b), floor, remainder)
    check(float(a), b, float(a) / float(b), float(floor), float(remainder))
    check(a, float(b), float(a) / float(b), float(floor), float(remainder))
    check(float(a), float(b), float(a) / float(b), float(floor), float(remainder))

check(1.0, 0.1, 10.0, 9.0, 0.09999999999999995)
check(-9223372036854775808, -1, 9223372036854775808.0, 9223372036854775808, 0)
check(1 << 100, 2, float(1 << 99), 1 << 99, 0)
check(True, 2, 0.5, 0, 1)

# Converting either operand to float first gives a different rounded quotient.
a = 9007199254740993
b = 3
assert a / b == 3002399751580331.0

def check_zero(a, b):
    for op in ['/', '//', '%']:
        try:
            if op == '/':
                a / b
            elif op == '//':
                a // b
            else:
                a % b
        except ZeroDivisionError:
            pass
        else:
            assert False
    assert 7 + 2 == 9

for a in [7, 7.0, 1 << 100]:
    for b in [0, 0.0, -0.0]:
        check_zero(a, b)

for a, b in [(-0.0, 2), (0, -2.0), (0.0, -2)]:
    assert math.copysign(1.0, a / b) == -1.0
    assert math.copysign(1.0, a // b) == -1.0
for a, b in [(0, -2.0), (0.0, -2), (4.0, -2.0)]:
    assert math.copysign(1.0, a % b) == -1.0

for a, b in [(float('nan'), 2.0), (float('inf'), 2), (float('-inf'), 2.0)]:
    assert math.isnan(a // b)
    assert math.isnan(a % b)
assert math.isinf(float('inf') / 2)
check(2, float('inf'), 0.0, 0.0, 2.0)

def format_mod(a, b):
    return a % b
assert format_mod('%s', 'value') == 'value'
"#;
    let mut runner = MontyRun::new(code.to_owned(), "test.py", vec![], CompileOptions::default()).unwrap();
    runner.run_no_limits(vec![]).unwrap();
}
