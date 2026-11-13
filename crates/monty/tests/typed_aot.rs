#![cfg(all(
    feature = "experimental-aot",
    not(feature = "experimental-fusion"),
    any(target_arch = "aarch64", target_arch = "x86_64")
))]

use monty::{MontyRun, experimental_aot};
use monty_types::{CompileOptions, MontyObject};

fn run(source: &str, enabled: bool) -> Result<MontyObject, String> {
    experimental_aot::set_enabled(enabled);
    MontyRun::new(source.to_owned(), "aot.py", vec![], CompileOptions::default())
        .unwrap()
        .run_no_limits(vec![])
        .map_err(|error| error.to_string())
}

fn compare(source: &str, native_return: bool) {
    let expected = run(source, false);
    for backend in [experimental_aot::Backend::Copied, experimental_aot::Backend::Shared] {
        experimental_aot::set_backend(backend);
        experimental_aot::reset_stats();
        assert_eq!(run(source, true), expected, "{source}; {backend:?}");
        if native_return {
            assert!(
                experimental_aot::stats().returns > 0,
                "no optimized return: {source}; {backend:?}; {:?}",
                experimental_aot::stats()
            );
            if backend == experimental_aot::Backend::Shared {
                assert!(experimental_aot::stats().shared_compilations > 0);
                assert_eq!(experimental_aot::stats().code_bytes, 0);
            }
        }
    }
}

#[test]
fn scalar_functions_and_loops() {
    for update in ["i + 3", "i * 7", "i // 3", "i % -3", "i ^ 12", "-i", "i & 7", "i | 8"] {
        compare(
            &format!(
                "def calculate(n: int) -> int:\n    i = -20\n    total = 0\n    while i < n:\n        if i % 2 == 0:\n            total += {update}\n        else:\n            total -= {update}\n        i += 1\n    return total\ncalculate(200)"
            ),
            true,
        );
    }
    compare("def f(x: int, y: int) -> int:\n    return x + y * 3\nf(4, 9)", true);
    compare("def f(x: int) -> int:\n    return x + (x := 2)\nf(7)", true);
    compare(
        "def f(x: int, y: int) -> int:\n    tmp = x\n    x = y\n    y = tmp\n    return x * 10 + y\nf(2, 3)",
        true,
    );
    compare(
        "def f(x: int) -> int:\n    while x < 100:\n        x += 1\n    return x\nf(0)",
        true,
    );
}

#[test]
fn runtime_guards_and_errors() {
    for source in [
        "def f(x: int) -> int:\n    return x + 1\nf(9223372036854775807)",
        "def f(x: int) -> int:\n    return -x\nf(-9223372036854775807 - 1)",
        "def f(x: int) -> int:\n    return x // -1\nf(-9223372036854775807 - 1)",
        "def f(x: int) -> int:\n    return x // 0\nf(12)",
        "def f(x: int) -> int:\n    return x % 0\nf(12)",
        "def f(x: int) -> int:\n    return x + 1\nf(0.5)",
        "def f(x: int) -> int:\n    return x + 1\nf(True)",
        "def f(x: int) -> int:\n    return x + 1\nf(999999999999999999999)",
        "def f(x: int) -> int:\n    return x + 1\nf('hello')",
        "def f(n: int) -> int:\n    if n:\n        x = 1\n    x\n    return 3\nf(0)",
    ] {
        compare(source, false);
    }
}

#[test]
fn floating_point_and_boolean_semantics() {
    for expression in [
        "x + y", "x - y", "x * y", "x / y", "x == y", "x != y", "x < y", "not x", "-x",
    ] {
        compare(
            &format!("def f(x: float, y: float):\n    return {expression}\nf(3.25, -2.5)"),
            true,
        );
    }
    for expression in ["x + y", "x & y", "x | y", "x ^ y", "~x", "not x", "x == y"] {
        compare(
            &format!("def f(x: bool, y: bool):\n    return {expression}\nf(True, False)"),
            true,
        );
    }
    for expression in ["x == y", "x < y", "x + y", "x / y"] {
        compare(
            &format!("def f(x: int, y: float):\n    return {expression}\nf(9007199254740993, 9007199254740992.0)"),
            false,
        );
    }
    compare("def f(x: int,y: int):\n    return x / y\nf(9007199254740993,3)", false);
    compare("def f(x: float,y: float):\n    return x / y\nf(2.0,0.0)", false);
}

