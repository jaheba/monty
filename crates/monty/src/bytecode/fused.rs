//! Peephole matching for experimental fused instructions.

use super::op::Opcode;

const PATTERNS: &[(Opcode, &[Opcode])] = &[
    (
        Opcode::FusedLoadLocal0LoadLocal1BinaryAdd,
        &[Opcode::LoadLocal0, Opcode::LoadLocal1, Opcode::BinaryAdd],
    ),
    (
        Opcode::FusedLoadLocal1LoadLocal2BinaryAdd,
        &[Opcode::LoadLocal1, Opcode::LoadLocal2, Opcode::BinaryAdd],
    ),
    (
        Opcode::FusedLoadSmallIntBinaryAdd,
        &[Opcode::LoadSmallInt, Opcode::BinaryAdd],
    ),
    (
        Opcode::FusedLoadSmallIntBinarySub,
        &[Opcode::LoadSmallInt, Opcode::BinarySub],
    ),
    (
        Opcode::FusedLoadSmallIntBinaryMul,
        &[Opcode::LoadSmallInt, Opcode::BinaryMul],
    ),
    (
        Opcode::FusedLoadSmallIntBinaryMod,
        &[Opcode::LoadSmallInt, Opcode::BinaryMod],
    ),
    (
        Opcode::FusedCompareLtJumpIfFalse,
        &[Opcode::CompareLt, Opcode::JumpIfFalse],
    ),
    (
        Opcode::FusedCompareEqJumpIfFalse,
        &[Opcode::CompareEq, Opcode::JumpIfFalse],
    ),
];

/// Replace only the first opcode; retained bytes keep all original offsets valid.
pub(super) fn fuse(bytecode: &mut [u8], offsets: &[usize]) {
    let mut index = 0;
    while index < offsets.len() {
        let best = PATTERNS
            .iter()
            .filter(|(_, ops)| {
                index + ops.len() <= offsets.len()
                    && ops
                        .iter()
                        .enumerate()
                        .all(|(i, op)| bytecode[offsets[index + i]] == *op as u8)
            })
            .max_by_key(|(_, ops)| ops.len());
        if let Some((fused, ops)) = best {
            bytecode[offsets[index]] = *fused as u8;
            index += ops.len();
        } else {
            index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_preserves_operands_and_interior_entry_points() {
        for (fused, ops) in PATTERNS {
            let mut bytes = Vec::new();
            let mut offsets = Vec::new();
            for op in *ops {
                offsets.push(bytes.len());
                bytes.push(*op as u8);
                match op {
                    Opcode::LoadLocal | Opcode::LoadSmallInt | Opcode::StoreLocal => bytes.push(9),
                    Opcode::JumpIfTrue | Opcode::JumpIfFalse => bytes.extend([0, 0]),
                    _ => {}
                }
            }
            let original = bytes.clone();
            fuse(&mut bytes, &offsets);
            assert_eq!(bytes[0], *fused as u8);
            assert_eq!(bytes[1..], original[1..]);
        }
    }
}
