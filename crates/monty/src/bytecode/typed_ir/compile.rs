//! Lower typed values and control flow to executable register instructions.

use std::io;

use super::{
    execute::{Compiled, InstructionKind, Operands},
    ir::{Kind, Operation, Plan, Terminator},
};
use crate::bytecode::Opcode;

struct Node {
    operation: InstructionKind,
    operands: Operands,
    next: usize,
    alternate: usize,
}

impl Compiled {
    pub(crate) fn compile(plan: &Plan, tracker: &monty_types::ResourceTracker) -> Option<Self> {
        Self::lower(plan, tracker).ok()
    }

    fn lower(plan: &Plan, tracker: &monty_types::ResourceTracker) -> io::Result<Self> {
        let invalid = || io::Error::other("unsupported typed instruction plan");
        let registers = plan.registers.checked_add(2).ok_or_else(invalid)?;
        let temps = [plan.registers, plan.registers + 1];
        // A mixed numeric operation adds at most two conversion nodes. Preflight
        // compiler scratch before constructing it, including branch tables.
        let maximum_nodes = plan
            .blocks
            .iter()
            .try_fold(plan.blocks.len(), |count, block| {
                count.checked_add(block.instructions.len().checked_mul(3)?)
            })
            .filter(|count| *count <= 16_384)
            .ok_or_else(invalid)?;
        let scratch = maximum_nodes
            .checked_mul(size_of::<Node>() + size_of::<super::execute::Instruction>() + 2 * size_of::<usize>())
            .ok_or_else(invalid)?;
        tracker.check_allocation(scratch).map_err(|_| invalid())?;
        let mut nodes = Vec::with_capacity(maximum_nodes);
        let mut blocks = Vec::with_capacity(plan.blocks.len());
        let mut terminals = Vec::with_capacity(plan.blocks.len());
        for block in &plan.blocks {
            blocks.push(nodes.len());
            for instruction in &block.instructions {
                let mut operands = Operands {
                    ip: instruction.ip,
                    weight: instruction.weight,
                    ..Operands::default()
                };
                let operation = match instruction.operation {
                    Operation::Copy { dst, src, kind } => {
                        operands.dst = dst;
                        operands.left = src;
                        if matches!(
                            kind,
                            Kind::Range | Kind::Iterator | Kind::SequenceIterator(_) | Kind::Maybe(_)
                        ) {
                            InstructionKind::Triple
                        } else {
                            InstructionKind::Copy
                        }
                    }
                    Operation::Range { dst, ref arguments } => {
                        if arguments.is_empty() || arguments.len() > 3 {
                            return Err(invalid());
                        }
                        operands.dst = dst;
                        operands.left = arguments[0];
                        operands.right = *arguments.get(1).unwrap_or(&0);
                        operands.third = *arguments.get(2).unwrap_or(&0);
                        operands.immediate = arguments.len() as u64;
                        InstructionKind::Range
                    }
                    Operation::Iter { dst, src, kind } => {
                        operands.dst = dst;
                        operands.left = src;
                        if kind == Kind::Range {
                            InstructionKind::Triple
                        } else {
                            InstructionKind::SequenceIter
                        }
                    }
                    Operation::Checkpoint => InstructionKind::Jump,
                    Operation::GuardBound { src } => {
                        operands.left = src;
                        InstructionKind::Bound
                    }
                    Operation::Bound { dst, bound } => {
                        operands.dst = dst;
                        operands.immediate = u64::from(bound);
                        InstructionKind::Immediate
                    }
                    Operation::Index { dst, object, key, kind } => {
                        operands.dst = dst;
                        operands.left = object;
                        operands.right = key;
                        if let Some(element) = kind.sequence_element() {
                            operands.immediate = element as u64;
                            InstructionKind::Index
                        } else if let Kind::Dict(element) = kind {
                            operands.immediate = element as u64;
                            InstructionKind::Read
                        } else {
                            return Err(invalid());
                        }
                    }
                    Operation::Length { dst, object, kind } => {
                        if !kind.reference() {
                            return Err(invalid());
                        }
                        operands.dst = dst;
                        operands.left = object;
                        operands.immediate = 256;
                        InstructionKind::Read
                    }
                    Operation::Unpack {
                        dst,
                        object,
                        count,
                        element,
                    } => {
                        operands.dst = dst;
                        operands.left = object;
                        operands.immediate = ((count as u64) << 8) | element as u64;
                        InstructionKind::Unpack
                    }
                    Operation::Unary { dst, src, op, kind } => {
                        operands.dst = dst;
                        operands.left = src;
                        match op {
                            Opcode::UnaryPos => InstructionKind::Copy,
                            Opcode::UnaryNeg => {
                                if kind == Kind::Float {
                                    InstructionKind::Fneg
                                } else {
                                    InstructionKind::Ineg
                                }
                            }
                            Opcode::UnaryNot => {
                                if kind == Kind::Float {
                                    InstructionKind::Fnot
                                } else {
                                    InstructionKind::Inot
                                }
                            }
                            Opcode::UnaryInvert => InstructionKind::Invert,
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
                        operands.dst = dst;
                        operands.left = left;
                        operands.right = right;
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
                            operands.immediate =
                                u64::from((lhs == rhs) == matches!(op, Opcode::CompareEq | Opcode::CompareIs));
                            InstructionKind::Immediate
                        } else {
                            let floating = lhs == Kind::Float
                                || rhs == Kind::Float
                                || matches!(op, Opcode::BinaryDiv | Opcode::InplaceDiv);
                            if floating {
                                for (source, kind, temp, is_left) in
                                    [(left, lhs, temps[0], true), (right, rhs, temps[1], false)]
                                {
                                    if kind != Kind::Float {
                                        let conversion = Operands {
                                            dst: temp,
                                            left: source,
                                            ip: instruction.ip,
                                            weight: operands.weight,
                                            ..Operands::default()
                                        };
                                        operands.weight = 0;
                                        let exact = compare || matches!(op, Opcode::BinaryDiv | Opcode::InplaceDiv);
                                        let next = nodes.len() + 1;
                                        nodes.push(Node {
                                            operation: if exact {
                                                InstructionKind::ExactFloat
                                            } else {
                                                InstructionKind::Float
                                            },
                                            operands: conversion,
                                            next,
                                            alternate: next,
                                        });
                                        if is_left {
                                            operands.left = temp;
                                        } else {
                                            operands.right = temp;
                                        }
                                    }
                                }
                            }
                            match (op, floating) {
                                (Opcode::BinaryAdd | Opcode::InplaceAdd, false) => InstructionKind::Iadd,
                                (Opcode::BinarySub | Opcode::InplaceSub, false) => InstructionKind::Isub,
                                (Opcode::BinaryMul | Opcode::InplaceMul, false) => InstructionKind::Imul,
                                (Opcode::BinaryFloorDiv | Opcode::InplaceFloorDiv, false) => InstructionKind::Ifloor,
                                (Opcode::BinaryMod | Opcode::InplaceMod, false) => InstructionKind::Imod,
                                (Opcode::BinaryAnd | Opcode::InplaceAnd, false) => InstructionKind::And,
                                (Opcode::BinaryOr | Opcode::InplaceOr, false) => InstructionKind::Or,
                                (Opcode::BinaryXor | Opcode::InplaceXor, false) => InstructionKind::Xor,
                                (Opcode::BinaryAdd | Opcode::InplaceAdd, true) => InstructionKind::Fadd,
                                (Opcode::BinarySub | Opcode::InplaceSub, true) => InstructionKind::Fsub,
                                (Opcode::BinaryMul | Opcode::InplaceMul, true) => InstructionKind::Fmul,
                                (Opcode::BinaryDiv | Opcode::InplaceDiv, true) => InstructionKind::Fdiv,
                                (Opcode::CompareEq | Opcode::CompareIs, false) => InstructionKind::Ieq,
                                (Opcode::CompareNe | Opcode::CompareIsNot, false) => InstructionKind::Ine,
                                (Opcode::CompareLt, false) => InstructionKind::Ilt,
                                (Opcode::CompareLe, false) => InstructionKind::Ile,
                                (Opcode::CompareGt, false) => InstructionKind::Igt,
                                (Opcode::CompareGe, false) => InstructionKind::Ige,
                                (Opcode::CompareEq, true) => InstructionKind::Feq,
                                (Opcode::CompareNe, true) => InstructionKind::Fne,
                                (Opcode::CompareLt, true) => InstructionKind::Flt,
                                (Opcode::CompareLe, true) => InstructionKind::Fle,
                                (Opcode::CompareGt, true) => InstructionKind::Fgt,
                                (Opcode::CompareGe, true) => InstructionKind::Fge,
                                _ => return Err(invalid()),
                            }
                        }
                    }
                };
                let next = nodes.len() + 1;
                nodes.push(Node {
                    operation,
                    operands,
                    next,
                    alternate: next,
                });
            }
            let mut operands = Operands {
                ip: block.terminal_ip,
                weight: block.terminal_weight,
                ..Operands::default()
            };
            let (operation, next, alternate) = match block.terminator {
                Terminator::Jump(target) => (InstructionKind::Jump, target, target),
                Terminator::ForIter {
                    iterator,
                    item,
                    element,
                    yes,
                    no,
                } => {
                    operands.left = iterator;
                    operands.dst = item;
                    if let Some(element) = element {
                        operands.right = iterator + 1;
                        operands.immediate = element as u64;
                        (InstructionKind::SequenceNext, yes, no)
                    } else {
                        (InstructionKind::ForIter, yes, no)
                    }
                }
                Terminator::Branch {
                    condition,
                    kind,
                    yes,
                    no,
                } => {
                    operands.left = condition;
                    (
                        if kind == Kind::Float {
                            InstructionKind::Fbranch
                        } else {
                            InstructionKind::Ibranch
                        },
                        yes,
                        no,
                    )
                }
                Terminator::Return { source, kind } => {
                    operands.left = source;
                    operands.immediate = match kind {
                        Kind::Int => 1,
                        Kind::Float => 2,
                        Kind::Bool => 3,
                        Kind::None => 4,
                        Kind::TupleValue(element, count) => (u64::from(count) << 8) | (16 + element as u64),
                        _ => return Err(invalid()),
                    };
                    (InstructionKind::Return, 0, 0)
                }
            };
            terminals.push(nodes.len());
            nodes.push(Node {
                operation,
                operands,
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
        let instructions = nodes
            .iter()
            .map(|node| (node.operation, node.operands, node.next, node.alternate));
        Self::new(instructions, registers)
    }
}
