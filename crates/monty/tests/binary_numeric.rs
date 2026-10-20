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
