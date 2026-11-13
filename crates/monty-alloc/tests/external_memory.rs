#![cfg(feature = "exit-code")]

use std::{env, process::Command};

use monty_alloc::{ExternalMemory, LimitedAllocator, set_hard_limit};
use monty_types::OOM_EXIT_CODE;

#[global_allocator]
static ALLOC: LimitedAllocator = LimitedAllocator;

#[test]
fn external_memory_obeys_the_worker_hard_ceiling() {
    const PROBE: &str = "MONTY_EXTERNAL_MEMORY_PROBE";
    if env::var_os(PROBE).is_some() {
        set_hard_limit(Some(1024 * 1024)).unwrap();
        let _mapping = ExternalMemory::new(2 * 1024 * 1024);
        panic!("unmetered executable allocation");
    }
    let output = Command::new(env::current_exe().unwrap())
        .args([
            "--exact",
            "external_memory_obeys_the_worker_hard_ceiling",
            "--nocapture",
        ])
        .env(PROBE, "1")
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(OOM_EXIT_CODE),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("exceeds the memory limit"));
}
