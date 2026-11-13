//! Fixed ABI for typed stencils; Rust VM and heap layouts stay behind callbacks.
#![allow(dead_code)]

use std::ffi::c_void;

pub type Entry = unsafe extern "C" fn(*mut Context, *const Patch) -> u64;
pub type Checkpoint = unsafe extern "C" fn(*mut c_void, usize) -> bool;
pub type Helper = unsafe extern "C" fn(*mut Context, *const Patch) -> bool;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct View {
    pub data: *const u64,
    pub length: usize,
    pub prepared: usize,
}

#[repr(C)]
pub struct Context {
    pub registers: *mut u64,
    pub length: usize,
    pub remaining: usize,
    pub opaque: *mut c_void,
    pub checkpoint: Checkpoint,
    pub ip: usize,
    pub result: u64,
    pub result_kind: usize,
    pub result_start: usize,
    /// Fuel granted across intervals; subtract remaining fuel at exit for executed work.
    pub instructions: u64,
    pub range_init: Helper,
    pub views: *mut View,
    pub views_len: usize,
    pub prepare_view: Helper,
    pub read: Helper,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Patch {
    pub dst: usize,
    pub left: usize,
    pub right: usize,
    pub third: usize,
    pub immediate: u64,
    pub ip: usize,
    pub weight: usize,
    pub next: usize,
    pub next_data: usize,
    pub alternate: usize,
    pub alternate_data: usize,
}

pub const MISS: u64 = 0;
pub const RETURN: u64 = 1;
pub const ERROR: u64 = 2;
