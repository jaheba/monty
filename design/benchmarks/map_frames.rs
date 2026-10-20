use std::{hint::black_box, time::Instant};
use monty_ecma::{JsProgress, JsRepl};
use monty_types::{NamedValues, PrintWriter, ResourceLimits};
fn main() {
    for (name, source) in [
        ("map", "let a = Array.from({length: 1000}, (_, i) => i); for (let i = 0; i < 20; i++) a = a.map(x => x + 1); a[0];"),
        ("calls", "function f(x) { return x + 1; } let s = 0; for(let i = 0; i < 500; i++) s = f(s); s;")
    ] {
        for _ in 0..5 {
            let start = Instant::now();
            for _ in 0..100 {
                let repl = JsRepl::new("bench.js", ResourceLimits::default()).unwrap();
                match repl.feed(source, &NamedValues::new(), PrintWriter::Disabled) {
                    JsProgress::Complete { value, .. } => { black_box(value); }
                    _ => panic!("benchmark failed"),
                }
            }
            println!("{name} {}", start.elapsed().as_nanos() / 100);
        }
    }
}