#[test]
fn short_circuit_and_unbound_locals() {
    for source in [
        "def f(x: int,y: int):\n    return x or y\nf(0,9)",
        "def f(x: int,y: int):\n    return x and y\nf(5,9)",
        "def f(x: int):\n    if x > 0:\n        y = 3\n    else:\n        y = 5\n    return y * 2\nf(-1)",
        "def f(x: int):\n    a = 2\n    b = 3\n    while x > 0:\n        tmp = a\n        a = b\n        b = tmp\n        x -= 1\n    return a * 10 + b\nf(5)",
    ] {
        compare(source, true);
    }
    compare(
        "def f(x: int):\n    if x:\n        y = 2\n    y\n    return 3\nf(0)",
        false,
    );
}

#[test]
fn virtual_range_loops() {
    for range in ["range(n)", "range(-10,n,3)", "range(n,-10,-3)", "range(0)"] {
        compare(
            &format!(
                "def f(n: int):\n    total = 0\n    for i in {range}:\n        total += i * 3\n    return total\nf(500)"
            ),
            true,
        );
    }
    compare(
        "def f(n: int):\n    total = 0\n    for i in range(n):\n        for j in range(i):\n            total += i + j\n    return total\nf(30)",
        true,
    );
    compare(
        "def f(n: int):\n    total = 0\n    for i in range(n):\n        if i % 2 == 0:\n            continue\n        if i > 20:\n            break\n        total += i\n    return total\nf(500)",
        true,
    );
    compare(
        "def f(n: int):\n    for i in range(0,n,0):\n        pass\n    return 3\nf(5)",
        false,
    );
}

#[test]
fn builtin_shadowing_preserves_callbacks() {
    compare(
        "def f(n: int):\n    total = 0\n    for i in range(n):\n        total += i\n    return total\na = f(10)\ndef replacement(n):\n    return [7, 9]\nrange = replacement\nb = f(10)\n(a,b)",
        true,
    );
    assert!(experimental_aot::stats().fallbacks > 0);
}

#[test]
fn suspension_round_trip_rebuilds_typed_helpers() {
    use monty::{FunctionCall, RunProgress};
    use monty_types::{PrintWriter, ResourceTracker};
    experimental_aot::set_enabled(true);
    experimental_aot::reset_stats();
    let runner=MontyRun::new(
        "def calculate(n: int) -> int:\n    total = 0\n    for i in range(n):\n        total += i\n    return total\ndef work(n: int) -> int:\n    x = calculate(n)\n    y = external(x)\n    return calculate(y)\nwork(10)".to_owned(),
        "suspend_aot.py",vec!["external".to_owned()],CompileOptions::default()).unwrap();
    let RunProgress::FunctionCall(call) = runner
        .start(
            vec![MontyObject::function("external".to_owned(), None)],
            ResourceTracker::default(),
            PrintWriter::Disabled,
        )
        .unwrap()
    else {
        panic!("expected suspension")
    };
    assert_eq!(call.args.arg(0).unwrap(), MontyObject::int(45));
    assert_eq!(experimental_aot::stats().returns, 1);
    let bytes = serde_json::to_vec(&call).unwrap();
    let RunProgress::Complete(expected) = call.resume(MontyObject::int(20), PrintWriter::Disabled).unwrap() else {
        panic!("expected completion")
    };
    experimental_aot::reset_stats();
    let restored: FunctionCall = serde_json::from_slice(&bytes).unwrap();
    let RunProgress::Complete(result) = restored.resume(MontyObject::int(20), PrintWriter::Disabled).unwrap() else {
        panic!("expected restored completion")
    };
    assert_eq!(result, expected);
    assert_eq!(result, MontyObject::int(190));
    assert!(experimental_aot::stats().compilations > 0);
    assert_eq!(experimental_aot::stats().returns, 1);
}

#[test]
fn infinite_typed_loop_polls_deadlines() {
    use std::time::Duration;

    use monty_types::{PrintWriter, ResourceLimits, ResourceTracker};
    experimental_aot::set_enabled(true);
    experimental_aot::reset_stats();
    let mut runner = MontyRun::new(
        "def f(n: int):\n    while True:\n        n = n ^ 1\nf(0)".to_owned(),
        "deadline_aot.py",
        vec![],
        CompileOptions::default(),
    )
    .unwrap();
    let error = runner
        .run(
            vec![],
            ResourceTracker::new(ResourceLimits::default().max_turn_duration(Duration::from_millis(10))),
            PrintWriter::Disabled,
        )
        .unwrap_err();
    assert!(error.to_string().contains("time limit"), "{error}");
    assert!(experimental_aot::stats().instructions > 0);
}

