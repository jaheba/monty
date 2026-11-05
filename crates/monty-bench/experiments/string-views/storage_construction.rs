//! Run with `rustc -O storage_construction.rs -o /tmp/string-storage && /tmp/string-storage`.
use std::{hint::black_box, rc::Rc, sync::Arc, time::Instant};

fn main() {
    println!("storage,bytes,median_ns_per_construction");
    for len in [8, 128, 4096, 1_000_000] {
        let iterations = if len >= 1_000_000 { 100 } else { 100_000 };
        for storage in ["Box<str>", "Arc<Box<str>>", "Arc<str>", "Rc<str>"] {
            let mut times = Vec::new();
            for _ in 0..7 {
                let start = Instant::now();
                for _ in 0..iterations {
                    let text = black_box("x").repeat(black_box(len)).into_boxed_str();
                    match storage {
                        "Box<str>" => {
                            black_box(text);
                        }
                        "Arc<Box<str>>" => {
                            black_box(Arc::new(text));
                        }
                        "Arc<str>" => {
                            black_box(Arc::<str>::from(text));
                        }
                        _ => {
                            black_box(Rc::<str>::from(text));
                        }
                    }
                }
                times.push(start.elapsed().as_nanos() / iterations);
            }
            times.sort_unstable();
            println!("{storage},{len},{}", times[3]);
        }
    }
}
