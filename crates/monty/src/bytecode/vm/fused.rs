//! Integer fast paths for the focused fusion experiment.

use super::{PyTrait, RunResult, VM, Value};

#[expect(clippy::inline_always, reason = "Fusion must avoid an additional call per dispatch.")]
impl VM<'_> {
    #[inline(always)]
    pub(super) fn fused_0(&mut self, countdown: &mut u8, limits: bool, print: bool) -> RunResult<()> {
        self.fused_local_add::<0, 1>(countdown, limits, print)
    }

    #[inline(always)]
    pub(super) fn fused_1(&mut self, countdown: &mut u8, limits: bool, print: bool) -> RunResult<()> {
        self.fused_local_add::<1, 2>(countdown, limits, print)
    }

    #[inline(always)]
    pub(super) fn fused_2(&mut self, countdown: &mut u8, limits: bool, print: bool) -> RunResult<()> {
        self.fused_small_int::<0>(countdown, limits, print)
    }

    #[inline(always)]
    pub(super) fn fused_3(&mut self, countdown: &mut u8, limits: bool, print: bool) -> RunResult<()> {
        self.fused_small_int::<1>(countdown, limits, print)
    }

    #[inline(always)]
    pub(super) fn fused_4(&mut self, countdown: &mut u8, limits: bool, print: bool) -> RunResult<()> {
        self.fused_small_int::<2>(countdown, limits, print)
    }

    #[inline(always)]
    pub(super) fn fused_5(&mut self, countdown: &mut u8, limits: bool, print: bool) -> RunResult<()> {
        self.fused_small_int::<3>(countdown, limits, print)
    }

    #[inline(always)]
    pub(super) fn fused_6(&mut self, countdown: &mut u8, limits: bool, print: bool) -> RunResult<()> {
        self.fused_compare_branch::<false>(countdown, limits, print)
    }

    #[inline(always)]
    pub(super) fn fused_7(&mut self, countdown: &mut u8, limits: bool, print: bool) -> RunResult<()> {
        self.fused_compare_branch::<true>(countdown, limits, print)
    }

    #[inline(always)]
    fn fused_local_add<const A: u16, const B: u16>(
        &mut self,
        countdown: &mut u8,
        limits: bool,
        print: bool,
    ) -> RunResult<()> {
        // Batch accounting only when no constituent crosses a checkpoint.
        let base = self.current_frame.stack_base();
        if *countdown >= 2
            && let (Value::Int(a), Value::Int(b)) = (&self.stack[base + A as usize], &self.stack[base + B as usize])
            && let Some(result) = a.checked_add(*b)
        {
            *countdown -= 2;
            self.instruction_ip = self.current_frame.ip + 1;
            self.current_frame.ip += 2;
            self.push(Value::Int(result));
            return Ok(());
        }

        // Undefined locals, other types and overflow keep the ordinary error/ownership paths.
        self.load_local(A)?;
        self.fused_checkpoint(countdown, limits, print)?;
        self.instruction_ip = self.current_frame.ip;
        self.current_frame.ip += 1;
        self.load_local(B)?;
        self.fused_checkpoint(countdown, limits, print)?;
        self.instruction_ip = self.current_frame.ip;
        self.current_frame.ip += 1;
        self.binary_add()
    }

    #[inline(always)]
    fn fused_small_int<const OP: u8>(&mut self, countdown: &mut u8, limits: bool, print: bool) -> RunResult<()> {
        let rhs = i64::from(self.current_frame.fetch_i8());
        let top = self.stack.len() - 1;
        if *countdown >= 1
            && let Value::Int(lhs) = &self.stack[top]
        {
            let result = match OP {
                0 => lhs.checked_add(rhs),
                1 => lhs.checked_sub(rhs),
                2 => lhs.checked_mul(rhs),
                // Python's remainder has the divisor's sign; checked_rem uses the dividend's.
                3 => lhs.checked_rem(rhs).and_then(|remainder| {
                    if remainder != 0 && (remainder < 0) != (rhs < 0) {
                        remainder.checked_add(rhs)
                    } else {
                        Some(remainder)
                    }
                }),
                _ => unreachable!(),
            };
            if let Some(result) = result {
                *countdown -= 1;
                self.instruction_ip = self.current_frame.ip;
                self.current_frame.ip += 1;
                self.stack[top] = Value::Int(result);
                return Ok(());
            }
        }

        // Bool, float, bigint and sequence operands use the existing binary methods.
        self.push(Value::Int(rhs));
        self.fused_checkpoint(countdown, limits, print)?;
        self.instruction_ip = self.current_frame.ip;
        self.current_frame.ip += 1;
        match OP {
            0 => self.binary_add(),
            1 => self.binary_sub(),
            2 => self.binary_mult(),
            3 => self.binary_mod(),
            _ => unreachable!(),
        }
    }

    #[inline(always)]
    fn fused_compare_branch<const EQ: bool>(&mut self, countdown: &mut u8, limits: bool, print: bool) -> RunResult<()> {
        let len = self.stack.len();
        if *countdown >= 1
            && let (Value::Int(lhs), Value::Int(rhs)) = (&self.stack[len - 2], &self.stack[len - 1])
        {
            let condition = if EQ { lhs == rhs } else { lhs < rhs };
            // Only immediate integers reach this truncation; no heap references need releasing.
            self.stack.truncate(len - 2);
            *countdown -= 1;
            self.instruction_ip = self.current_frame.ip;
            self.current_frame.ip += 1;
            let offset = self.current_frame.fetch_i16();
            if !condition {
                jump_relative!(self.current_frame.ip, offset);
            }
            return Ok(());
        }

        // Equality may return an arbitrary object; its truth conversion belongs to the branch.
        if EQ {
            self.compare_eq()?;
        } else {
            self.compare_lt()?;
        }
        self.fused_checkpoint(countdown, limits, print)?;
        self.instruction_ip = self.current_frame.ip;
        self.current_frame.ip += 1;
        let offset = self.current_frame.fetch_i16();
        let cond = self.pop();
        let result = cond.py_bool(self);
        cond.drop_with(self);
        if !result? {
            jump_relative!(self.current_frame.ip, offset);
        }
        Ok(())
    }

    #[inline]
    fn fused_checkpoint(&mut self, countdown: &mut u8, limits: bool, print: bool) -> RunResult<()> {
        if let Some(c) = countdown.checked_sub(1) {
            *countdown = c;
        } else {
            *countdown = u8::MAX;
            self.dispatch_checkpoint(limits, print)?;
        }
        Ok(())
    }
}
