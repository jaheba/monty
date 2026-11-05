//! Timings for string-slicing workloads; run with `cargo run -p monty --profile profiling --example string_views`.
use std::{hint::black_box, time::Instant};

use monty::{Dump, MontyRepl, MontyRun, SessionRef, dump};
use monty_types::{CompileOptions, PrintWriter, ResourceTracker};

fn main() {
    println!("workload,median_ns");
    for (name, code) in [
        ("half", "s = 'x' * 1000000\nv = s[:500000]\nlen(v)"),
        ("prefix", "s = 'x' * 20000\nwhile len(s) > 1:\n    s = s[1:]\nlen(s)"),
        ("suffix", "s = 'x' * 20000\nwhile len(s) > 1:\n    s = s[:-1]\nlen(s)"),
        (
            "chain",
            "s = 'x' * 1000000\na = s[:500000]\nb = a[:300000]\nc = b[:200000]\nlen(c)",
        ),
        (
            "lines",
            "s = ('x' * 200 + '\\n') * 1000\nlines = s.splitlines()\nlen(lines)",
        ),
        (
            "tokens",
            "s = 'x' * 1000000\nv = [s[i:i+8] for i in range(1000)]\nlen(v)",
        ),
        ("retain", "s = 'x' * 1000000\nv = s[:128]\ns = None\nlen(v)"),
        ("unicode", "s = 'é🌍' * 100000\nv = s[1000:100000]\nlen(v)"),
        (
            "operations",
            "s = ' x' * 100000\nv = s[1000:100000]\nfor i in range(100):\n    assert v == v.strip() or v != v.strip()\n    hash(v)\n    v + 'a'\nlen(v)",
        ),
    ] {
        let mut runner = MontyRun::new(code.to_owned(), "bench.py", vec![], CompileOptions::default()).unwrap();
        black_box(runner.run_no_limits(vec![]).unwrap());
        let mut times = Vec::new();
        for _ in 0..7 {
            let start = Instant::now();
            black_box(runner.run_no_limits(vec![]).unwrap());
            times.push(start.elapsed().as_nanos());
        }
        times.sort_unstable();
        println!("{name},{}", times[3]);
    }

    let mut repl = MontyRepl::new("bench.py", ResourceTracker::default(), CompileOptions::default());
    repl.feed_run(
        "s = 'é🌍' * 100000\nv = s[1000:100000]\ns = None",
        vec![],
        PrintWriter::Disabled,
    )
    .unwrap();
    let start = Instant::now();
    let bytes = dump("bench.py", None, &[], SessionRef::Idle(&repl)).unwrap();
    black_box(Dump::load(&bytes).unwrap());
    let elapsed = start.elapsed().as_nanos();
    println!("snapshot,{elapsed}");

    for (name, length) in [("retained_half", 500_000), ("retained_tiny", 128)] {
        let mut repl = MontyRepl::new("bench.py", ResourceTracker::default(), CompileOptions::default());
        let code = format!("s = 'x' * 1000000\nv = s[:{length}]\ns = None");
        let start = Instant::now();
        repl.feed_run(&code, vec![], PrintWriter::Disabled).unwrap();
        let elapsed = start.elapsed().as_nanos();
        println!("{name},{elapsed}");
        black_box(repl);
    }
}
