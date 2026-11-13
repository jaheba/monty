//! Experimental copy-and-patch tier. Its controls and counters are thread-local.
#![allow(unsafe_code)]

#[cfg(monty_native)]
pub(crate) mod abi;

use std::cell::Cell;
#[cfg(monty_native)]
use std::{cell::RefCell, mem, ptr};

#[cfg(monty_native)]
use abi::{Patch, Slot};

#[cfg(monty_native)]
use super::{Code, Opcode};
#[cfg(monty_native)]
use crate::value::Value;

/// Counters establish native coverage independently of elapsed time.
#[derive(Debug, Default, Clone, Copy)]
pub struct Stats {
    pub compilations: u64,
    pub compilation_ns: u64,
    pub code_bytes: u64,
    pub entries: u64,
    pub instructions: u64,
    pub dictionary_reads: u64,
    pub range_steps: u64,
}

thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(!cfg!(feature = "experimental-aot")) };
    static STATS: Cell<Stats> = const { Cell::new(Stats { compilations: 0, compilation_ns: 0, code_bytes: 0, entries: 0, instructions: 0, dictionary_reads: 0, range_steps: 0 }) };
    #[cfg(monty_native)]
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

/// Enables or disables this tier for subsequent execution on the current thread.
pub fn set_enabled(enabled: bool) {
    ENABLED.with(|value| value.set(enabled));
}

/// Returns this thread's native compilation and execution counters.
pub fn stats() -> Stats {
    STATS.with(Cell::get)
}

/// Clears counters without dropping compiled artifacts.
pub fn reset_stats() {
    STATS.with(|value| value.set(Stats::default()));
}

#[cfg(monty_native)]
pub(crate) fn enabled() -> bool {
    ENABLED.with(Cell::get)
}

#[cfg(monty_native)]
pub(crate) fn record(f: impl FnOnce(&mut Stats)) {
    STATS.with(|value| {
        let mut stats = value.get();
        f(&mut stats);
        value.set(stats);
    });
}

#[derive(Default)]
#[cfg(monty_native)]
pub(crate) struct Scratch {
    pub stack: Vec<Slot>,
    pub globals: Vec<Slot>,
    pub stack_values: Vec<Value>,
    pub global_values: Vec<Value>,
}

#[cfg(monty_native)]
pub(crate) fn take_scratch() -> Scratch {
    SCRATCH.with(|value| mem::take(&mut *value.borrow_mut()))
}

#[cfg(monty_native)]
pub(crate) fn put_scratch(scratch: Scratch) {
    debug_assert!(scratch.stack_values.is_empty() && scratch.global_values.is_empty());
    SCRATCH.with(|value| *value.borrow_mut() = scratch);
}

#[cfg(monty_native)]
pub(crate) fn encode(value: &Value) -> Slot {
    match value {
        Value::Undefined => Slot::default(),
        Value::Int(value) => Slot {
            tag: abi::INT,
            bits: value.cast_unsigned(),
        },
        Value::Float(value) => Slot {
            tag: abi::FLOAT,
            bits: value.to_bits(),
        },
        Value::Bool(value) => Slot {
            tag: abi::BOOL,
            bits: u64::from(*value),
        },
        Value::None => Slot {
            tag: abi::NONE,
            bits: 0,
        },
        // Long integer constants need the interpreter's heap materialization path.
        Value::InternLongInt(_) => Slot {
            tag: abi::BLOCKED,
            bits: 0,
        },
        _ => Slot {
            tag: abi::BORROWED,
            bits: ptr::from_ref(value) as usize as u64,
        },
    }
}

#[cfg(monty_native)]
mod backend {
    use std::{
        io, mem, ptr,
        sync::atomic::{AtomicUsize, Ordering},
        time::Instant,
    };

    use memmap2::{Mmap, MmapMut};

    use super::{Code, Opcode, Patch, Slot, abi::Entry, encode, record};

    include!(concat!(env!("OUT_DIR"), "/native_stencils.rs"));

    static MAPPED_BYTES: AtomicUsize = AtomicUsize::new(0);
    const MAX_MAPPED_BYTES: usize = 8 * 1024 * 1024;

    #[derive(Debug)]
    struct MappingBudget(usize);

    impl Drop for MappingBudget {
        fn drop(&mut self) {
            MAPPED_BYTES.fetch_sub(self.0, Ordering::Relaxed);
        }
    }

    #[derive(Debug)]
    pub struct NativeCode {
        memory: Mmap,
        patches: Vec<Patch>,
        entries: Vec<Option<usize>>,
        offsets: Vec<usize>,
        pub(crate) constants: Vec<Slot>,
        _budget: MappingBudget,
        _external_memory: monty_alloc::ExternalMemory,
    }

