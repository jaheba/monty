use std::io::{self, Read};

use monty_type_checking::{SourceFile, TypeChecker};
use monty_types::TypeCheckingConfig;

#[global_allocator]
static ALLOC: monty_alloc::LimitedAllocator = monty_alloc::LimitedAllocator;

fn main() {
    monty_alloc::set_hard_limit(Some(512 * 1024 * 1024)).unwrap();
    let mut source = String::new();
    io::stdin().read_to_string(&mut source).unwrap();
    let mut checker = TypeChecker::default();
    let status = match checker.run(
        &SourceFile::new(&source, "attack.py"),
        None,
        TypeCheckingConfig::default(),
    ) {
        Ok(None) => "clean",
        Ok(Some(_)) => "diagnostics",
        Err(_) => "internal-error",
    };
    println!("{status}");
}
