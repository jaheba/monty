use monty::MontyRun;
use monty_types::{CompileOptions, MontyObject};

fn run(source: &str, expected: i64) {
    let mut runner = MontyRun::new(source.to_owned(), "fusion.py", vec![], CompileOptions::default()).unwrap();
    assert_eq!(runner.run_no_limits(vec![]).unwrap(), MontyObject::int(expected));
}

#[test]
fn errors_stop_a_fused_sequence_after_handler_transfer() {
    run(
        "def f(a):\n    try:\n        return a + missing\n    except UnboundLocalError:\n        return 42\n    missing = 1\nf(3)",
        42,
    );
    run(
        "def f(a, b):\n    try:\n        return a // b\n    except ZeroDivisionError:\n        return 43\nf(3, 0)",
        43,
    );
    run(
        "def f(a, b):\n    try:\n        return a[b]\n    except IndexError:\n        return 44\nf([], 0)",
        44,
    );
}

#[test]
fn fused_operations_keep_generic_operand_behavior() {
    run("def f(a, b):\n    c = a + b\n    return len(c)\nf([1, 2], [3])", 3);
    run("def f(a, b):\n    c = a + b\n    return len(c)\nf('ab', 'cd')", 4);
    run("def f(a, b):\n    return (a + b) % 97\nf(10 ** 40, 10 ** 40)", 85);
}

#[test]
fn branches_and_local_moves_preserve_values() {
    run(
        "def f(n):\n    a, b, total = 0, 7, 0\n    for i in range(n):\n        if a < b:\n            total += 1\n        if a == b:\n            total += 2\n        a = (a + 1) % 17\n    return total\nf(17)",
        9,
    );
    run(
        "def f(n):\n    a, b, c, d = 1, 2, 3, 4\n    for i in range(n):\n        a, b, c, d = b, c, d, a\n    return a * 1000 + b * 100 + c * 10 + d\nf(3)",
        4123,
    );
}

#[test]
fn integer_fast_paths_fall_back_on_overflow_and_other_types() {
    run("def f(a, b):\n    return (a + b) % 97\nf(9223372036854775807, 1)", 79);
    run("def f(a):\n    return (a + 1) % 97\nf(9223372036854775807)", 79);
    run("def f(a):\n    return (a - 1) % 97\nf(-9223372036854775808)", 17);
    run("def f(a):\n    return (a * 3) % 97\nf(9223372036854775807)", 40);
    run("def f(a):\n    return len(a * 3)\nf('ab')", 6);
    run("def f(a):\n    return int(a + 1)\nf(2.5)", 3);
    run(
        "def f(a):\n    try:\n        return a + 1\n    except TypeError:\n        return 42\ndef g(a, b):\n    try:\n        return a + b\n    except TypeError:\n        return 42\nint(f(True) == g(True, 1))",
        1,
    );
}

#[test]
fn small_integer_modulo_keeps_python_signs_and_zero_errors() {
    run("def f(a):\n    return a % 7\nf(-10)", 4);
    run("def f(a):\n    return a % -7\nf(10)", -4);
    run("def f(a):\n    return a % -7\nf(-10)", -3);
    run("def f(a):\n    return a % -1\nf(-9223372036854775808)", 0);
    run(
        "def f(a):\n    try:\n        return a % 0\n    except ZeroDivisionError:\n        return 42\nf(5)",
        42,
    );
}

#[test]
fn local_add_reports_unbound_operands_before_arithmetic() {
    run(
        "def f(a):\n    try:\n        return a + b\n    except UnboundLocalError:\n        return 42\n    b = 1\nf(5)",
        42,
    );
    run(
        "def f(n):\n    try:\n        return a + b\n    except UnboundLocalError:\n        return 43\n    a, b = 1, 2\nf(5)",
        43,
    );
}