#[test]
fn typed_sequences_and_unpacking() {
    for annotation in ["list[int]", "tuple[int, ...]", "list[int] | tuple[int, ...]"] {
        let argument = if annotation.starts_with("tuple") {
            "(2,3,5,7)"
        } else {
            "[2,3,5,7]"
        };
        compare(
            &format!(
                "def f(data: {annotation}, n: int) -> int:\n    total = 0\n    for i in range(n):\n        total += data[i % len(data)]\n    return total\nf({argument},1000)"
            ),
            true,
        );
    }
    compare(
        "def f(data: list[float], n: int) -> float:\n    total = 0.0\n    for i in range(n):\n        total += data[i % 3 - 3]\n    return total\nf([0.5,1.25,-3.0],100)",
        true,
    );
    compare(
        "def f(data: tuple[int, ...], n: int) -> int:\n    total = 0\n    range_it = range(n)\n    for i in range_it:\n        a,b,c = data\n        total += a * 100 + b * 10 + c\n    return total\nf((1,2,3),100)",
        true,
    );
    compare(
        "def f(data: list[int], n: int) -> int:\n    other = data\n    total = 0\n    for i in range(n):\n        total += other[i % 3]\n    return total\nf([1,2,3],100)",
        true,
    );
}

#[test]
fn sequence_mismatches_and_python_callbacks() {
    for argument in [
        "[1,2]",
        "[1,True,3]",
        "[1,'wrong',3]",
        "[1,999999999999999999999,3]",
        "[1,2,3,4]",
    ] {
        compare(
            &format!(
                "def f(data: list[int], n: int):\n    total = 0\n    for i in range(n):\n        a,b,c = data\n        total += a + b + c\n    return total\nf({argument},5)"
            ),
            false,
        );
    }
    compare(
        "def f(data: list[int], n: int):\n    total = 0\n    for i in range(n):\n        total += data[i]\n    return total\nf([1,2,3],10)",
        false,
    );
    compare(
        "events = []\nclass Custom:\n    def __getitem__(self,i):\n        events.append(i)\n        return i * 2\ndef f(data: list[int], n: int):\n    total = 0\n    for i in range(n):\n        total += data[i]\n    return total\nresult = f(Custom(),5)\n(result,events)",
        false,
    );
}

#[test]
fn typed_dictionary_reads() {
    compare(
        "def f(row: dict[str,int],key: str,n: int) -> int:\n    total = 0\n    for _ in range(n):\n        total += row[key]\n    return total\nkey = 'long-key-' * 4096\nf({key: 7},key,1000)",
        true,
    );
    compare(
        "def f(row: dict[str,int], key: str, n: int) -> int:\n    total = 0\n    for i in range(n):\n        total += row[key] + row['tax']\n    return total + len(row)\nf({'price':7,'tax':2}, ''.join(['pr','ice']),1000)",
        true,
    );
    for argument in ["{}", "{'price':'wrong'}", "{'price':999999999999999999999}"] {
        compare(
            &format!(
                "def f(row: dict[str,int],n: int):\n    total = 0\n    for i in range(n):\n        total += row['price']\n    return total\nf({argument},5)"
            ),
            false,
        );
    }
    compare(
        "events = []\nclass Key:\n    def __hash__(self):\n        return hash('price')\n    def __eq__(self,other):\n        events.append(other)\n        return True\ndef f(row: dict[str,int],n: int):\n    total = 0\n    for i in range(n):\n        total += row['price']\n    return total\nresult = f({Key():7},5)\n(result,events)",
        false,
    );
}

#[test]
fn immutable_range_aliases_are_values() {
    compare(
        "def f(n: int):\n    total = 0\n    old = range(3)\n    for i in range(n):\n        previous = old\n        old = range(i)\n        for j in previous:\n            total += j\n    return total\nf(10)",
        true,
    );
}

