//! Typed register instructions and their guarded execution.

use std::io;

use crate::types::Range;

#[derive(Debug, Clone, Copy)]
pub(super) enum InstructionKind {
    Copy,
    Iadd,
    Isub,
    Imul,
    Ifloor,
    Imod,
    And,
    Or,
    Xor,
    Ineg,
    Invert,
    Inot,
    Fnot,
    Fadd,
    Fsub,
    Fmul,
    Fdiv,
    Fneg,
    Float,
    ExactFloat,
    Ieq,
    Ine,
    Ilt,
    Ile,
    Igt,
    Ige,
    Feq,
    Fne,
    Flt,
    Fle,
    Fgt,
    Fge,
    Jump,
    Ibranch,
    Fbranch,
    Return,
    Immediate,
    Range,
    Triple,
    ForIter,
    Index,
    Read,
    Unpack,
    Bound,
    SequenceIter,
    SequenceNext,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Operands {
    pub dst: usize,
    pub left: usize,
    pub right: usize,
    pub third: usize,
    pub immediate: u64,
    pub ip: usize,
    pub weight: usize,
}

#[derive(Clone, Copy, Default)]
pub(crate) enum View {
    #[default]
    Unprepared,
    Buffered(usize),
    Direct(usize),
}

impl View {
    fn len(self) -> usize {
        match self {
            Self::Unprepared => 0,
            Self::Buffered(length) | Self::Direct(length) => length,
        }
    }
}

pub(crate) struct Context<'a> {
    pub registers: &'a mut [u64],
    pub views: &'a mut [View],
    pub remaining: usize,
    pub ip: usize,
    pub result: u64,
    pub result_kind: usize,
    pub result_start: usize,
    /// Fuel granted across intervals; subtract remaining fuel for executed work.
    pub instructions: u64,
}

