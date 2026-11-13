//! Matched typed-kernel timings, with fresh and retained compilation boundaries.

use std::{
    env,
    hint::black_box,
    time::{Duration, Instant},
};

use monty::{MontyRun, experimental_aot};
use monty_types::CompileOptions;

fn compile(source: &str) -> MontyRun {
    MontyRun::new(source.to_owned(), "aot_bench.py", vec![], CompileOptions::default()).unwrap()
}

fn main() {
    let cases = [
        (
            "tiny",
            "def f(x: int, y: int) -> int:\n    return (x * y + 3) // 2\nf(17,4)",
        ),
        (
            "while_20",
            "def f(n: int) -> int:\n    i = 0\n    s = 0\n    while i < n:\n        s += i * 3\n        i += 1\n    return s\nf(20)",
        ),
        (
            "while_100",
            "def f(n: int) -> int:\n    i = 0\n    s = 0\n    while i < n:\n        s += i * 3\n        i += 1\n    return s\nf(100)",
        ),
        (
            "while_1000",
            "def f(n: int) -> int:\n    i = 0\n    s = 0\n    while i < n:\n        s += i * 3\n        i += 1\n    return s\nf(1000)",
        ),
        (
            "range_1000",
            "def f(n: int) -> int:\n    s = 0\n    for i in range(n):\n        s += i * 3\n    return s\nf(1000)",
        ),
        (
            "branches",
            "def f(n: int) -> int:\n    i = 0\n    s = 0\n    while i < n:\n        if i % 3 == 0:\n            s += i\n        else:\n            s -= i\n        i += 1\n    return s\nf(1000)",
        ),
        (
            "nested_range",
            "def f(n: int) -> int:\n    s = 0\n    for i in range(n):\n        for j in range(i):\n            s += i + j\n    return s\nf(100)",
        ),
        (
            "float_loop",
            "def f(n: int, step: float) -> float:\n    s = 0.0\n    i = 0\n    while i < n:\n        s += step\n        i += 1\n    return s\nf(1000,0.5)",
        ),
        (
            "helper_calls",
            "def f(x: int) -> int:\n    return x * 3 + 1\ni = 0\ns = 0\nwhile i < 1000:\n    s += f(i)\n    i += 1\ns",
        ),
        ("untyped_fallback", "s = 'hello'\ns.upper().replace('L','x')"),
    ];
    let samples = env::args().nth(1).map_or(9, |s| s.parse::<usize>().unwrap());
    println!("case,boundary,mode,sample,ns,compilations,code_bytes,entries,returns,fallbacks,instructions,fast_calls");
    for (name, source) in cases {
        experimental_aot::set_enabled(false);
        let expected = compile(source).run_no_limits(vec![]).unwrap();
        for fresh in [true, false] {
            for sample in 0..samples {
                for offset in 0..3 {
                    let mode = (sample + offset) % 3;
                    experimental_aot::set_enabled(mode != 0);
                    experimental_aot::set_backend(if mode == 2 {
                        experimental_aot::Backend::Shared
                    } else {
                        experimental_aot::Backend::Copied
                    });
                    let mut runner = compile(source);
                    assert_eq!(runner.run_no_limits(vec![]).unwrap(), expected, "{name}");
                    experimental_aot::reset_stats();
                    let start = Instant::now();
                    let mut runs = 0u64;
                    while start.elapsed() < Duration::from_millis(25) {
                        if fresh {
                            black_box(compile(source).run_no_limits(vec![]).unwrap());
                        } else {
                            black_box(runner.run_no_limits(vec![]).unwrap());
                        }
                        runs += 1;
                    }
                    let ns = start.elapsed().as_nanos() / u128::from(runs);
                    let s = experimental_aot::stats();
                    let boundary = if fresh { "fresh" } else { "retained" };
                    let mode = ["interpreter", "aot_copied", "aot_shared"][mode];
                    println!(
                        "{name},{boundary},{mode},{sample},{ns},{},{},{},{},{},{},{}",
                        s.compilations, s.code_bytes, s.entries, s.returns, s.fallbacks, s.instructions, s.fast_calls
                    );
                }
            }
        }
    }
}