#[test]
fn maybe_bound_locals_and_scalar_tuple_returns() {
    for n in [0, 1, 10, -1] {
        compare(
            &format!("def f(n: int):\n    for i in range(n):\n        last = i * 3\n    return last\nf({n})"),
            n > 0,
        );
    }
    for value in ["True", "None", "1.25"] {
        compare(
            &format!("def f(n: int):\n    if n > 0:\n        value = {value}\n    return value\nf(1)"),
            true,
        );
        compare(
            &format!("def f(n: int):\n    if n > 0:\n        value = {value}\n    return value\nf(0)"),
            false,
        );
    }
    for expression in ["(x,x+1,x*3)", "()"] {
        compare(&format!("def f(x: int):\n    return {expression}\nf(7)"), true);
    }
    compare("def f(x: float):\n    return (x,-x,x*3.0)\nf(1.25)", true);
    compare("def f(x: bool):\n    return (x,not x,x and False)\nf(True)", false);
    compare("def f(x: bool):\n    return (x,not x)\nf(True)", true);
    let assignments = "        a,b,c,d,e,f,g,h,i,j = data\n".repeat(400);
    compare(
        &format!(
            "def unpack(data: tuple[int,...] | list[int], n: int):\n    iterations = range(n)\n    for _ in iterations:\n{assignments}    return (a,b,c,d,e,f,g,h,i,j)\nunpack(tuple(range(10)),2)"
        ),
        true,
    );
    compare(
        &format!(
            "def unpack(data: tuple[int,...] | list[int], n: int):\n    iterations = range(n)\n    for _ in iterations:\n{assignments}    return (a,b,c,d,e,f,g,h,i,j)\nunpack(tuple(range(10)),0)"
        ),
        false,
    );
}

#[test]
fn repeated_native_calls_keep_dispatch_batches_bounded() {
    compare(
        "def leaf(n: int) -> int:\n    i = 0\n    while i < n:\n        i += 1\n    return i\ndef repeat():\n    total = 0\n    for i in range(10_000):\n        total += leaf(100)\n    return total\nrepeat()",
        true,
    );
    assert_eq!(experimental_aot::stats().returns, 10_000);
}

#[test]
fn large_sparse_sequences_validate_only_the_accessed_values() {
    compare(
        "def f(data: list[int],n: int):\n    total = 0\n    for i in range(n):\n        total += data[i]\n    return total\ndata = list(range(10_000))\ndata[-1] = 'unused'\nf(data,100)",
        true,
    );
    compare(
        "def f(data: list[int],n: int):\n    total = 0\n    for i in range(n):\n        total += data[-i - 1]\n    return total\nf(list(range(10_000)),100)",
        true,
    );
    compare(
        "def f(data: list[int],n: int):\n    total = 0\n    for i in range(n):\n        total += data[i]\n    return total\ndata = list(range(10_000))\ndata[50] = 'wrong'\nf(data,100)",
        false,
    );
}

#[test]
fn scalar_calls_preserve_binding_recursion_and_errors() {
    compare(
        "def leaf(x: int,y: int) -> int:\n    return (x + y) * 3\ntotal = 0\nfor i in range(20_000):\n    total += leaf(i,7)\ntotal",
        true,
    );
    assert_eq!(experimental_aot::stats().fast_calls, 20_000);
    assert_eq!(experimental_aot::stats().instructions, 100_000);
    for source in [
        "def f(x: int,y: int = 2) -> int:\n    return x + y\nf(7)",
        "def f(x: int,y: int) -> int:\n    return x + y\nf(y=2,x=7)",
        "def f(x: int,*args) -> int:\n    return x + 1\nf(7,8)",
        "def f(x: int,y: int) -> int:\n    return x // y\nf(7,0)",
        "def f(x: int) -> int:\n    return x + 1\ndef outer():\n    return f('wrong')\nouter()",
    ] {
        compare(source, false);
    }

    let source = "def leaf(x: int) -> int:\n    return x + 1\ndef caller(n):\n    if n:\n        return caller(n-1)\n    return leaf(7)\ncaller(3)";
    for depth in 1..8 {
        let execute = |enabled| {
            experimental_aot::set_enabled(enabled);
            MontyRun::new(source.to_owned(), "recursion.py", vec![], CompileOptions::default())
                .unwrap()
                .run(
                    vec![],
                    monty_types::ResourceTracker::new(
                        monty_types::ResourceLimits::default().max_recursion_depth(depth),
                    ),
                    monty_types::PrintWriter::Stdout,
                )
                .map_err(|error| error.to_string())
        };
        assert_eq!(execute(true), execute(false), "depth {depth}");
    }
}

