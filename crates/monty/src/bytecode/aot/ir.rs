//! Typed values and explicit control flow for effect-free function kernels.

use std::{
    collections::{BTreeSet, VecDeque},
    iter::repeat_n,
    ops::Range,
};

use super::{BuiltinHint, FunctionHints, TypeHint};
use crate::{
    builtins::BuiltinsFunctions,
    bytecode::{Code, Opcode},
    intern::StringId,
    types::Type,
    value::Value,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Unbound,
    Unknown,
    Int,
    Float,
    Bool,
    None,
    Range,
    Iterator,
    SequenceIterator(Element),
    Maybe(Element),
    TupleValue(Element, u16),
    Callable(BuiltinHint),
    List(Element),
    Tuple(Element),
    Sequence(Element),
    Dict(Element),
    Str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Element {
    Int = 1,
    Float = 2,
    Bool = 3,
    None = 4,
}
impl Element {
    fn hint(hint: &TypeHint) -> Option<Self> {
        match hint {
            TypeHint::Int => Some(Self::Int),
            TypeHint::Float => Some(Self::Float),
            TypeHint::Bool => Some(Self::Bool),
            TypeHint::None => Some(Self::None),
            _ => None,
        }
    }
    pub(crate) fn kind(self) -> Kind {
        match self {
            Self::Int => Kind::Int,
            Self::Float => Kind::Float,
            Self::Bool => Kind::Bool,
            Self::None => Kind::None,
        }
    }
}

impl Kind {
    fn hint(hint: &TypeHint) -> Self {
        match hint {
            TypeHint::Int => Self::Int,
            TypeHint::Float => Self::Float,
            TypeHint::Bool => Self::Bool,
            TypeHint::None => Self::None,
            TypeHint::Str => Self::Str,
            TypeHint::List(element) => Element::hint(element).map_or(Self::Unknown, Self::List),
            TypeHint::Tuple(element) => Element::hint(element).map_or(Self::Unknown, Self::Tuple),
            TypeHint::Dict { key, value } if **key == TypeHint::Str => {
                Element::hint(value).map_or(Self::Unknown, Self::Dict)
            }
            TypeHint::Union(choices) => {
                let mut element = None;
                for choice in choices {
                    let kind = Self::hint(choice);
                    let Some(current) = kind.sequence_element() else {
                        return Self::Unknown;
                    };
                    if element.is_some_and(|old| old != current) {
                        return Self::Unknown;
                    }
                    element = Some(current);
                }
                element.map_or(Self::Unknown, Self::Sequence)
            }
            _ => Self::Unknown,
        }
    }

    pub(crate) fn scalar(self) -> bool {
        matches!(self, Self::Int | Self::Float | Self::Bool | Self::None)
    }
    fn ordinary(self) -> bool {
        self.scalar() || self.reference() || matches!(self, Self::Range | Self::Maybe(_))
    }
    pub(crate) fn reference(self) -> bool {
        matches!(
            self,
            Self::List(_) | Self::Tuple(_) | Self::Sequence(_) | Self::Dict(_) | Self::Str
        )
    }
    pub(crate) fn sequence_element(self) -> Option<Element> {
        match self {
            Self::List(element) | Self::Tuple(element) | Self::Sequence(element) => Some(element),
            _ => None,
        }
    }

    fn join(self, other: Self) -> Self {
        if self == other {
            return self;
        }
        let maybe = |kind| match kind {
            Self::Int => Some(Element::Int),
            Self::Float => Some(Element::Float),
            Self::Bool => Some(Element::Bool),
            Self::None => Some(Element::None),
            Self::Maybe(element) => Some(element),
            _ => None,
        };
        match (self, other) {
            (Self::Unbound, kind) | (kind, Self::Unbound) => maybe(kind).map_or(Self::Unknown, Self::Maybe),
            (Self::Maybe(element), kind) | (kind, Self::Maybe(element)) if maybe(kind) == Some(element) => {
                Self::Maybe(element)
            }
            _ => Self::Unknown,
        }
    }

    pub(crate) fn numeric(self) -> bool {
        matches!(self, Self::Int | Self::Float | Self::Bool)
    }
}

#[derive(Debug, Clone)]
pub(crate) enum Operation {
    Copy {
        dst: usize,
        src: usize,
        kind: Kind,
    },
    Binary {
        dst: usize,
        left: usize,
        right: usize,
        op: Opcode,
        lhs: Kind,
        rhs: Kind,
    },
    Unary {
        dst: usize,
        src: usize,
        op: Opcode,
        kind: Kind,
    },
    Range {
        dst: usize,
        arguments: Vec<usize>,
    },
    Iter {
        dst: usize,
        src: usize,
        kind: Kind,
    },
    Length {
        dst: usize,
        object: usize,
        kind: Kind,
    },
    Index {
        dst: usize,
        object: usize,
        key: usize,
        kind: Kind,
    },
    Unpack {
        dst: usize,
        object: usize,
        count: usize,
        element: Element,
    },
    GuardBound {
        src: usize,
    },
    Bound {
        dst: usize,
        bound: bool,
    },
    Checkpoint,
}

#[derive(Debug, Clone)]
pub(crate) struct Instruction {
    pub ip: usize,
    pub weight: usize,
    pub operation: Operation,
}

#[derive(Debug, Clone)]
pub(crate) enum Terminator {
    Jump(usize),
    ForIter {
        iterator: usize,
        item: usize,
        element: Option<Element>,
        yes: usize,
        no: usize,
    },
    Branch {
        condition: usize,
        kind: Kind,
        yes: usize,
        no: usize,
    },
    Return {
        source: usize,
        kind: Kind,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct Block {
    pub instructions: Vec<Instruction>,
    pub terminal_ip: usize,
    pub terminal_weight: usize,
    pub terminator: Terminator,
}

#[derive(Debug)]
pub(crate) struct Plan {
    pub scalar_call_cost: Option<usize>,
    pub needs_views: bool,
    pub parameters: Vec<Kind>,
    pub builtin_globals: Vec<(usize, BuiltinHint)>,
    pub locals: usize,
    pub registers: usize,
    pub initial_values: Vec<(usize, u64)>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone)]
struct Decoded {
    ip: usize,
    next: usize,
    op: Opcode,
    argument: usize,
    target: Option<usize>,
    builtin: Option<BuiltinHint>,
}

impl Decoded {
    fn terminal(&self) -> bool {
        matches!(
            self.op,
            Opcode::Jump
                | Opcode::JumpIfFalse
                | Opcode::JumpIfTrue
                | Opcode::JumpIfFalseOrPop
                | Opcode::JumpIfTrueOrPop
                | Opcode::ForIter
                | Opcode::ReturnValue
        )
    }
}

#[derive(Clone, PartialEq, Eq)]
struct State {
    locals: Vec<Kind>,
    stack: Vec<Kind>,
}

fn binary_result(op: Opcode, left: Kind, right: Kind) -> Option<Kind> {
    use Opcode as O;
    if matches!(op, O::CompareEq | O::CompareNe) && left.scalar() && right.scalar() {
        return Some(Kind::Bool);
    }
    if matches!(op, O::CompareIs | O::CompareIsNot)
        && left.scalar()
        && right.scalar()
        && (left == Kind::None || right == Kind::None || (left == Kind::Bool && right == Kind::Bool))
    {
        return Some(Kind::Bool);
    }
    if !left.numeric() || !right.numeric() {
        return None;
    }
    match op {
        O::CompareEq | O::CompareNe | O::CompareLt | O::CompareLe | O::CompareGt | O::CompareGe => Some(Kind::Bool),
        O::BinaryAdd | O::InplaceAdd | O::BinarySub | O::InplaceSub | O::BinaryMul | O::InplaceMul => {
            Some(if left == Kind::Float || right == Kind::Float {
                Kind::Float
            } else {
                Kind::Int
            })
        }
        O::BinaryDiv | O::InplaceDiv => Some(Kind::Float),
        O::BinaryFloorDiv | O::InplaceFloorDiv | O::BinaryMod | O::InplaceMod
            if left != Kind::Float && right != Kind::Float =>
        {
            Some(Kind::Int)
        }
        O::BinaryAnd | O::InplaceAnd | O::BinaryOr | O::InplaceOr | O::BinaryXor | O::InplaceXor
            if left != Kind::Float && right != Kind::Float =>
        {
            Some(if left == Kind::Bool && right == Kind::Bool {
                Kind::Bool
            } else {
                Kind::Int
            })
        }
        _ => None,
    }
}

fn unary_result(op: Opcode, kind: Kind) -> Option<Kind> {
    match op {
        Opcode::UnaryNot if kind.scalar() => Some(Kind::Bool),
        Opcode::UnaryNeg | Opcode::UnaryPos if kind.numeric() => {
            Some(if kind == Kind::Bool { Kind::Int } else { kind })
        }
        Opcode::UnaryInvert if matches!(kind, Kind::Bool | Kind::Int) => Some(Kind::Int),
        _ => None,
    }
}

fn constant(code: &Code, instruction: &Decoded) -> Option<(Kind, u64)> {
    match instruction.op {
        Opcode::LoadSmallInt => Some((
            Kind::Int,
            i64::from(u8::try_from(instruction.argument).ok()?.cast_signed()).cast_unsigned(),
        )),
        Opcode::LoadTrue => Some((Kind::Bool, 1)),
        Opcode::LoadFalse => Some((Kind::Bool, 0)),
        Opcode::LoadNone => Some((Kind::None, 0)),
        Opcode::LoadConst => match code.constants().get(instruction.argument)? {
            Value::Int(value) => Some((Kind::Int, value.cast_unsigned())),
            Value::Float(value) => Some((Kind::Float, value.to_bits())),
            Value::Bool(value) => Some((Kind::Bool, u64::from(*value))),
            Value::None => Some((Kind::None, 0)),
            Value::InternString(_) => Some((
                Kind::Str,
                (code.aot_hints()?.parameters.len() + instruction.argument) as u64,
            )),
            _ => None,
        },
        _ => None,
    }
}

fn is_load_local(op: Opcode) -> bool {
    matches!(
        op,
        Opcode::LoadLocal0
            | Opcode::LoadLocal1
            | Opcode::LoadLocal2
            | Opcode::LoadLocal3
            | Opcode::LoadLocal
            | Opcode::LoadLocalW
    )
}

type LocalSet = [u64; 4];

// Only edge copies are eliminated. Discarded arithmetic and local loads can still
// raise Python errors, so their operations and bound guards must execute.
fn live_local_exits(decoded: &[Decoded], ranges: &[Range<usize>], starts: &[usize]) -> Option<Vec<LocalSet>> {
    let mut successors = Vec::with_capacity(ranges.len());
    for range in ranges {
        let last = decoded[range.clone()].last()?;
        let mut targets = Vec::with_capacity(2);
        if last.op != Opcode::ReturnValue {
            if let Some(target) = last.target {
                targets.push(starts.binary_search(&target).ok()?);
            }
            if last.op != Opcode::Jump {
                targets.push(starts.binary_search(&last.next).ok()?);
            }
        }
        successors.push(targets);
    }
    let mut entries = vec![[0; 4]; ranges.len()];
    let mut exits = entries.clone();
    for _ in 0..256 {
        let mut changed = false;
        for index in (0..ranges.len()).rev() {
            let mut live = [0; 4];
            for &target in &successors[index] {
                for (word, incoming) in live.iter_mut().zip(entries[target]) {
                    *word |= incoming;
                }
            }
            exits[index] = live;
            for instruction in decoded[ranges[index].clone()].iter().rev() {
                let reads = is_load_local(instruction.op);
                if reads || matches!(instruction.op, Opcode::StoreLocal | Opcode::StoreLocalW) {
                    let word = live.get_mut(instruction.argument / 64)?;
                    let bit = 1 << (instruction.argument % 64);
                    if reads {
                        *word |= bit;
                    } else {
                        *word &= !bit;
                    }
                }
            }
            changed |= live != entries[index];
            entries[index] = live;
        }
        if !changed {
            return Some(exits);
        }
    }
    None
}

fn transfer(code: &Code, instruction: &Decoded, state: &mut State) -> Option<()> {
    let op = instruction.op;
    if let Some((kind, _)) = constant(code, instruction) {
        state.stack.push(kind);
    } else if is_load_local(op) {
        let kind = *state.locals.get(instruction.argument)?;
        // An unbound load is observable even when its result is subsequently discarded.
        if !kind.ordinary() {
            return None;
        }
        state.stack.push(if let Kind::Maybe(element) = kind {
            element.kind()
        } else {
            kind
        });
    } else {
        match op {
            Opcode::StoreLocal | Opcode::StoreLocalW => {
                let kind = state.stack.pop()?;
                if !kind.ordinary() {
                    return None;
                }
                *state.locals.get_mut(instruction.argument)? = kind;
            }
            Opcode::LoadGlobalCallable => state.stack.push(Kind::Callable(instruction.builtin?)),
            Opcode::CallBuiltinType | Opcode::CallBuiltinFunction | Opcode::CallFunction => {
                let start = state.stack.len().checked_sub(instruction.argument)?;
                let arguments = state.stack.drain(start..).collect::<Vec<_>>();
                let builtin = if op == Opcode::CallFunction {
                    let Kind::Callable(builtin) = state.stack.pop()? else {
                        return None;
                    };
                    builtin
                } else {
                    instruction.builtin?
                };
                let result = match builtin {
                    BuiltinHint::Range => {
                        if !(1..=3).contains(&arguments.len())
                            || arguments.iter().any(|kind| !matches!(kind, Kind::Int | Kind::Bool))
                        {
                            return None;
                        }
                        Kind::Range
                    }
                    BuiltinHint::Len => {
                        if arguments.len() != 1
                            || !(arguments[0].sequence_element().is_some() || matches!(arguments[0], Kind::Dict(_)))
                        {
                            return None;
                        }
                        Kind::Int
                    }
                };
                state.stack.push(result);
            }
            Opcode::BinarySubscr => {
                let key = state.stack.pop()?;
                let object = state.stack.pop()?;
                let element = if let Some(element) = object.sequence_element() {
                    if !matches!(key, Kind::Int | Kind::Bool) {
                        return None;
                    }
                    element
                } else if let Kind::Dict(element) = object {
                    if key != Kind::Str {
                        return None;
                    }
                    element
                } else {
                    return None;
                };
                state.stack.push(element.kind());
            }
            Opcode::BuildTuple => {
                let start = state.stack.len().checked_sub(instruction.argument)?;
                let items = state.stack.drain(start..).collect::<Vec<_>>();
                let element = match items.first().copied().unwrap_or(Kind::None) {
                    Kind::Int => Element::Int,
                    Kind::Float => Element::Float,
                    Kind::Bool => Element::Bool,
                    Kind::None => Element::None,
                    _ => return None,
                };
                if items.iter().any(|kind| *kind != element.kind()) {
                    return None;
                }
                state
                    .stack
                    .push(Kind::TupleValue(element, u16::try_from(items.len()).ok()?));
            }
            Opcode::UnpackSequence => {
                let value = state.stack.pop()?;
                let element = match value {
                    Kind::TupleValue(element, count) if usize::from(count) == instruction.argument => element,
                    _ => value.sequence_element()?,
                };
                if instruction.argument > 256 {
                    return None;
                }
                state.stack.extend(repeat_n(element.kind(), instruction.argument));
            }
            Opcode::GetIter => {
                let kind = state.stack.pop()?;
                let iterator = if kind == Kind::Range {
                    Kind::Iterator
                } else {
                    Kind::SequenceIterator(kind.sequence_element()?)
                };
                state.stack.push(iterator);
            }
            Opcode::ForIter => {
                if !matches!(state.stack.last()?, Kind::Iterator | Kind::SequenceIterator(_)) {
                    return None;
                }
            }
            Opcode::Pop => {
                state.stack.pop()?;
            }
            Opcode::Dup => {
                let kind = *state.stack.last()?;
                if !kind.ordinary() {
                    return None;
                }
                state.stack.push(kind);
            }
            Opcode::Rot2 => {
                let n = state.stack.len().checked_sub(2)?;
                state.stack.swap(n, n + 1);
            }
            Opcode::Nop | Opcode::Jump => {}
            Opcode::JumpIfTrue | Opcode::JumpIfFalse | Opcode::JumpIfTrueOrPop | Opcode::JumpIfFalseOrPop => {
                if !state.stack.last()?.scalar() {
                    return None;
                }
            }
            Opcode::ReturnValue => {
                if !state.stack.last()?.scalar() && !matches!(state.stack.last()?, Kind::TupleValue(_, _)) {
                    return None;
                }
            }
            Opcode::UnaryNeg | Opcode::UnaryPos | Opcode::UnaryNot | Opcode::UnaryInvert => {
                let kind = state.stack.pop()?;
                state.stack.push(unary_result(op, kind)?);
            }
            _ => {
                let right = state.stack.pop()?;
                let left = state.stack.pop()?;
                state.stack.push(binary_result(op, left, right)?);
            }
        }
    }
    (state.stack.len() <= 256).then_some(())
}

impl Plan {
    pub(crate) fn compile(code: &Code, hints: &FunctionHints) -> Option<Self> {
        if cfg!(feature = "experimental-fusion")
            || code.has_exception_handlers()
            || code.bytecode().len() > 16_384
            || hints.locals > 256
            || hints.parameters.len() > usize::from(hints.locals)
        {
            return None;
        }
        let bytes = code.bytecode();
        let mut decoded = Vec::new();
        let mut starts = BTreeSet::from([0]);
        let mut builtin_globals = Vec::new();
        let mut ip = 0;
        while ip < bytes.len() {
            let op = Opcode::from_repr(bytes[ip])?;
            // Reject unsupported/effectful bodies before building control-flow state.
            if !matches!(
                op,
                Opcode::LoadConst
                    | Opcode::LoadNone
                    | Opcode::LoadTrue
                    | Opcode::LoadFalse
                    | Opcode::LoadSmallInt
                    | Opcode::LoadLocal0
                    | Opcode::LoadLocal1
                    | Opcode::LoadLocal2
                    | Opcode::LoadLocal3
                    | Opcode::LoadLocal
                    | Opcode::LoadLocalW
                    | Opcode::StoreLocal
                    | Opcode::StoreLocalW
                    | Opcode::LoadGlobalCallable
                    | Opcode::CallFunction
                    | Opcode::CallBuiltinType
                    | Opcode::CallBuiltinFunction
                    | Opcode::BinarySubscr
                    | Opcode::BuildTuple
                    | Opcode::UnpackSequence
                    | Opcode::GetIter
                    | Opcode::ForIter
                    | Opcode::Pop
                    | Opcode::Dup
                    | Opcode::Rot2
                    | Opcode::Nop
                    | Opcode::Jump
                    | Opcode::ReturnValue
                    | Opcode::JumpIfTrue
                    | Opcode::JumpIfFalse
                    | Opcode::JumpIfTrueOrPop
                    | Opcode::JumpIfFalseOrPop
            ) && binary_result(op, Kind::Int, Kind::Int).is_none()
                && binary_result(op, Kind::Bool, Kind::Bool).is_none()
                && unary_result(op, Kind::Int).is_none()
            {
                return None;
            }
            let next = ip + op.encoded_len(bytes, ip)?;
            let u16_arg = || usize::from(u16::from_le_bytes([bytes[ip + 1], bytes[ip + 2]]));
            if op == Opcode::BuildTuple
                && !matches!(
                    bytes.get(next).and_then(|byte| Opcode::from_repr(*byte)),
                    Some(Opcode::ReturnValue | Opcode::UnpackSequence)
                )
            {
                return None;
            }
            let mut builtin = None;
            let argument = match op {
                Opcode::LoadLocal0 => 0,
                Opcode::LoadLocal1 => 1,
                Opcode::LoadLocal2 => 2,
                Opcode::LoadLocal3 => 3,
                Opcode::LoadLocal | Opcode::StoreLocal | Opcode::LoadSmallInt => usize::from(bytes[ip + 1]),
                Opcode::LoadLocalW | Opcode::StoreLocalW | Opcode::LoadConst | Opcode::BuildTuple => u16_arg(),
                Opcode::UnpackSequence => usize::from(bytes[ip + 1]),
                Opcode::LoadGlobalCallable => {
                    let id = StringId::from_index(u16::from_le_bytes([bytes[ip + 3], bytes[ip + 4]]));
                    let hint = hints.builtin_names.iter().find(|(name, _)| *name == id)?.1;
                    builtin = Some(hint);
                    builtin_globals.push((u16_arg(), hint));
                    u16_arg()
                }
                Opcode::CallFunction => usize::from(bytes[ip + 1]),
                Opcode::CallBuiltinType => {
                    if Type::callable_from_u8(bytes[ip + 1])? != Type::Range {
                        return None;
                    }
                    builtin = Some(BuiltinHint::Range);
                    usize::from(bytes[ip + 2])
                }
                Opcode::CallBuiltinFunction => {
                    if BuiltinsFunctions::from_repr(bytes[ip + 1])? != BuiltinsFunctions::Len {
                        return None;
                    }
                    builtin = Some(BuiltinHint::Len);
                    usize::from(bytes[ip + 2])
                }
                _ => 0,
            };
            let target = if matches!(
                op,
                Opcode::Jump
                    | Opcode::JumpIfTrue
                    | Opcode::JumpIfFalse
                    | Opcode::JumpIfTrueOrPop
                    | Opcode::JumpIfFalseOrPop
                    | Opcode::ForIter
            ) {
                let offset = i16::from_le_bytes([bytes[ip + 1], bytes[ip + 2]]);
                let target = next.checked_add_signed(isize::from(offset))?;
                starts.insert(target);
                Some(target)
            } else {
                None
            };
            let instruction = Decoded {
                ip,
                next,
                op,
                argument,
                target,
                builtin,
            };
            if instruction.terminal() && next < bytes.len() {
                starts.insert(next);
            }
            decoded.push(instruction);
            ip = next;
        }
        if decoded.is_empty() || starts.len() > 256 {
            return None;
        }
        let starts = starts.into_iter().collect::<Vec<_>>();
        let mut ranges = Vec::new();
        for (index, &start) in starts.iter().enumerate() {
            let first = decoded.binary_search_by_key(&start, |i| i.ip).ok()?;
            let end = match starts.get(index + 1) {
                Some(next) => decoded.binary_search_by_key(next, |i| i.ip).ok()?,
                None => decoded.len(),
            };
            ranges.push(first..end);
        }
        let parameters = hints.parameters.iter().map(Kind::hint).collect::<Vec<_>>();
        let locals = usize::from(hints.locals);
        let mut entry = State {
            locals: vec![Kind::Unbound; locals],
            stack: Vec::new(),
        };
        entry.locals[..parameters.len()].copy_from_slice(&parameters);
        let mut states = vec![None; ranges.len()];
        states[0] = Some(entry);
        let mut queue = VecDeque::from([0]);
        let mut visits = 0;
        let mut failed = vec![false; ranges.len()];
        let mut max_stack = 0;
        while let Some(index) = queue.pop_front() {
            visits += 1;
            if visits > 65_536 {
                return None;
            }
            let mut state = states[index].clone()?;
            max_stack = max_stack.max(state.stack.len());
            let instructions = &decoded[ranges[index].clone()];
            failed[index] = false;
            for instruction in instructions {
                if transfer(code, instruction, &mut state).is_none() {
                    failed[index] = true;
                    break;
                }
                max_stack = max_stack.max(state.stack.len());
            }
            if failed[index] {
                continue;
            }
            let last = instructions.last()?;
            let mut successors = Vec::new();
            match last.op {
                Opcode::ReturnValue => {}
                Opcode::ForIter => {
                    let mut exhausted = state.clone();
                    exhausted.stack.pop()?;
                    successors.push((last.target?, exhausted));
                    let element = match state.stack.last()? {
                        Kind::Iterator => Kind::Int,
                        Kind::SequenceIterator(element) => element.kind(),
                        _ => return None,
                    };
                    state.stack.push(element);
                    successors.push((last.next, state));
                }
                Opcode::Jump => successors.push((last.target?, state)),
                Opcode::JumpIfTrue | Opcode::JumpIfFalse => {
                    state.stack.pop()?;
                    successors.push((last.target?, state.clone()));
                    successors.push((last.next, state));
                }
                Opcode::JumpIfTrueOrPop | Opcode::JumpIfFalseOrPop => {
                    successors.push((last.target?, state.clone()));
                    state.stack.pop()?;
                    successors.push((last.next, state));
                }
                _ => successors.push((last.next, state)),
            }
            for (target, incoming) in successors {
                let target = starts.binary_search(&target).ok()?;
                let changed = match &mut states[target] {
                    None => {
                        states[target] = Some(incoming);
                        true
                    }
                    Some(old) => {
                        if old.stack.len() != incoming.stack.len() {
                            return None;
                        }
                        let mut changed = false;
                        for (old, new) in old
                            .locals
                            .iter_mut()
                            .chain(&mut old.stack)
                            .zip(incoming.locals.iter().chain(&incoming.stack))
                        {
                            let joined = old.join(*new);
                            if joined != *old {
                                *old = joined;
                                changed = true;
                            }
                        }
                        changed
                    }
                };
                if changed {
                    queue.push_back(target);
                }
            }
        }
        if failed.iter().any(|&failed| failed) {
            return None;
        }
        let live_exits = if ranges.len() > 1 {
            Some(live_local_exits(&decoded, &ranges, &starts)?)
        } else {
            None
        };
        let mut plan = Self {
            scalar_call_cost: None,
            needs_views: false,
            parameters,
            builtin_globals,
            locals,
            registers: 3 * (locals + max_stack),
            initial_values: Vec::new(),
            blocks: Vec::new(),
        };
        let mut block_map = vec![None; ranges.len()];
        for (index, state) in states.iter().enumerate() {
            if state.is_some() {
                block_map[index] = Some(plan.blocks.len());
                plan.blocks.push(Block {
                    instructions: Vec::new(),
                    terminal_ip: 0,
                    terminal_weight: 0,
                    terminator: Terminator::Jump(0),
                });
            }
        }
        for (index, range) in ranges.into_iter().enumerate() {
            let Some(mut state) = states[index].clone() else {
                continue;
            };
            let mut local_values = (0..locals).map(|i| 3 * i).collect::<Vec<_>>();
            let mut stack = (locals..locals + state.stack.len()).map(|i| 3 * i).collect::<Vec<_>>();
            let mut output = Vec::new();
            let mut pending = 0;
            let instructions = &decoded[range];
            let last = instructions.last()?;
            for instruction in instructions {
                let op = instruction.op;
                if instruction.terminal() {
                    break;
                }
                pending += 1;
                if let Some((_, bits)) = constant(code, instruction) {
                    let dst = plan.fresh()?;
                    plan.initial_values.push((dst, bits));
                    stack.push(dst);
                } else if is_load_local(op) {
                    let src = *local_values.get(instruction.argument)?;
                    if matches!(state.locals[instruction.argument], Kind::Maybe(_)) {
                        output.push(Instruction {
                            ip: instruction.ip,
                            weight: pending,
                            operation: Operation::GuardBound { src: src + 1 },
                        });
                        pending = 0;
                    }
                    stack.push(src);
                } else {
                    let operation = match op {
                        Opcode::StoreLocal | Opcode::StoreLocalW => {
                            local_values[instruction.argument] = stack.pop()?;
                            None
                        }
                        Opcode::LoadGlobalCallable => {
                            let dst = plan.fresh()?;
                            plan.initial_values.push((dst, 0));
                            stack.push(dst);
                            None
                        }
                        Opcode::CallBuiltinType | Opcode::CallBuiltinFunction | Opcode::CallFunction => {
                            let start = stack.len().checked_sub(instruction.argument)?;
                            let arguments = stack.drain(start..).collect();
                            if op == Opcode::CallFunction {
                                stack.pop()?;
                            }
                            let builtin = if op == Opcode::CallFunction {
                                let Kind::Callable(builtin) = state.stack[state.stack.len() - instruction.argument - 1]
                                else {
                                    return None;
                                };
                                builtin
                            } else {
                                instruction.builtin?
                            };
                            match builtin {
                                BuiltinHint::Range => {
                                    let dst = plan.fresh_range()?;
                                    stack.push(dst);
                                    Some(Operation::Range { dst, arguments })
                                }
                                BuiltinHint::Len => {
                                    let dst = plan.fresh()?;
                                    stack.push(dst);
                                    Some(Operation::Length {
                                        dst,
                                        object: arguments[0],
                                        kind: *state.stack.last()?,
                                    })
                                }
                            }
                        }
                        Opcode::BinarySubscr => {
                            let key = stack.pop()?;
                            let object = stack.pop()?;
                            let dst = plan.fresh()?;
                            stack.push(dst);
                            plan.needs_views |= state.stack[state.stack.len() - 2].sequence_element().is_some();
                            Some(Operation::Index {
                                dst,
                                object,
                                key,
                                kind: state.stack[state.stack.len() - 2],
                            })
                        }
                        Opcode::BuildTuple => {
                            let start = stack.len().checked_sub(instruction.argument)?;
                            let values = stack.drain(start..).collect::<Vec<_>>();
                            let kind = state.stack.get(start).copied().unwrap_or(Kind::None);
                            let dst = plan.registers;
                            for src in values {
                                let target = plan.fresh()?;
                                output.push(Instruction {
                                    ip: instruction.ip,
                                    weight: pending,
                                    operation: Operation::Copy { dst: target, src, kind },
                                });
                                pending = 0;
                            }
                            if instruction.argument == 0 {
                                plan.fresh()?;
                            }
                            stack.push(dst);
                            None
                        }
                        Opcode::UnpackSequence => {
                            let object = stack.pop()?;
                            if let Kind::TupleValue(_, count) = *state.stack.last()? {
                                // A private tuple immediately unpacked needs no Python allocation.
                                stack.extend((0..usize::from(count)).rev().map(|index| object + index));
                                None
                            } else {
                                plan.needs_views = true;
                                let dst = plan.registers;
                                let element = state.stack.last()?.sequence_element()?;
                                for _ in 0..instruction.argument {
                                    stack.push(plan.fresh()?);
                                }
                                Some(Operation::Unpack {
                                    dst,
                                    object,
                                    count: instruction.argument,
                                    element,
                                })
                            }
                        }
                        Opcode::GetIter => {
                            let src = stack.pop()?;
                            let dst = plan.fresh_range()?;
                            stack.push(dst);
                            let kind = *state.stack.last()?;
                            plan.needs_views |= kind.sequence_element().is_some();
                            Some(Operation::Iter { dst, src, kind })
                        }
                        Opcode::Pop => {
                            stack.pop()?;
                            None
                        }
                        Opcode::Dup => {
                            stack.push(*stack.last()?);
                            None
                        }
                        Opcode::Rot2 => {
                            let n = stack.len().checked_sub(2)?;
                            stack.swap(n, n + 1);
                            None
                        }
                        Opcode::Nop => None,
                        Opcode::UnaryNeg | Opcode::UnaryPos | Opcode::UnaryNot | Opcode::UnaryInvert => {
                            let src = stack.pop()?;
                            let dst = plan.fresh()?;
                            stack.push(dst);
                            Some(Operation::Unary {
                                dst,
                                src,
                                op,
                                kind: *state.stack.last()?,
                            })
                        }
                        _ => {
                            let right = stack.pop()?;
                            let left = stack.pop()?;
                            let dst = plan.fresh()?;
                            stack.push(dst);
                            let n = state.stack.len();
                            Some(Operation::Binary {
                                dst,
                                left,
                                right,
                                op,
                                lhs: *state.stack.get(n.checked_sub(2)?)?,
                                rhs: state.stack[n - 1],
                            })
                        }
                    };
                    if let Some(operation) = operation {
                        output.push(Instruction {
                            ip: instruction.ip,
                            weight: pending,
                            operation,
                        });
                        pending = 0;
                    }
                }
                transfer(code, instruction, &mut state)?;
                if pending >= 64 {
                    output.push(Instruction {
                        ip: instruction.ip,
                        weight: pending,
                        operation: Operation::Checkpoint,
                    });
                    pending = 0;
                }
            }
            let target = |ip: usize| block_map[starts.binary_search(&ip).ok()?];
            let terminator = if last.op == Opcode::ReturnValue {
                Terminator::Return {
                    source: *stack.last()?,
                    kind: *state.stack.last()?,
                }
            } else {
                // Parallel edge copies preserve old local values used by other destinations.
                let copies = local_values
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| {
                        state.locals[*i].ordinary()
                            && live_exits
                                .as_ref()
                                .is_none_or(|exits| exits[index][i / 64] & (1 << (i % 64)) != 0)
                    })
                    .map(|(i, &src)| (3 * i, src, state.locals[i]))
                    .chain(
                        stack
                            .iter()
                            .enumerate()
                            .map(|(i, &src)| (3 * (locals + i), src, state.stack[i])),
                    )
                    .filter(|(dst, src, _)| dst != src)
                    .collect::<Vec<_>>();
                let destinations = copies.iter().map(|(dst, _, _)| *dst).collect::<BTreeSet<_>>();
                let mut safe = Vec::new();
                for (dst, src, kind) in copies {
                    let src = if destinations.contains(&src) {
                        let temp = if matches!(
                            kind,
                            Kind::Range | Kind::Iterator | Kind::SequenceIterator(_) | Kind::Maybe(_)
                        ) {
                            plan.fresh_range()?
                        } else {
                            plan.fresh()?
                        };
                        output.push(Instruction {
                            ip: last.ip,
                            weight: 0,
                            operation: Operation::Copy { dst: temp, src, kind },
                        });
                        temp
                    } else {
                        src
                    };
                    safe.push((dst, src, kind));
                }
                for (dst, src, kind) in safe {
                    output.push(Instruction {
                        ip: last.ip,
                        weight: 0,
                        operation: Operation::Copy { dst, src, kind },
                    });
                }
                for (index, kind) in state.locals.iter().enumerate() {
                    let bound = if *kind == Kind::Unbound {
                        Some(false)
                    } else if kind.scalar() || kind.reference() {
                        Some(true)
                    } else {
                        None
                    };
                    if let Some(bound) = bound {
                        output.push(Instruction {
                            ip: last.ip,
                            weight: 0,
                            operation: Operation::Bound {
                                dst: 3 * index + 1,
                                bound,
                            },
                        });
                    }
                }
                match last.op {
                    Opcode::Jump => Terminator::Jump(target(last.target?)?),
                    Opcode::ForIter => Terminator::ForIter {
                        iterator: 3 * (locals + stack.len() - 1),
                        item: 3 * (locals + stack.len()),
                        element: match state.stack.last()? {
                            Kind::Iterator => None,
                            Kind::SequenceIterator(element) => Some(*element),
                            _ => return None,
                        },
                        yes: target(last.next)?,
                        no: target(last.target?)?,
                    },
                    Opcode::JumpIfTrue | Opcode::JumpIfFalse | Opcode::JumpIfTrueOrPop | Opcode::JumpIfFalseOrPop => {
                        let jump = target(last.target?)?;
                        let next = target(last.next)?;
                        let (yes, no) = if matches!(last.op, Opcode::JumpIfTrue | Opcode::JumpIfTrueOrPop) {
                            (jump, next)
                        } else {
                            (next, jump)
                        };
                        Terminator::Branch {
                            condition: 3 * (locals + stack.len() - 1),
                            kind: *state.stack.last()?,
                            yes,
                            no,
                        }
                    }
                    _ => Terminator::Jump(target(last.next)?),
                }
            };
            let terminal_weight = pending + usize::from(last.terminal() && last.op != Opcode::ReturnValue);
            plan.blocks[block_map[index]?] = Block {
                instructions: output,
                terminal_ip: last.ip,
                terminal_weight,
                terminator,
            };
        }
        let bound_flags = plan
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| {
                if let Operation::GuardBound { src } = instruction.operation {
                    Some(src)
                } else {
                    None
                }
            })
            .collect::<BTreeSet<_>>();
        for block in &mut plan.blocks {
            block.instructions.retain(
                |instruction| !matches!(instruction.operation,Operation::Bound{dst,..} if !bound_flags.contains(&dst)),
            );
        }
        if plan.blocks.len() == 1 && plan.parameters.iter().all(|kind| kind.scalar()) {
            let block = &plan.blocks[0];
            let scalar = matches!(block.terminator,Terminator::Return{kind,..} if kind.scalar())
                && block.instructions.iter().all(|instruction| {
                    matches!(instruction.operation,
                    Operation::Copy{kind,..}|Operation::Unary{kind,..} if kind.scalar())
                        || matches!(instruction.operation,Operation::Binary{lhs,rhs,..} if lhs.scalar()&&rhs.scalar())
                        || matches!(
                            instruction.operation,
                            Operation::Bound { .. } | Operation::GuardBound { .. } | Operation::Checkpoint
                        )
                });
            let cost = block
                .instructions
                .iter()
                .map(|instruction| instruction.weight)
                .sum::<usize>()
                + block.terminal_weight
                + 1;
            if scalar && cost <= 255 {
                plan.scalar_call_cost = Some(cost);
            }
        }
        Some(plan)
    }

    fn fresh_range(&mut self) -> Option<usize> {
        let first = self.fresh()?;
        self.fresh()?;
        self.fresh()?;
        Some(first)
    }

    fn fresh(&mut self) -> Option<usize> {
        let register = self.registers;
        self.registers = self.registers.checked_add(1).filter(|&n| n <= 8192)?;
        Some(register)
    }
}
