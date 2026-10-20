use monty::MontyRun;
use monty_ecma::{JsProgress, JsRepl};
use monty_types::{CompileOptions, NamedValues, PrintWriter, ResourceLimits, ResourceTracker};
use std::{hint::black_box, time::Instant};
fn main() {
    let cases = [
        (
            "calls",
            "def f(x):\n    return x + 1\ns = 0\nfor i in range(500):\n    s = f(s)\ns",
            "function f(x) { return x + 1; } let s = 0; for (let i=0; i<500; i++) { s=f(s); } s;",
        ),
        (
            "unwind",
            "def f(x):\n    try:\n        if x % 2:\n            raise ValueError('x')\n        return x\n    except ValueError:\n        return 0\ns = 0\nfor i in range(100):\n    s += f(i)\ns",
            "function f(x) { try { if(x%2) throw new Error('x'); return x; } catch(e) { return 0; } } let s=0; for(let i=0;i<100;i++) s+=f(i); s;",
        ),
        (
            "strings",
            "import re\ns = 'a😀é' * 50\ntotal = 0\nfor i in range(300):\n    total += len(s[50:100]) + s.find('é') + re.search('é', s).start()\ntotal",
            "let s='a😀é'.repeat(50); let total=0; for(let i=0;i<300;i++) total += s.slice(50,100).length + s.indexOf('é') + /é/u.exec(s).index; total;",
        ),
    ];
    for (name, py, js) in cases {
        for _ in 0..5 {
            let start = Instant::now();
            for _ in 0..100 {
                let mut run = MontyRun::new(py.to_owned(), "bench.py", vec![], CompileOptions::default()).unwrap();
                black_box(
                    run.run(vec![], ResourceTracker::default(), PrintWriter::Disabled)
                        .unwrap(),
                );
            }
            println!("python/{name} {}", start.elapsed().as_nanos() / 100);
            let start = Instant::now();
            for _ in 0..100 {
                let repl = JsRepl::new("bench.js", ResourceLimits::default()).unwrap();
                match repl.feed(js, &NamedValues::new(), PrintWriter::Disabled) {
                    JsProgress::Complete { value, .. } => {
                        black_box(value);
                    }
                    _ => panic!("benchmark failed"),
                }
            }
            println!("javascript/{name} {}", start.elapsed().as_nanos() / 100);
        }
    }
}