#[test]
fn dead_edge_locals_preserve_discarded_errors() {
    for source in [
        "def f(data: list[int]):\n    total = 0\n    for value in data:\n        temporary = value * 3\n        if temporary > 5:\n            excess = temporary - 5\n            total += excess\n        else:\n            total -= temporary\n    return total\nf([1,2,3,4])",
        "def f(n: int):\n    for i in range(n):\n        ignored = i + 7\n    return i\nf(3)",
        "def f(flag: bool):\n    if flag:\n        value = 7\n    value\n    return 3\nf(True)",
    ] {
        compare(source, true);
    }
    for source in [
        "def f(flag: bool):\n    if flag:\n        value = 7\n    value\n    return 3\nf(False)",
        "def f(n: int):\n    for i in range(n):\n        ignored = i // 0\n    return 3\nf(2)",
        "def f(n: int):\n    for i in range(n):\n        ignored = i + 7\n    return i\nf(0)",
    ] {
        compare(source, false);
    }
}

#[test]
fn source_instruction_diagnostics_cover_refills_and_misses() {
    for (source, instructions) in [
        ("def f(x: int,y: int) -> int:\n    return (x * y + 3) // 2\nf(17,4)", 7),
        (
            "def f(n: int) -> int:\n    i = 0\n    s = 0\n    while i < n:\n        s += i * 3\n        i += 1\n    return s\nf(20)",
            309,
        ),
        (
            "def f(n: int) -> int:\n    i = 0\n    s = 0\n    while i < n:\n        s += i * 3\n        i += 1\n    return s\nf(1000)",
            15_009,
        ),
        (
            "def f(n: int) -> int:\n    s = 0\n    for i in range(n):\n        s += i * 3\n    return s\nf(1000)",
            9_008,
        ),
    ] {
        compare(source, true);
        assert_eq!(experimental_aot::stats().instructions, instructions, "{source}");
    }
    compare("def f(x: int):\n    return 7 // x\nf(0)", false);
    let stats = experimental_aot::stats();
    assert_eq!(stats.entries, 1);
    assert_eq!(stats.instructions, 3);
}

#[test]
fn private_tuple_assignments_preserve_parallel_values() {
    for source in [
        "def f(n: int) -> int:\n    a,b = 0,1\n    for _ in range(n):\n        a,b = b,a+b\n    return a\nf(30)",
        "def f(x: float,y: float):\n    x,y = y,x+1.0\n    return x-y\nf(1.25,9.5)",
        "def f(x: int):\n    a,b,c = (x,x+1,x+2)\n    return a*100+b*10+c\nf(3)",
    ] {
        compare(source, true);
    }
    compare("def f(x: int):\n    a,b = (x,x+1,x+2)\n    return a+b\nf(3)", false);
    compare("def f(x: int):\n    return (x,x+1)[0]\nf(3)", false);
}

#[test]
fn readonly_sequence_loops_keep_python_iteration_semantics() {
    for source in [
        "def f(data: list[int]) -> int:\n    total = 0\n    for item in data:\n        total += item * item\n    return total\nf(list(range(100)))",
        "def f(data: tuple[float,...]) -> float:\n    total = 0.0\n    for item in data:\n        total += item * item\n    return total\nf((1.25,-2.5,3.75))",
        "def f(data: list[bool]) -> int:\n    total = 0\n    for item in data:\n        if item:\n            total += 1\n    return total\nf([True,False,True])",
        "def f(data: list[None]) -> int:\n    total = 0\n    for item in data:\n        if item is None:\n            total += 1\n    return total\nf([None,None])",
        "def f(data: list[int] | tuple[int,...]) -> int:\n    total = 0\n    for item in data:\n        if item == 5:\n            break\n        if item % 2:\n            continue\n        total += item\n    else:\n        total += 100\n    return total\nf(tuple(range(10))) + f([])",
        "def f(data: list[int]) -> int:\n    total = 0\n    for x in data:\n        for y in data:\n            total += x*y\n    return total\nf([1,2,3])",
        "def f(data: list[int]) -> int:\n    total = 0\n    for x in data:\n        total += x\n    return total\nf(list(range(10_000)))",
    ] {
        compare(source, true);
    }
    for source in [
        "def f(data: list[int]):\n    for item in data:\n        pass\n    return item\nf([])",
        "def f(data: list[int]):\n    total = 0\n    for item in data:\n        total += item\n    return total\nf([1,'wrong',3])",
        "def f(data: list[int]):\n    total = 0\n    for item in data:\n        total += item\n    return total\ndata = list(range(10_000))\ndata[5000] = 'wrong'\nf(data)",
    ] {
        compare(source, false);
    }
}

