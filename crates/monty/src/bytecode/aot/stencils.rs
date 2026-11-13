//! Offline templates use unboxed registers and immutable typed operands.
#![allow(dead_code, unsafe_op_in_unsafe_fn)]
mod abi;
use abi::*;

#[inline(always)]
unsafe fn view(c: &mut Context, p: &Patch, source: u64) -> Option<View> {
    let index = source as usize;
    if index >= c.views_len {
        return None;
    }
    if c.views.add(index).read().prepared == 0 && !(c.prepare_view)(c, p) {
        return None;
    }
    let view = c.views.add(index).read();
    (view.prepared == 1 || view.prepared == 2).then_some(view)
}

#[inline(always)]
unsafe fn execute<const OP: u8>(c: &mut Context, p: &Patch) -> Option<u8> {
    if p.dst >= c.length || p.left >= c.length || p.right >= c.length {
        return None;
    }
    let left = c.registers.add(p.left).read();
    let right = c.registers.add(p.right).read();
    let a = left as i64;
    let b = right as i64;
    let x = f64::from_bits(left);
    let y = f64::from_bits(right);
    let value = match OP {
        0 => left,
        1 => a.checked_add(b)? as u64,
        2 => a.checked_sub(b)? as u64,
        3 => a.checked_mul(b)? as u64,
        4 => {
            let q = a.checked_div(b)?;
            let r = a.checked_rem(b)?;
            if r != 0 && (r < 0) != (b < 0) {
                q.checked_sub(1)? as u64
            } else {
                q as u64
            }
        }
        5 => {
            let r = a.checked_rem(b)?;
            if r != 0 && (r < 0) != (b < 0) {
                r.checked_add(b)? as u64
            } else {
                r as u64
            }
        }
        6 => left & right,
        7 => left | right,
        8 => left ^ right,
        9 => a.checked_neg()? as u64,
        10 => !left,
        11 => u64::from(left == 0),
        12 => u64::from(x == 0.0),
        13 => (x + y).to_bits(),
        14 => (x - y).to_bits(),
        15 => (x * y).to_bits(),
        16 => {
            if y == 0.0 {
                return None;
            }
            (x / y).to_bits()
        }
        17 => (-x).to_bits(),
        18 => (a as f64).to_bits(),
        // Exact mixed comparisons and int/int true division require lossless conversion.
        19 => {
            if !(-9_007_199_254_740_992..=9_007_199_254_740_992).contains(&a) {
                return None;
            }
            (a as f64).to_bits()
        }
        20 => u64::from(a == b),
        21 => u64::from(a != b),
        22 => u64::from(a < b),
        23 => u64::from(a <= b),
        24 => u64::from(a > b),
        25 => u64::from(a >= b),
        26 => u64::from(x == y),
        27 => u64::from(x != y),
        28 => u64::from(x < y),
        29 => u64::from(x <= y),
        30 => u64::from(x > y),
        31 => u64::from(x >= y),
        32 => return Some(1),
        33 => return Some(if left != 0 { 1 } else { 2 }),
        34 => return Some(if x != 0.0 { 1 } else { 2 }),
        35 => {
            c.result = left;
            c.result_start = p.left;
            c.result_kind = p.immediate as usize;
            c.ip = p.ip;
            return Some(3);
        }
        36 => p.immediate,
        37 => {
            if !(c.range_init)(c, p) {
                return None;
            }
            return Some(1);
        }
        38 => {
            if p.left.checked_add(2)? >= c.length || p.dst.checked_add(2)? >= c.length {
                return None;
            }
            let b = c.registers.add(p.left + 1).read();
            let d = c.registers.add(p.left + 2).read();
            c.registers.add(p.dst + 1).write(b);
            c.registers.add(p.dst + 2).write(d);
            left
        }
        39 => {
            if p.left.checked_add(2)? >= c.length {
                return None;
            }
            let remaining = c.registers.add(p.left + 2).read();
            if remaining == 0 {
                return Some(2);
            }
            let step = c.registers.add(p.left + 1).read() as i64;
            c.registers.add(p.left + 2).write(remaining - 1);
            if remaining > 1 {
                c.registers.add(p.left).write(a.wrapping_add(step) as u64);
            }
            left
        }
        40 => {
            let view = view(c, p, left)?;
            if view.prepared == 2 {
                if !(c.read)(c, p) {
                    return None;
                }
                return Some(1);
            }
            let length = i64::try_from(view.length).ok()?;
            let index = if b < 0 { b.checked_add(length)? } else { b };
            if index < 0 || index >= length {
                return None;
            }
            view.data.add(index as usize).read()
        }
        41 => {
            if !(c.read)(c, p) {
                return None;
            }
            return Some(1);
        }
        42 => {
            let view = view(c, p, left)?;
            let count = (p.immediate >> 8) as usize;
            if view.length != count || p.dst.checked_add(count)? > c.length {
                return None;
            }
            for index in 0..count {
                c.registers
                    .add(p.dst + index)
                    .write(view.data.add(count - index - 1).read());
            }
            return Some(1);
        }
        43 => {
            if left == 0 {
                return None;
            }
            return Some(1);
        }
        44 => {
            if p.dst.checked_add(2)? >= c.length {
                return None;
            }
            let view = view(c, p, left)?;
            c.registers.add(p.dst + 1).write(0);
            c.registers.add(p.dst + 2).write(view.length as u64);
            left
        }
        45 => {
            if p.left.checked_add(2)? >= c.length || p.right != p.left + 1 {
                return None;
            }
            let index = c.registers.add(p.left + 1).read();
            let length = c.registers.add(p.left + 2).read();
            if index >= length {
                return Some(2);
            }
            let view = view(c, p, left)?;
            if index >= view.length as u64 {
                return None;
            }
            let value = if view.prepared == 2 {
                if !(c.read)(c, p) {
                    return None;
                }
                c.registers.add(p.dst).read()
            } else {
                view.data.add(index as usize).read()
            };
            c.registers.add(p.left + 1).write(index + 1);
            value
        }
        _ => return None,
    };
    c.registers.add(p.dst).write(value);
    Some(1)
}

