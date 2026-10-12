#[cfg(all(feature = "worker-alloc", not(codspeed)))]
use std::env;
#[cfg(all(feature = "worker-alloc", not(codspeed)))]
use std::sync::atomic::Ordering;
use std::time::Duration;

#[cfg(codspeed)]
use codspeed_criterion_compat::{BatchSize, Criterion, black_box, criterion_group, criterion_main};
#[cfg(not(codspeed))]
use criterion::{BatchSize, Criterion, black_box};
use monty::MontyRepl;
#[cfg(all(feature = "worker-alloc", not(codspeed)))]
use monty_types::LIVE_MEMORY;
use monty_types::{CompileOptions, MontyObject, PrintWriter, ResourceTracker};

#[cfg(feature = "worker-alloc")]
#[global_allocator]
static ALLOC: monty_alloc::LimitedAllocator = monty_alloc::LimitedAllocator;

fn session(setup: &str, body: &str) -> MontyRepl {
    let mut repl = MontyRepl::new("lists.py", ResourceTracker::default(), CompileOptions::default());
    let mut indented = String::new();
    for line in body.lines() {
        indented.push_str("    ");
        indented.push_str(line);
        indented.push('\n');
    }
    repl.feed_run(
        &format!("{setup}\ndef bench():\n{indented}"),
        vec![],
        PrintWriter::Disabled,
    )
    .unwrap();
    repl
}

fn result(repl: &mut MontyRepl) -> i64 {
    let value = repl
        .call_function("bench", Vec::<MontyObject>::new(), PrintWriter::Disabled)
        .unwrap();
    value.as_ref().try_into().unwrap()
}

// Each timed call gets a fresh session. Setup, list construction and compilation
// are excluded; call dispatch and scalar export are included in every case.
fn benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("lists");
    group.sample_size(10);
    group.warm_up_time(Duration::from_millis(100));
    group.measurement_time(Duration::from_millis(300));
    for n in [16_i64, 4_096, 1_000_000] {
        let ints = format!("xs = list(range({n}))");
        let floats = format!("xs = [float(i) for i in range({n})]");
        let nullable = format!("xs = [None if i % 8 == 0 else i for i in range({n})]");
        let mixed = format!("xs = [i if i % 2 == 0 else float(i) for i in range({n})]");
        let strings = format!("xs = ['value'] * {n}");
        let cases = [
            ("len_int", &ints, "return len(xs)".to_owned(), n),
            ("sum_int", &ints, "return sum(xs)".to_owned(), n * (n - 1) / 2),
            ("sum_float", &floats, "return int(sum(xs))".to_owned(), n * (n - 1) / 2),
            ("sum_mixed", &mixed, "return int(sum(xs))".to_owned(), n * (n - 1) / 2),
            (
                "sum_overflow",
                &ints,
                "return sum(xs, 9223372036854775807) - 9223372036854775807".to_owned(),
                n * (n - 1) / 2,
            ),
            (
                "loop_int",
                &ints,
                "total = 0\nfor x in xs:\n    total += x\nreturn total".to_owned(),
                n * (n - 1) / 2,
            ),
            ("copy_int", &ints, "return len(xs.copy())".to_owned(), n),
            ("slice_int", &ints, "return len(xs[:])".to_owned(), n),
            ("repeat_int", &ints, "return len(xs * 4)".to_owned(), n * 4),
            ("copy_nullable", &nullable, "return len(xs.copy())".to_owned(), n),
            ("copy_strings", &strings, "return len(xs.copy())".to_owned(), n),
            ("copy_mixed", &mixed, "return len(xs.copy())".to_owned(), n),
            (
                "promote_append",
                &ints,
                "xs.append(None)\nreturn len(xs)".to_owned(),
                n + 1,
            ),
            ("set_int", &ints, "xs[-1] = 42\nreturn xs[-1]".to_owned(), 42),
            ("index_int", &ints, "return xs[-1]".to_owned(), n - 1),
            (
                "build_range",
                &String::new(),
                format!("return len(list(range({n})))"),
                n,
            ),
            (
                "build_append",
                &String::new(),
                format!("xs = []\nfor i in range({n}):\n    xs.append(i)\nreturn len(xs)"),
                n,
            ),
        ];
        for (name, setup, body, expected) in cases {
            assert_eq!(result(&mut session(setup, &body)), expected, "{name}/{n}");
            group.bench_function(format!("{name}/{n}"), |b| {
                b.iter_batched_ref(
                    || session(setup, &body),
                    |repl| black_box(result(repl)),
                    BatchSize::PerIteration,
                );
            });
        }
    }
    group.finish();
}

#[cfg(all(feature = "worker-alloc", not(codspeed)))]
fn memory() {
    for n in [16, 4_096, 1_000_000] {
        for (name, expression) in [
            ("int", format!("list(range({n}))")),
            ("float", format!("[float(i) for i in range({n})]")),
            ("nullable", format!("[None if i % 8 == 0 else i for i in range({n})]")),
            ("mixed", format!("[i if i % 2 == 0 else float(i) for i in range({n})]")),
        ] {
            let before = LIVE_MEMORY.load(Ordering::Relaxed);
            let repl = session(&format!("xs = {expression}"), "return len(xs)");
            let retained = LIVE_MEMORY.load(Ordering::Relaxed).saturating_sub(before);
            println!("{name}/{n}: {retained} retained bytes (whole session)");
            drop(repl);
        }
    }
}

#[cfg(not(codspeed))]
fn main() {
    #[cfg(feature = "worker-alloc")]
    if env::args().any(|arg| arg == "--memory") {
        memory();
        return;
    }
    let mut c = Criterion::default().configure_from_args();
    benchmarks(&mut c);
    c.final_summary();
}

#[cfg(codspeed)]
criterion_group!(benches, benchmarks);
#[cfg(codspeed)]
criterion_main!(benches);
