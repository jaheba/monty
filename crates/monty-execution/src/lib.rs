//! Bytecode mechanisms independent of language values, opcodes and frame layouts.

/// Reads a fixed-width operand and advances the frame offset.
///
/// # Panics
/// Panics on an invalid offset or truncated operand without advancing the offset.
#[inline]
pub fn fetch<const N: usize>(bytecode: &[u8], ip: &mut usize) -> [u8; N] {
    let Some(bytes) = bytecode.get(*ip..).and_then(<[u8]>::first_chunk::<N>) else {
        unreachable!("instruction IP is out of bounds of the bytecode")
    };
    *ip += N;
    *bytes
}

/// Releases a frame or handler's stack region through language-owned cleanup.
///
/// `TOP_FIRST` selects reverse release order. The prefix and capacity are retained.
/// Cleanup must not execute interpreted code or suspend.
///
/// # Panics
/// Panics when `base` is beyond the stack.
#[inline]
pub fn release_stack<T, const TOP_FIRST: bool>(stack: &mut Vec<T>, base: usize, mut release: impl FnMut(T)) {
    assert!(base <= stack.len(), "stack region starts past the stack");
    let region = stack.drain(base..);
    if TOP_FIRST {
        region.rev().for_each(release);
    } else {
        region.for_each(&mut release);
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::{fetch, release_stack};

    #[test]
    fn mixed_operands_resume_at_the_next_byte() {
        let code = [7, 0x34, 0x12, 0xff, 0xff];
        let mut ip = 0;
        assert_eq!(fetch::<1>(&code, &mut ip), [7]);
        let saved_ip = ip;
        assert_eq!(u16::from_le_bytes(fetch(&code, &mut ip)), 0x1234);
        assert_eq!(i16::from_le_bytes(fetch(&code, &mut ip)), -1);
        assert_eq!(ip, code.len());
        ip = saved_ip;
        assert_eq!(fetch::<2>(&code, &mut ip), [0x34, 0x12]);
    }

    #[test]
    fn invalid_offsets_and_truncation_do_not_advance() {
        for initial in [1, 2, usize::MAX] {
            let mut ip = initial;
            assert!(catch_unwind(AssertUnwindSafe(|| fetch::<2>(&[0, 1], &mut ip))).is_err());
            assert_eq!(ip, initial);
        }
    }

    #[test]
    fn frame_cleanup_preserves_caller_and_release_order() {
        let mut stack = vec![0, 1, 2, 3];
        let capacity = stack.capacity();
        let mut released = Vec::new();
        release_stack::<_, true>(&mut stack, 2, |value| released.push(value));
        assert_eq!(stack, [0, 1]);
        assert_eq!(released, [3, 2]);
        assert_eq!(stack.capacity(), capacity);
        release_stack::<_, false>(&mut stack, 0, |value| released.push(value));
        assert_eq!(released, [3, 2, 0, 1]);
        assert_eq!(stack, []);
        release_stack::<_, true>(&mut stack, 0, |_| panic!("empty region"));
    }
}
