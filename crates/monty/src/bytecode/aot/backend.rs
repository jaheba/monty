//! Link typed operations from offline templates without a runtime compiler dependency.
#![allow(unsafe_code)]

use std::{
    io, mem, ptr,
    sync::atomic::{AtomicUsize, Ordering},
};

use memmap2::{Mmap, MmapMut};

use super::{
    abi::{Entry, Patch},
    ir::{Kind, Operation, Plan, Terminator},
};
use crate::bytecode::Opcode;

include!(concat!(env!("OUT_DIR"), "/aot_stencils.rs"));
include!(concat!(env!("OUT_DIR"), "/aot_shared_stencils.rs"));
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
pub(crate) struct Artifact {
    memory: Option<Mmap>,
    entry: Entry,
    patches: Vec<Patch>,
    pub registers: usize,
    _budget: Option<MappingBudget>,
    _external_memory: Option<monty_alloc::ExternalMemory>,
}

struct Node {
    stencil: &'static str,
    patch: Patch,
    next: usize,
    alternate: usize,
}

impl Artifact {
    pub(crate) fn entry(&self) -> (Entry, *const Patch) {
        (self.entry, self.patches.as_ptr())
    }

    pub(crate) fn code_bytes(&self) -> usize {
        self.memory.as_ref().map_or(0, |memory| memory.len())
    }

    pub(crate) fn compile(plan: &Plan, tracker: &monty_types::ResourceTracker) -> Option<Self> {
        Self::link(plan, tracker).ok()
    }