pub(crate) trait Helpers {
    fn checkpoint(&mut self, ip: usize) -> bool;
    fn prepare_view(&mut self, context: &mut Context<'_>, operands: &Operands) -> bool;
    fn read(&mut self, context: &mut Context<'_>, operands: &Operands) -> bool;
    fn buffer(&self, index: usize) -> Option<&[u64]>;
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Return,
    Miss,
    Error,
}

enum Direction {
    Next,
    Alternate,
    Return,
}

#[derive(Debug)]
pub(super) struct Instruction {
    immediate: u64,
    dst: u16,
    left: u16,
    right: u16,
    third: u16,
    next: u16,
    alternate: u16,
    ip: u16,
    weight: u8,
    operation: InstructionKind,
}

#[derive(Debug)]
pub(crate) struct Compiled {
    instructions: Vec<Instruction>,
    pub registers: usize,
}

impl Compiled {
    pub(super) fn new(
        nodes: impl ExactSizeIterator<Item = (InstructionKind, Operands, usize, usize)>,
        registers: usize,
    ) -> io::Result<Self> {
        let invalid = || io::Error::other("unsupported typed register instruction");
        let mut instructions = Vec::with_capacity(nodes.len());
        for (operation, operands, next, alternate) in nodes {
            if [operands.dst, operands.left, operands.right, operands.third]
                .iter()
                .any(|&index| index >= registers)
            {
                return Err(invalid());
            }
            instructions.push(Instruction {
                immediate: operands.immediate,
                dst: operands.dst.try_into().map_err(|_| invalid())?,
                left: operands.left.try_into().map_err(|_| invalid())?,
                right: operands.right.try_into().map_err(|_| invalid())?,
                third: operands.third.try_into().map_err(|_| invalid())?,
                next: next.try_into().map_err(|_| invalid())?,
                alternate: alternate.try_into().map_err(|_| invalid())?,
                ip: operands.ip.try_into().map_err(|_| invalid())?,
                weight: operands.weight.try_into().map_err(|_| invalid())?,
                operation,
            });
        }
        Ok(Self {
            instructions,
            registers,
        })
    }

    pub(crate) fn run(&self, context: &mut Context<'_>, helpers: &mut impl Helpers) -> Outcome {
        let mut pc = 0;
        loop {
            let Some(instruction) = self.instructions.get(pc) else {
                return Outcome::Miss;
            };
            let operands = Operands {
                immediate: instruction.immediate,
                dst: usize::from(instruction.dst),
                left: usize::from(instruction.left),
                right: usize::from(instruction.right),
                third: usize::from(instruction.third),
                ip: usize::from(instruction.ip),
                weight: usize::from(instruction.weight),
            };
            context.ip = operands.ip;
            if context.remaining < operands.weight {
                if !helpers.checkpoint(operands.ip) {
                    return Outcome::Error;
                }
                context.instructions += (255 - context.remaining) as u64;
                context.remaining = 255;
            }
            context.remaining -= operands.weight;
            match execute(context, &operands, instruction.operation, helpers) {
                Some(Direction::Next) => pc = usize::from(instruction.next),
                Some(Direction::Alternate) => pc = usize::from(instruction.alternate),
                Some(Direction::Return) => return Outcome::Return,
                None => return Outcome::Miss,
            }
        }
    }
}

fn view(context: &mut Context<'_>, operands: &Operands, source: u64, helpers: &mut impl Helpers) -> Option<View> {
    let index = usize::try_from(source).ok()?;
    if matches!(context.views.get(index)?, View::Unprepared) && !helpers.prepare_view(context, operands) {
        return None;
    }
    let view = *context.views.get(index)?;
    (!matches!(view, View::Unprepared)).then_some(view)
}

#[expect(clippy::float_cmp, reason = "Python equality compares floating point values exactly")]
fn execute(
    context: &mut Context<'_>,
    operands: &Operands,
    operation: InstructionKind,
    helpers: &mut impl Helpers,
) -> Option<Direction> {
    if operands.dst >= context.registers.len()
        || operands.left >= context.registers.len()
        || operands.right >= context.registers.len()
    {
        return None;
    }
    let left = *context.registers.get(operands.left)?;
    let right = *context.registers.get(operands.right)?;
    let integer_left = left.cast_signed();
    let integer_right = right.cast_signed();
    let float_left = f64::from_bits(left);
    let float_right = f64::from_bits(right);
    let value = match operation {
        InstructionKind::Copy => left,
        InstructionKind::Iadd => integer_left.checked_add(integer_right)?.cast_unsigned(),
        InstructionKind::Isub => integer_left.checked_sub(integer_right)?.cast_unsigned(),
        InstructionKind::Imul => integer_left.checked_mul(integer_right)?.cast_unsigned(),
        InstructionKind::Ifloor => {
            let quotient = integer_left.checked_div(integer_right)?;
            let remainder = integer_left.checked_rem(integer_right)?;
            if remainder != 0 && (remainder < 0) != (integer_right < 0) {
                quotient.checked_sub(1)?.cast_unsigned()
            } else {
                quotient.cast_unsigned()
            }
        }
        InstructionKind::Imod => {
            let remainder = integer_left.checked_rem(integer_right)?;
            if remainder != 0 && (remainder < 0) != (integer_right < 0) {
                remainder.checked_add(integer_right)?.cast_unsigned()
            } else {
                remainder.cast_unsigned()
            }
        }
        InstructionKind::And => left & right,
        InstructionKind::Or => left | right,
        InstructionKind::Xor => left ^ right,
        InstructionKind::Ineg => integer_left.checked_neg()?.cast_unsigned(),
        InstructionKind::Invert => !left,
        InstructionKind::Inot => u64::from(left == 0),
        InstructionKind::Fnot => u64::from(float_left == 0.0),
        InstructionKind::Fadd => (float_left + float_right).to_bits(),
        InstructionKind::Fsub => (float_left - float_right).to_bits(),
        InstructionKind::Fmul => (float_left * float_right).to_bits(),
        InstructionKind::Fdiv => {
            if float_right == 0.0 {
                return None;
            }
            (float_left / float_right).to_bits()
        }
        InstructionKind::Fneg => (-float_left).to_bits(),
        InstructionKind::Float => (integer_left as f64).to_bits(),
        // Exact mixed comparisons and int/int true division require lossless conversion.
        InstructionKind::ExactFloat => {
            if !(-9_007_199_254_740_992..=9_007_199_254_740_992).contains(&integer_left) {
                return None;
            }
            (integer_left as f64).to_bits()
        }
        InstructionKind::Ieq => u64::from(integer_left == integer_right),
        InstructionKind::Ine => u64::from(integer_left != integer_right),
        InstructionKind::Ilt => u64::from(integer_left < integer_right),
        InstructionKind::Ile => u64::from(integer_left <= integer_right),
        InstructionKind::Igt => u64::from(integer_left > integer_right),
        InstructionKind::Ige => u64::from(integer_left >= integer_right),
        InstructionKind::Feq => u64::from(float_left == float_right),
        InstructionKind::Fne => u64::from(float_left != float_right),
        InstructionKind::Flt => u64::from(float_left < float_right),
        InstructionKind::Fle => u64::from(float_left <= float_right),
        InstructionKind::Fgt => u64::from(float_left > float_right),
        InstructionKind::Fge => u64::from(float_left >= float_right),
        InstructionKind::Jump => return Some(Direction::Next),
        InstructionKind::Ibranch => {
            return Some(if left != 0 {
                Direction::Next
            } else {
                Direction::Alternate
            });
        }
        InstructionKind::Fbranch => {
            return Some(if float_left == 0.0 {
                Direction::Alternate
            } else {
                Direction::Next
            });
        }
        InstructionKind::Return => {
            context.result = left;
            context.result_start = operands.left;
            context.result_kind = usize::try_from(operands.immediate).ok()?;
            context.ip = operands.ip;
            return Some(Direction::Return);
        }
        InstructionKind::Immediate => operands.immediate,
        InstructionKind::Range => {
            initialize_range(context, operands)?;
            return Some(Direction::Next);
        }
        InstructionKind::Triple => {
            if operands.left.checked_add(2)? >= context.registers.len()
                || operands.dst.checked_add(2)? >= context.registers.len()
            {
                return None;
            }
            let integer_right = *context.registers.get(operands.left + 1)?;
            let third = *context.registers.get(operands.left + 2)?;
            *context.registers.get_mut(operands.dst + 1)? = integer_right;
            *context.registers.get_mut(operands.dst + 2)? = third;
            left
        }
        InstructionKind::ForIter => {
            if operands.left.checked_add(2)? >= context.registers.len() {
                return None;
            }
            let remaining = *context.registers.get(operands.left + 2)?;
            if remaining == 0 {
                return Some(Direction::Alternate);
            }
            let step = (*context.registers.get(operands.left + 1)?).cast_signed();
            *context.registers.get_mut(operands.left + 2)? = remaining - 1;
            if remaining > 1 {
                *context.registers.get_mut(operands.left)? = integer_left.wrapping_add(step).cast_unsigned();
            }
            left
        }
        InstructionKind::Index => {
            let view = view(context, operands, left, helpers)?;
            if matches!(view, View::Direct(_)) {
                if !helpers.read(context, operands) {
                    return None;
                }
                return Some(Direction::Next);
            }
            let length = i64::try_from(view.len()).ok()?;
            let index = if integer_right < 0 {
                integer_right.checked_add(length)?
            } else {
                integer_right
            };
            if index < 0 || index >= length {
                return None;
            }
            *helpers
                .buffer(usize::try_from(left).ok()?)?
                .get(usize::try_from(index).ok()?)?
        }
        InstructionKind::Read => {
            if !helpers.read(context, operands) {
                return None;
            }
            return Some(Direction::Next);
        }
        InstructionKind::Unpack => {
            let view = view(context, operands, left, helpers)?;
            let count = usize::try_from(operands.immediate >> 8).ok()?;
            if view.len() != count || !matches!(view, View::Buffered(_)) {
                return None;
            }
            let values = helpers.buffer(usize::try_from(left).ok()?)?;
            let destination = context
                .registers
                .get_mut(operands.dst..operands.dst.checked_add(count)?)?;
            for (dst, src) in destination.iter_mut().zip(values.iter().rev()) {
                *dst = *src;
            }
            return Some(Direction::Next);
        }
        InstructionKind::Bound => {
            if left == 0 {
                return None;
            }
            return Some(Direction::Next);
        }
        InstructionKind::SequenceIter => {
            if operands.dst.checked_add(2)? >= context.registers.len() {
                return None;
            }
            let view = view(context, operands, left, helpers)?;
            *context.registers.get_mut(operands.dst + 1)? = 0;
            *context.registers.get_mut(operands.dst + 2)? = view.len() as u64;
            left
        }
        InstructionKind::SequenceNext => {
            if operands.left.checked_add(2)? >= context.registers.len() || operands.right != operands.left + 1 {
                return None;
            }
            let index = *context.registers.get(operands.left + 1)?;
            let length = *context.registers.get(operands.left + 2)?;
            if index >= length {
                return Some(Direction::Alternate);
            }
            let view = view(context, operands, left, helpers)?;
            if index >= view.len() as u64 {
                return None;
            }
            let value = if matches!(view, View::Direct(_)) {
                if !helpers.read(context, operands) {
                    return None;
                }
                *context.registers.get(operands.dst)?
            } else {
                *helpers
                    .buffer(usize::try_from(left).ok()?)?
                    .get(usize::try_from(index).ok()?)?
            };
            *context.registers.get_mut(operands.left + 1)? = index + 1;
            value
        }
    };
    *context.registers.get_mut(operands.dst)? = value;
    Some(Direction::Next)
}

fn initialize_range(context: &mut Context<'_>, operands: &Operands) -> Option<()> {
    let count = usize::try_from(operands.immediate).ok()?;
    let first = context.registers.get(operands.left)?.cast_signed();
    let second = context.registers.get(operands.right)?.cast_signed();
    let third = context.registers.get(operands.third)?.cast_signed();
    let (start, stop, step) = match count {
        1 => (0, first, 1),
        2 => (first, second, 1),
        3 => (first, second, third),
        _ => return None,
    };
    if step == 0 {
        return None;
    }
    let remaining = Range::new(start, stop, step).len() as u64;
    context
        .registers
        .get_mut(operands.dst..operands.dst.checked_add(3)?)?
        .copy_from_slice(&[start.cast_unsigned(), step.cast_unsigned(), remaining]);
    Some(())
}
