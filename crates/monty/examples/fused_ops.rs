//! Parse once and measure execution independently of the worker protocol.
use std::{env, fs, hint::black_box, path::Path, time::Instant};

use monty::MontyRun;
use monty_types::CompileOptions;

fn main() {
    println!("case,sample,ns");
    for path in env::args().skip(1) {
        let source = fs::read_to_string(&path).unwrap();
        let source = source.replace("print(result)", "result");
        let mut runner = MontyRun::new(source, "bench.py", vec![], CompileOptions::default()).unwrap();
        let expected = runner.run_no_limits(vec![]).unwrap();
        for _ in 0..3 {
            assert_eq!(runner.run_no_limits(vec![]).unwrap(), expected);
        }
        let name = Path::new(&path).file_stem().unwrap().to_str().unwrap();
        for sample in 0..15 {
            let start = Instant::now();
            let mut runs = 0;
            while start.elapsed().as_millis() < 100 {
                black_box(runner.run_no_limits(vec![]).unwrap());
                runs += 1;
            }
            println!("{name},{sample},{}", start.elapsed().as_nanos() / runs);
        }
    }
}
