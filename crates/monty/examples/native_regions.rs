//! Compare native and interpreted execution, including compilation for fresh programs.

use std::{
    env,
    hint::black_box,
    time::{Duration, Instant},
};

use monty::{MontyRun, experimental_native};
use monty_types::CompileOptions;

fn compile(source: &str) -> MontyRun {
    MontyRun::new(source.to_owned(), "bench.py", vec![], CompileOptions::default()).unwrap()
}

fn main() {
    let cases = [
        ("tiny_expression", "x = 17\ny = 4\n(x * y + 3) // 2"),
        (
            "short_while",
            "i = 0\ns = 0\nwhile i < 20:\n    s += i * 3\n    i += 1\ns",
        ),
        (
            "while_1000",
            "i = 0\ns = 0\nwhile i < 1000:\n    s += i * 3\n    i += 1\ns",
        ),
        (
            "branches",
            "i = 0\ns = 0\nwhile i < 1000:\n    if i % 3 == 0:\n        s += i\n    else:\n        s -= i\n    i += 1\ns",
        ),
        ("for_range", "s = 0\nfor i in range(1000):\n    s += i * 3\ns"),
        (
            "dict_short",
            "row = {'price': 7}\ni = 0\ns = 0\nwhile i < 20:\n    s += row['price'] * 3\n    i += 1\ns",
        ),
        (
            "dict_loop",
            "row = {'price': 7}\ni = 0\ns = 0\nwhile i < 1000:\n    s += row['price'] * 3\n    i += 1\ns",
        ),
        (
            "dict_mutation",
            "d = {'x': 0}\ni = 0\nwhile i < 1000:\n    d['x'] = d['x'] + 1\n    i += 1\nd['x']",
        ),
        (
            "helper_calls",
            "def adjust(x):\n    return x * 3 + 1\ni = 0\ns = 0\nwhile i < 1000:\n    s += adjust(i)\n    i += 1\ns",
        ),
        (
            "local_loop",
            "def calculate(n):\n    i = 0\n    s = 0\n    while i < n:\n        s += i * 3\n        i += 1\n    return s\ncalculate(1000)",
        ),
        (
            "float_fallback",
            "x = 0.0\ni = 0\nwhile i < 1000:\n    x += 0.5\n    i += 1\nx",
        ),
        ("string_fallback", "s = 'hello'\ns.upper().replace('L', 'x')"),
    ];
    let samples = env::args().nth(1).map_or(7, |s| s.parse::<usize>().unwrap());
    println!(
        "case,boundary,mode,sample,ns,compilations,native_instructions,dictionary_reads,range_steps,compilation_ns,code_bytes,entries"
    );
    for (name, source) in cases {
        experimental_native::set_enabled(false);
        let expected = compile(source).run_no_limits(vec![]).unwrap();
        for fresh in [true, false] {
            for sample in 0..samples {
                for enabled in if sample % 2 == 0 { [false, true] } else { [true, false] } {
                    experimental_native::set_enabled(enabled);
                    let mut runner = compile(source);
                    assert_eq!(runner.run_no_limits(vec![]).unwrap(), expected);
                    experimental_native::reset_stats();
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
                    let stats = experimental_native::stats();
                    let boundary = if fresh { "fresh" } else { "retained" };
                    let mode = if enabled { "native" } else { "interpreter" };
                    println!(
                        "{name},{boundary},{mode},{sample},{ns},{},{},{},{},{},{},{}",
                        stats.compilations,
                        stats.instructions,
                        stats.dictionary_reads,
                        stats.range_steps,
                        stats.compilation_ns,
                        stats.code_bytes,
                        stats.entries,
                    );
                }
            }
        }
    }
}