macro_rules! stencil {
    ($name:ident,$op:expr) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(context: *mut Context, patch: *const Patch) -> u64 {
            let c = &mut *context;
            let p = &*patch;
            c.ip = p.ip;
            if c.remaining < p.weight {
                if !(c.checkpoint)(c.opaque, p.ip) {
                    return ERROR;
                }
                // Accumulate diagnostics per checkpoint, excluding discarded fuel.
                c.instructions += (255 - c.remaining) as u64;
                c.remaining = 255;
            }
            if p.weight > c.remaining {
                return MISS;
            }
            c.remaining -= p.weight;
            let Some(direction) = execute::<$op>(c, p) else {
                return MISS;
            };
            if direction == 3 {
                return RETURN;
            }
            let (address, data) = if direction == 2 {
                (p.alternate, p.alternate_data)
            } else {
                (p.next, p.next_data)
            };
            let next: Entry = std::mem::transmute(address);
            next(context, data as *const Patch)
        }
    };
}

stencil!(monty_stencil_copy, 0);
stencil!(monty_stencil_iadd, 1);
stencil!(monty_stencil_isub, 2);
stencil!(monty_stencil_imul, 3);
stencil!(monty_stencil_ifloor, 4);
stencil!(monty_stencil_imod, 5);
stencil!(monty_stencil_and, 6);
stencil!(monty_stencil_or, 7);
stencil!(monty_stencil_xor, 8);
stencil!(monty_stencil_ineg, 9);
stencil!(monty_stencil_invert, 10);
stencil!(monty_stencil_inot, 11);
stencil!(monty_stencil_fnot, 12);
stencil!(monty_stencil_fadd, 13);
stencil!(monty_stencil_fsub, 14);
stencil!(monty_stencil_fmul, 15);
stencil!(monty_stencil_fdiv, 16);
stencil!(monty_stencil_fneg, 17);
stencil!(monty_stencil_float, 18);
stencil!(monty_stencil_exact_float, 19);
stencil!(monty_stencil_ieq, 20);
stencil!(monty_stencil_ine, 21);
stencil!(monty_stencil_ilt, 22);
stencil!(monty_stencil_ile, 23);
stencil!(monty_stencil_igt, 24);
stencil!(monty_stencil_ige, 25);
stencil!(monty_stencil_feq, 26);
stencil!(monty_stencil_fne, 27);
stencil!(monty_stencil_flt, 28);
stencil!(monty_stencil_fle, 29);
stencil!(monty_stencil_fgt, 30);
stencil!(monty_stencil_fge, 31);
stencil!(monty_stencil_jump, 32);
stencil!(monty_stencil_ibranch, 33);
stencil!(monty_stencil_fbranch, 34);
stencil!(monty_stencil_return, 35);
stencil!(monty_stencil_immediate, 36);

stencil!(monty_stencil_range, 37);
stencil!(monty_stencil_triple, 38);
stencil!(monty_stencil_for_iter, 39);

stencil!(monty_stencil_index, 40);
stencil!(monty_stencil_read, 41);
stencil!(monty_stencil_unpack, 42);

stencil!(monty_stencil_bound, 43);

stencil!(monty_stencil_sequence_iter, 44);
stencil!(monty_stencil_sequence_next, 45);