    struct Instruction {
        ip: usize,
        next: usize,
        branch: Option<usize>,
        argument: usize,
        stencil: &'static str,
    }

    impl NativeCode {
        pub(crate) fn entry(&self, ip: usize) -> Option<(Entry, *const Patch)> {
            let index = self.entries.get(ip).copied().flatten()?;
            let patch = &self.patches[index];
            let address = self.memory.as_ptr() as usize + self.offsets[index];
            // SAFETY: the address names an audited stencil in this artifact's live RX mapping.
            let entry = unsafe { mem::transmute::<usize, Entry>(address) };
            Some((entry, ptr::from_ref(patch)))
        }

        pub(crate) fn compile(code: &Code) -> Option<Self> {
            // The independent fusion experiment changes instruction boundaries in place.
            if cfg!(feature = "experimental-fusion") || code.bytecode().len() > 65_536 {
                return None;
            }
            let start = Instant::now();
            let artifact = Self::link(code).ok()?;
            record(|stats| {
                stats.compilations += 1;
                stats.compilation_ns += u64::try_from(start.elapsed().as_nanos()).unwrap_or(u64::MAX);
                stats.code_bytes += artifact.memory.len() as u64;
            });
            Some(artifact)
        }

        fn link(code: &Code) -> io::Result<Self> {
            let invalid = || io::Error::other("unsupported native bytecode");
            let bytes = code.bytecode();
            let mut instructions = Vec::new();
            let mut boundaries = vec![None; bytes.len()];
            let mut ip = 0;
            while ip < bytes.len() {
                let op = Opcode::from_repr(bytes[ip]).ok_or_else(invalid)?;
                let len = op.encoded_len(bytes, ip).ok_or_else(invalid)?;
                let next = ip + len;
                let u16_arg = || usize::from(u16::from_le_bytes([bytes[ip + 1], bytes[ip + 2]]));
                let (stencil, argument) = match op {
                    Opcode::LoadConst => ("const", u16_arg()),
                    Opcode::LoadSmallInt => ("small_int", usize::from(bytes[ip + 1])),
                    Opcode::LoadNone => ("none", 0),
                    Opcode::LoadTrue => ("true_value", 0),
                    Opcode::LoadFalse => ("false_value", 0),
                    Opcode::LoadLocal0 => ("local", 0),
                    Opcode::LoadLocal1 => ("local", 1),
                    Opcode::LoadLocal2 => ("local", 2),
                    Opcode::LoadLocal3 => ("local", 3),
                    Opcode::LoadLocal => ("local", usize::from(bytes[ip + 1])),
                    Opcode::LoadLocalW => ("local", u16_arg()),
                    Opcode::StoreLocal => ("store_local", usize::from(bytes[ip + 1])),
                    Opcode::StoreLocalW => ("store_local", u16_arg()),
                    Opcode::LoadGlobal => ("global", u16_arg()),
                    Opcode::StoreGlobal => ("store_global", u16_arg()),
                    Opcode::BinaryAdd | Opcode::InplaceAdd => ("add", 0),
                    Opcode::BinarySub | Opcode::InplaceSub => ("sub", 0),
                    Opcode::BinaryMul | Opcode::InplaceMul => ("mul", 0),
                    Opcode::BinaryAnd | Opcode::InplaceAnd => ("and", 0),
                    Opcode::BinaryOr | Opcode::InplaceOr => ("or", 0),
                    Opcode::BinaryXor | Opcode::InplaceXor => ("xor", 0),
                    Opcode::BinaryFloorDiv | Opcode::InplaceFloorDiv => ("floor", 0),
                    Opcode::BinaryMod | Opcode::InplaceMod => ("mod", 0),
                    Opcode::CompareEq => ("eq", 0),
                    Opcode::CompareNe => ("ne", 0),
                    Opcode::CompareLt => ("lt", 0),
                    Opcode::CompareLe => ("le", 0),
                    Opcode::CompareGt => ("gt", 0),
                    Opcode::CompareGe => ("ge", 0),
                    Opcode::Jump => ("jump", 0),
                    Opcode::JumpIfFalse => ("false", 0),
                    Opcode::JumpIfTrue => ("true", 0),
                    Opcode::Pop => ("pop", 0),
                    Opcode::Dup => ("dup", 0),
                    Opcode::UnaryNeg => ("neg", 0),
                    Opcode::UnaryNot => ("not", 0),
                    Opcode::BinarySubscr => ("getitem", 0),
                    Opcode::ForIter => ("for_iter", 0),
                    Opcode::Nop => ("nop", 0),
                    _ => ("exit", 0),
                };
                let branch = if matches!(
                    op,
                    Opcode::Jump | Opcode::JumpIfFalse | Opcode::JumpIfTrue | Opcode::ForIter
                ) {
                    let offset = i16::from_le_bytes([bytes[ip + 1], bytes[ip + 2]]);
                    Some(next.checked_add_signed(isize::from(offset)).ok_or_else(invalid)?)
                } else {
                    None
                };
                boundaries[ip] = Some(instructions.len());
                instructions.push(Instruction {
                    ip,
                    next,
                    branch,
                    argument,
                    stencil,
                });
                ip = next;
            }
            if !instructions.iter().any(|instruction| instruction.stencil != "exit") {
                return Err(invalid());
            }

            let stencil = |name: &str| {
                STENCILS
                    .iter()
                    .find(|(candidate, _)| *candidate == name)
                    .map(|(_, bytes)| *bytes)
                    .ok_or_else(invalid)
            };
            let mut offsets = Vec::with_capacity(instructions.len());
            let mut size = 0usize;
            for instruction in &instructions {
                size = (size + 15) & !15;
                offsets.push(size);
                size = size
                    .checked_add(stencil(instruction.stencil)?.len())
                    .ok_or_else(invalid)?;
            }
            if size > 4 * 1024 * 1024 {
                return Err(invalid());
            }
            // Conservatively cover page rounding, including 64 KiB allocation granularity.
            let reservation = size.next_multiple_of(65_536);
            let mut used = MAPPED_BYTES.load(Ordering::Relaxed);
            loop {
                let total = used
                    .checked_add(reservation)
                    .filter(|&n| n <= MAX_MAPPED_BYTES)
                    .ok_or_else(invalid)?;
                match MAPPED_BYTES.compare_exchange_weak(used, total, Ordering::Relaxed, Ordering::Relaxed) {
                    Ok(_) => break,
                    Err(current) => used = current,
                }
            }
            let budget = MappingBudget(reservation);
            let external_memory = monty_alloc::ExternalMemory::new(reservation);
            let mut memory = MmapMut::map_anon(size)?;
            let base = memory.as_ptr() as usize;
            let mut patches = vec![Patch::default(); instructions.len()];
            let patch_pointer = patches.as_mut_ptr();
            let patch_base = patch_pointer as usize;
            let destination = |ip: usize| -> io::Result<(usize, usize)> {
                let index = boundaries.get(ip).copied().flatten().ok_or_else(invalid)?;
                Ok((base + offsets[index], patch_base + index * size_of::<Patch>()))
            };
            let mut entries = vec![None; bytes.len()];
            for (index, instruction) in instructions.iter().enumerate() {
                let body = stencil(instruction.stencil)?;
                memory[offsets[index]..offsets[index] + body.len()].copy_from_slice(body);
                let current = base + offsets[index];
                let (next, next_data) = if instruction.stencil == "exit" {
                    (current, patch_base + index * size_of::<Patch>())
                } else {
                    destination(if instruction.stencil == "jump" {
                        instruction.branch.unwrap()
                    } else {
                        instruction.next
                    })?
                };
                let (alternate, alternate_data) = match instruction.branch {
                    Some(target) if instruction.stencil != "jump" => destination(target)?,
                    _ => (current, patch_base + index * size_of::<Patch>()),
                };
                let patch = Patch {
                    ip: instruction.ip,
                    argument: instruction.argument,
                    next,
                    next_data,
                    alternate,
                    alternate_data,
                };
                // SAFETY: the vector is fixed and index is in bounds. Initialize
                // through the same raw base used by its internal graph pointers.
                unsafe {
                    patch_pointer.add(index).write(patch);
                }
                if instruction.stencil != "exit" {
                    entries[instruction.ip] = Some(index);
                }
            }
            let memory = memory.make_exec()?;
            if memory.as_ptr() as usize != base {
                return Err(io::Error::other("executable mapping moved"));
            }
            // SAFETY: the whole initialized image is immutable and remains alive in the artifact.
            unsafe {
                wasmtime_internal_jit_icache_coherence::clear_cache(memory.as_ptr().cast(), memory.len())
                    .map_err(io::Error::other)?;
            }
            wasmtime_internal_jit_icache_coherence::pipeline_flush_mt().map_err(io::Error::other)?;
            let constants = code.constants().iter().map(encode).collect();
            Ok(Self {
                memory,
                patches,
                entries,
                offsets,
                constants,
                _budget: budget,
                _external_memory: external_memory,
            })
        }
    }
}

#[cfg(monty_native)]
pub(crate) use backend::NativeCode;