#[test]
fn generated_scalar_and_control_flow_differential() {
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed.cast_signed()
    };
    for case in 0..80 {
        let x = if case < 8 {
            [i64::MIN, i64::MAX, -1, 0, 1, 2, -2, 255][case]
        } else {
            next()
        };
        let y = if case % 5 == 0 {
            0
        } else if case % 3 == 0 {
            -1
        } else {
            next()
        };
        for expression in ["x+y", "x-y", "x*y", "x//y", "x%y", "x&y", "x|y", "x^y", "x<y", "x>=y"] {
            compare(
                &format!("def f(x: int,y: int):\n    return {expression}\nrepr(f({x},{y}))"),
                false,
            );
        }
    }
    for x in [
        "0.0",
        "-0.0",
        "1.25",
        "-2.5",
        "1e308",
        "1e-308",
        "float('inf')",
        "float('nan')",
    ] {
        for y in ["0.0", "-0.0", "3.0", "1e308", "float('inf')"] {
            for expression in ["x+y", "x-y", "x*y", "x/y", "x==y", "x!=y", "not x"] {
                compare(
                    &format!("def f(x: float,y: float):\n    return {expression}\nrepr(f({x},{y}))"),
                    false,
                );
            }
        }
    }
    for n in 0..12 {
        let source = format!(
            "def f(data: list[int],n: int):\n    a,b = 3,7\n    total = 0\n    for i in range(n):\n        for item in data:\n            if (item+i) % 3 == 0:\n                a,b = b,a+item\n            else:\n                a,b = a-item,b+i\n            total += a-b\n    return (a,b,total)\nf([-8,0,1,3,9],{n})"
        );
        compare(&source, true);
    }
}

#[test]
fn reused_code_accepts_changing_inputs_without_recompilation() {
    let source = "def total(values: list[int],limit: int) -> int:\n    result = 0\n    for value in values:\n        if value < limit:\n            result += value\n    return result\ntotal(data,limit)";
    for backend in [experimental_aot::Backend::Copied, experimental_aot::Backend::Shared] {
        experimental_aot::set_enabled(false);
        let mut reference = MontyRun::new(
            source.to_owned(),
            "inputs.py",
            vec!["data".to_owned(), "limit".to_owned()],
            CompileOptions::default(),
        )
        .unwrap();
        experimental_aot::set_enabled(true);
        experimental_aot::set_backend(backend);
        let mut optimized = MontyRun::new(
            source.to_owned(),
            "inputs.py",
            vec!["data".to_owned(), "limit".to_owned()],
            CompileOptions::default(),
        )
        .unwrap();
        experimental_aot::reset_stats();
        for (length, limit, wrong) in [
            (0, 0, false),
            (1, 2, false),
            (4096, 5000, false),
            (4097, 1000, false),
            (10_000, 2500, false),
            (9, 5, true),
            (4, -1, false),
        ] {
            let mut values = (0..length).map(MontyObject::int).collect::<Vec<_>>();
            if wrong {
                values[2] = MontyObject::float(1.5);
            }
            let inputs = vec![MontyObject::list(values), MontyObject::int(limit)];
            experimental_aot::set_enabled(false);
            let expected = reference
                .run_no_limits(inputs.clone())
                .map_err(|error| error.to_string());
            experimental_aot::set_enabled(true);
            assert_eq!(
                optimized.run_no_limits(inputs).map_err(|error| error.to_string()),
                expected,
                "{length}/{limit}/{wrong}/{backend:?}"
            );
        }
        let stats = experimental_aot::stats();
        assert_eq!(stats.compilations, 1, "{stats:?}");
        assert_eq!(stats.returns, 6, "{stats:?}");
        assert_eq!(stats.fallbacks, 1, "{stats:?}");
    }
}