    fn link(plan: &Plan, tracker: &monty_types::ResourceTracker) -> io::Result<Self> {
        let invalid = || io::Error::other("unsupported typed native plan");
        let registers = plan.registers.checked_add(2).ok_or_else(invalid)?;
        let temps = [plan.registers, plan.registers + 1];
        // A mixed numeric operation adds at most two conversion nodes. Preflight
        // linker scratch before constructing it, including patch and address tables.
        let maximum_nodes = plan
            .blocks
            .iter()
            .try_fold(plan.blocks.len(), |count, block| {
                count.checked_add(block.instructions.len().checked_mul(3)?)
            })
            .filter(|count| *count <= 16_384)
            .ok_or_else(invalid)?;
        let scratch = maximum_nodes
            .checked_mul(size_of::<Node>() + 2 * size_of::<Patch>() + 3 * size_of::<usize>())
            .ok_or_else(invalid)?;
        tracker.check_allocation(scratch).map_err(|_| invalid())?;
        let mut nodes = Vec::with_capacity(maximum_nodes);
        let mut blocks = Vec::with_capacity(plan.blocks.len());
        let mut terminals = Vec::with_capacity(plan.blocks.len());
        for block in &plan.blocks {
            blocks.push(nodes.len());
            for instruction in &block.instructions {
                let mut patch = Patch {
                    ip: instruction.ip,
                    weight: instruction.weight,
                    ..Patch::default()
                };
                let stencil = match instruction.operation {
                    Operation::Copy { dst, src, kind } => {
                        patch.dst = dst;
                        patch.left = src;
                        if matches!(
                            kind,
                            Kind::Range | Kind::Iterator | Kind::SequenceIterator(_) | Kind::Maybe(_)
                        ) {
                            "triple"
                        } else {
                            "copy"
                        }
                    }
                    Operation::Range { dst, ref arguments } => {
                        if arguments.is_empty() || arguments.len() > 3 {
                            return Err(invalid());
                        }
                        patch.dst = dst;
                        patch.left = arguments[0];
                        patch.right = *arguments.get(1).unwrap_or(&0);
                        patch.third = *arguments.get(2).unwrap_or(&0);
                        patch.immediate = arguments.len() as u64;
                        "range"
                    }
                    Operation::Iter { dst, src, kind } => {
                        patch.dst = dst;
                        patch.left = src;
                        if kind == Kind::Range { "triple" } else { "sequence_iter" }
                    }
                    Operation::Checkpoint => "jump",
                    Operation::GuardBound { src } => {
                        patch.left = src;
                        "bound"
                    }
                    Operation::Bound { dst, bound } => {
                        patch.dst = dst;
                        patch.immediate = u64::from(bound);
                        "immediate"
                    }
                    Operation::Index { dst, object, key, kind } => {
                        patch.dst = dst;
                        patch.left = object;
                        patch.right = key;
                        if let Some(element) = kind.sequence_element() {
                            patch.immediate = element as u64;
                            "index"
                        } else if let Kind::Dict(element) = kind {
                            patch.immediate = element as u64;
                            "read"
                        } else {
                            return Err(invalid());
                        }
                    }
                    Operation::Length { dst, object, kind } => {
                        if !kind.reference() {
                            return Err(invalid());
                        }
                        patch.dst = dst;
                        patch.left = object;
                        patch.immediate = 256;
                        "read"
                    }
                    Operation::Unpack {
                        dst,
                        object,
                        count,
                        element,
                    } => {
                        patch.dst = dst;
                        patch.left = object;
                        patch.immediate = ((count as u64) << 8) | element as u64;
                        "unpack"
                    }
                    Operation::Unary { dst, src, op, kind } => {
                        patch.dst = dst;
                        patch.left = src;
                        match op {
                            Opcode::UnaryPos => "copy",
                            Opcode::UnaryNeg => {
                                if kind == Kind::Float {
                                    "fneg"
                                } else {
                                    "ineg"
                                }
                            }
                            Opcode::UnaryNot => {
                                if kind == Kind::Float {
                                    "fnot"
                                } else {
                                    "inot"
                                }
                            }
                            Opcode::UnaryInvert => "invert",
                            _ => return Err(invalid()),
                        }
                    }
                    Operation::Binary {
                        dst,
                        left,
                        right,
                        op,
                        lhs,
                        rhs,
                    } => {
                        patch.dst = dst;
                        patch.left = left;
                        patch.right = right;
                        let compare = matches!(
                            op,
                            Opcode::CompareEq
                                | Opcode::CompareNe
                                | Opcode::CompareLt
                                | Opcode::CompareLe
                                | Opcode::CompareGt
                                | Opcode::CompareGe
                        );
                        if lhs == Kind::None || rhs == Kind::None {
                            if !matches!(
                                op,
                                Opcode::CompareEq | Opcode::CompareNe | Opcode::CompareIs | Opcode::CompareIsNot
                            ) {
                                return Err(invalid());
                            }
                            patch.immediate =
                                u64::from((lhs == rhs) == matches!(op, Opcode::CompareEq | Opcode::CompareIs));
                            "immediate"
                        } else {
                            let floating = lhs == Kind::Float
                                || rhs == Kind::Float
                                || matches!(op, Opcode::BinaryDiv | Opcode::InplaceDiv);
                            if floating {
                                for (source, kind, temp, is_left) in
                                    [(left, lhs, temps[0], true), (right, rhs, temps[1], false)]
                                {
                                    if kind != Kind::Float {
                                        let conversion = Patch {
                                            dst: temp,
                                            left: source,
                                            ip: instruction.ip,
                                            weight: patch.weight,
                                            ..Patch::default()
                                        };
                                        patch.weight = 0;
                                        let exact = compare || matches!(op, Opcode::BinaryDiv | Opcode::InplaceDiv);
                                        let next = nodes.len() + 1;
                                        nodes.push(Node {
                                            stencil: if exact { "exact_float" } else { "float" },
                                            patch: conversion,
                                            next,
                                            alternate: next,
                                        });
                                        if is_left { patch.left = temp } else { patch.right = temp }
                                    }
                                }
                            }
                            match (op, floating) {
                                (Opcode::BinaryAdd | Opcode::InplaceAdd, false) => "iadd",
                                (Opcode::BinarySub | Opcode::InplaceSub, false) => "isub",
                                (Opcode::BinaryMul | Opcode::InplaceMul, false) => "imul",
                                (Opcode::BinaryFloorDiv | Opcode::InplaceFloorDiv, false) => "ifloor",
                                (Opcode::BinaryMod | Opcode::InplaceMod, false) => "imod",
                                (Opcode::BinaryAnd | Opcode::InplaceAnd, false) => "and",
                                (Opcode::BinaryOr | Opcode::InplaceOr, false) => "or",
                                (Opcode::BinaryXor | Opcode::InplaceXor, false) => "xor",
                                (Opcode::BinaryAdd | Opcode::InplaceAdd, true) => "fadd",
                                (Opcode::BinarySub | Opcode::InplaceSub, true) => "fsub",
                                (Opcode::BinaryMul | Opcode::InplaceMul, true) => "fmul",
                                (Opcode::BinaryDiv | Opcode::InplaceDiv, true) => "fdiv",
                                (Opcode::CompareEq | Opcode::CompareIs, false) => "ieq",
                                (Opcode::CompareNe | Opcode::CompareIsNot, false) => "ine",
                                (Opcode::CompareLt, false) => "ilt",
                                (Opcode::CompareLe, false) => "ile",
                                (Opcode::CompareGt, false) => "igt",
                                (Opcode::CompareGe, false) => "ige",
                                (Opcode::CompareEq, true) => "feq",
                                (Opcode::CompareNe, true) => "fne",
                                (Opcode::CompareLt, true) => "flt",
                                (Opcode::CompareLe, true) => "fle",
                                (Opcode::CompareGt, true) => "fgt",
                                (Opcode::CompareGe, true) => "fge",
                                _ => return Err(invalid()),
                            }
                        }
                    }
                };
                let next = nodes.len() + 1;
                nodes.push(Node {
                    stencil,
                    patch,
                    next,
                    alternate: next,
                });
            }
            let mut patch = Patch {
                ip: block.terminal_ip,
                weight: block.terminal_weight,
                ..Patch::default()
            };
            let (stencil, next, alternate) = match block.terminator {
                Terminator::Jump(target) => ("jump", target, target),
                Terminator::ForIter {
                    iterator,
                    item,
                    element,
                    yes,
                    no,
                } => {
                    patch.left = iterator;
                    patch.dst = item;
                    if let Some(element) = element {
                        patch.right = iterator + 1;
                        patch.immediate = element as u64;
                        ("sequence_next", yes, no)
                    } else {
                        ("for_iter", yes, no)
                    }
                }
                Terminator::Branch {
                    condition,
                    kind,
                    yes,
                    no,
                } => {
                    patch.left = condition;
                    (if kind == Kind::Float { "fbranch" } else { "ibranch" }, yes, no)
                }
                Terminator::Return { source, kind } => {
                    patch.left = source;
                    patch.immediate = match kind {
                        Kind::Int => 1,
                        Kind::Float => 2,
                        Kind::Bool => 3,
                        Kind::None => 4,
                        Kind::TupleValue(element, count) => (u64::from(count) << 8) | (16 + element as u64),
                        _ => return Err(invalid()),
                    };
                    ("return", 0, 0)
                }
            };
            terminals.push(nodes.len());
            nodes.push(Node {
                stencil,
                patch,
                next,
                alternate,
            });
        }
        for index in terminals {
            let node = &mut nodes[index];
            node.next = *blocks.get(node.next).ok_or_else(invalid)?;
            node.alternate = *blocks.get(node.alternate).ok_or_else(invalid)?;
        }
        if nodes.is_empty() || nodes.len() > 16_384 {
            return Err(invalid());
        }
        if super::backend() == super::Backend::Shared {
            let metadata = nodes.len().checked_mul(size_of::<Patch>()).ok_or_else(invalid)?;
            tracker.check_allocation(metadata).map_err(|_| invalid())?;
            let mut patches = vec![Patch::default(); nodes.len()];
            // Vec moves do not retag its allocation. Build every internal pointer
            // from one raw base and initialize through that base before publication.
            let patch_base = patches.as_mut_ptr();
            let patch_address = |index| {
                if index >= nodes.len() {
                    return Err(invalid());
                }
                Ok(patch_base.wrapping_add(index) as usize)
            };
            let mut addresses = Vec::with_capacity(nodes.len());
            for node in &nodes {
                let entry = SHARED_STENCILS
                    .iter()
                    .find(|(name, _)| *name == node.stencil)
                    .ok_or_else(invalid)?
                    .1;
                addresses.push(entry);
            }
            for (index, node) in nodes.iter().enumerate() {
                if node.patch.dst >= registers
                    || node.patch.left >= registers
                    || node.patch.right >= registers
                    || node.patch.third >= registers
                    || node.patch.weight > 255
                {
                    return Err(invalid());
                }
                let next_data = patch_address(node.next)?;
                let alternate_data = patch_address(node.alternate)?;
                let patch = Patch {
                    next: *addresses.get(node.next).ok_or_else(invalid)? as usize,
                    next_data,
                    alternate: *addresses.get(node.alternate).ok_or_else(invalid)? as usize,
                    alternate_data,
                    ..node.patch
                };
                // SAFETY: index is within this fixed initialized vector; no references
                // into it coexist with these writes or escape before publication.
                unsafe {
                    patch_base.add(index).write(patch);
                }
            }
            super::record(|stats| stats.shared_compilations += 1);
            return Ok(Self {
                memory: None,
                entry: addresses[0],
                patches,
                registers,
                _budget: None,
                _external_memory: None,
            });
        }
        let stencil = |name| {
            STENCILS
                .iter()
                .find(|(candidate, _)| *candidate == name)
                .map(|(_, bytes)| *bytes)
                .ok_or_else(invalid)
        };
        let mut offsets = Vec::with_capacity(nodes.len());
        let mut size = 0;
        for node in &nodes {
            if node.patch.dst >= registers
                || node.patch.left >= registers
                || node.patch.right >= registers
                || node.patch.third >= registers
                || node.patch.weight > 255
            {
                return Err(invalid());
            }
            size = (size + 15) & !15;
            offsets.push(size);
            size += stencil(node.stencil)?.len();
        }
        if size > 4 * 1024 * 1024 {
            return Err(invalid());
        }
        let reservation = size.next_multiple_of(65_536);
        let metadata = nodes.len().checked_mul(size_of::<Patch>() * 2).ok_or_else(invalid)?;
        tracker
            .check_allocation(reservation.checked_add(metadata).ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
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
        let mut patches = vec![Patch::default(); nodes.len()];
        let patch_base = patches.as_mut_ptr();
        let destination = |index: usize| -> io::Result<(usize, usize)> {
            Ok((
                base + *offsets.get(index).ok_or_else(invalid)?,
                patch_base.wrapping_add(index) as usize,
            ))
        };
        let mut prepared = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            let bytes = stencil(node.stencil)?;
            memory[offsets[index]..offsets[index] + bytes.len()].copy_from_slice(bytes);
            let (next, next_data) = destination(node.next)?;
            let (alternate, alternate_data) = destination(node.alternate)?;
            prepared.push(Patch {
                next,
                next_data,
                alternate,
                alternate_data,
                ..node.patch
            });
        }
        // SAFETY: both buffers contain exactly nodes.len() initialized patches and
        // do not overlap. Raw initialization preserves the graph pointers' provenance.
        unsafe {
            ptr::copy_nonoverlapping(prepared.as_ptr(), patch_base, prepared.len());
        }
        let memory = memory.make_exec()?;
        if memory.as_ptr() as usize != base {
            return Err(invalid());
        }
        // SAFETY: the initialized mapping is immutable and retained for every invocation.
        unsafe {
            wasmtime_internal_jit_icache_coherence::clear_cache(memory.as_ptr().cast(), memory.len())
                .map_err(io::Error::other)?;
        }
        wasmtime_internal_jit_icache_coherence::pipeline_flush_mt().map_err(io::Error::other)?;
        // SAFETY: the first checked template starts at this immutable RX mapping's base.
        let entry = unsafe { mem::transmute::<*const u8, Entry>(memory.as_ptr()) };
        Ok(Self {
            memory: Some(memory),
            entry,
            patches,
            registers,
            _budget: Some(budget),
            _external_memory: Some(external_memory),
        })
    }
}
