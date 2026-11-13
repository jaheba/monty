//! Scalar image shared by the runtime and independently compiled Rust stencils.
#![allow(dead_code, reason = "operation constants are used by the standalone stencil crate")]

use std::ffi::c_void;

pub const UNBOUND: u64 = 0;
pub const INT: u64 = 1;
pub const BOOL: u64 = 2;
pub const NONE: u64 = 3;
pub const FLOAT: u64 = 4;
pub const BORROWED: u64 = 5;
pub const BLOCKED: u64 = 6;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Slot {
    pub tag: u64,
    pub bits: u64,
}

pub type Lookup = unsafe extern "C" fn(*mut c_void, Slot, Slot) -> Slot;
pub type RangeNext = unsafe extern "C" fn(*mut c_void, Slot) -> Slot;
pub type Entry = unsafe extern "C" fn(*mut Context, *const Patch) -> u64;

/// All buffers are fixed for an invocation. Borrowed values remain owned by the VM.
#[repr(C)]
pub struct Context {
    pub stack: *mut Slot,
    pub capacity: usize,
    pub sp: usize,
    pub locals: usize,
    pub globals: *mut Slot,
    pub globals_len: usize,
    pub constants: *const Slot,
    pub constants_len: usize,
    pub remaining: usize,
    pub ip: usize,
    pub opaque: *mut c_void,
    pub lookup: Lookup,
    pub range_next: RangeNext,
    pub dictionary_reads: u64,
    pub range_steps: u64,
}

/// Destinations refer only to immutable code and patch records in the same artifact.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Patch {
    pub ip: usize,
    pub argument: usize,
    pub next: usize,
    pub next_data: usize,
    pub alternate: usize,
    pub alternate_data: usize,
}

pub const EXIT: u8 = 0;
pub const CONST: u8 = 1;
pub const SMALL_INT: u8 = 2;
pub const LOCAL: u8 = 3;
pub const STORE_LOCAL: u8 = 4;
pub const GLOBAL: u8 = 5;
pub const STORE_GLOBAL: u8 = 6;
pub const ADD: u8 = 7;
pub const SUB: u8 = 8;
pub const MUL: u8 = 9;
pub const EQ: u8 = 10;
pub const NE: u8 = 11;
pub const LT: u8 = 12;
pub const LE: u8 = 13;
pub const GT: u8 = 14;
pub const GE: u8 = 15;
pub const JUMP: u8 = 16;
pub const FALSE: u8 = 17;
pub const TRUE: u8 = 18;
pub const POP: u8 = 19;
pub const DUP: u8 = 20;
pub const NEG: u8 = 21;
pub const NOT: u8 = 22;
pub const AND: u8 = 23;
pub const OR: u8 = 24;
pub const XOR: u8 = 25;
pub const FLOOR: u8 = 26;
pub const MOD: u8 = 27;
pub const GETITEM: u8 = 28;
pub const NONE_VALUE: u8 = 29;
pub const TRUE_VALUE: u8 = 30;
pub const FALSE_VALUE: u8 = 31;
pub const NOP: u8 = 32;
pub const FOR_ITER: u8 = 33;
