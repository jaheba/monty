//! Compiled separately by build.rs; no Rust VM layout is exposed to copied code.
#![allow(dead_code, unsafe_op_in_unsafe_fn)]

mod abi;
use abi::*;

#[inline(always)]
unsafe fn truth(slot: Slot) -> Option<bool> {
    match slot.tag {
        INT | BOOL => Some(slot.bits != 0),
        NONE => Some(false),
        FLOAT => Some(f64::from_bits(slot.bits) != 0.0),
        _ => None,
    }
}

#[inline(always)]
unsafe fn push(context: &mut Context, value: Slot) -> bool {
    if context.sp >= context.capacity {
        return false;
    }
    let target = context.stack.add(context.sp);
    // A separate volatile tag store prevents LLVM from introducing relocated constant pools.
    std::ptr::addr_of_mut!((*target).tag).write_volatile(value.tag);
    std::ptr::addr_of_mut!((*target).bits).write(value.bits);
    context.sp += 1;
    true
}

// No operation changes its image before every guard for that operation has passed.
#[inline(always)]
unsafe fn step<const OP: u8>(c: &mut Context, p: &Patch) -> u8 {
    if OP == EXIT || c.remaining == 0 {
        return 0;
    }

    let direction = match OP {
        CONST => {
            if p.argument >= c.constants_len {
                return 0;
            }
            let value = c.constants.add(p.argument).read();
            if value.tag == BLOCKED || !push(c, value) {
                return 0;
            }
            1
        }
        SMALL_INT | NONE_VALUE | TRUE_VALUE | FALSE_VALUE => {
            let value = match OP {
                SMALL_INT => Slot {
                    tag: INT,
                    bits: i64::from((p.argument as u8).cast_signed()).cast_unsigned(),
                },
                NONE_VALUE => Slot { tag: NONE, bits: 0 },
                TRUE_VALUE => Slot { tag: BOOL, bits: 1 },
                _ => Slot { tag: BOOL, bits: 0 },
            };
            if !push(c, value) {
                return 0;
            }
            1
        }
        LOCAL | GLOBAL => {
            let (source, len) = if OP == LOCAL {
                (c.stack, c.locals)
            } else {
                (c.globals, c.globals_len)
            };
            if p.argument >= len {
                return 0;
            }
            let value = source.add(p.argument).read();
            if value.tag == UNBOUND || !push(c, value) {
                return 0;
            }
            1
        }
        STORE_LOCAL | STORE_GLOBAL => {
            let (target, len) = if OP == STORE_LOCAL {
                (c.stack, c.locals)
            } else {
                (c.globals, c.globals_len)
            };
            if p.argument >= len || c.sp <= c.locals {
                return 0;
            }
            c.sp -= 1;
            target.add(p.argument).write(c.stack.add(c.sp).read());
            1
        }
        ADD | SUB | MUL | EQ | NE | LT | LE | GT | GE | AND | OR | XOR | FLOOR | MOD => {
            if c.sp < c.locals + 2 {
                return 0;
            }
            let a = c.stack.add(c.sp - 2).read();
            let b = c.stack.add(c.sp - 1).read();
            if a.tag != INT || b.tag != INT {
                return 0;
            }
            let (a, b) = (a.bits as i64, b.bits as i64);
            let result = match OP {
                ADD => a.checked_add(b),
                SUB => a.checked_sub(b),
                MUL => a.checked_mul(b),
                AND => Some(a & b),
                OR => Some(a | b),
                XOR => Some(a ^ b),
                FLOOR | MOD => {
                    let Some(q) = a.checked_div(b) else { return 0 };
                    let r = a % b;
                    let adjust = r != 0 && (r < 0) != (b < 0);
                    Some(if OP == FLOOR {
                        q - i64::from(adjust)
                    } else if adjust {
                        r + b
                    } else {
                        r
                    })
                }
                EQ => Some(i64::from(a == b)),
                NE => Some(i64::from(a != b)),
                LT => Some(i64::from(a < b)),
                LE => Some(i64::from(a <= b)),
                GT => Some(i64::from(a > b)),
                _ => Some(i64::from(a >= b)),
            };
            let Some(value) = result else { return 0 };
            let tag = if OP >= EQ && OP <= GE { BOOL } else { INT };
            c.stack.add(c.sp - 2).write(Slot {
                tag,
                bits: value as u64,
            });
            c.sp -= 1;
            1
        }
        FALSE | TRUE => {
            if c.sp <= c.locals {
                return 0;
            }
            let Some(value) = truth(c.stack.add(c.sp - 1).read()) else {
                return 0;
            };
            c.sp -= 1;
            if value == (OP == TRUE) { 2 } else { 1 }
        }
        POP | DUP | NEG | NOT => {
            if c.sp <= c.locals {
                return 0;
            }
            let value = c.stack.add(c.sp - 1).read();
            match OP {
                POP => c.sp -= 1,
                DUP => {
                    if !push(c, value) {
                        return 0;
                    }
                }
                NEG => {
                    if value.tag != INT {
                        return 0;
                    }
                    let Some(negated) = (value.bits as i64).checked_neg() else {
                        return 0;
                    };
                    c.stack.add(c.sp - 1).write(Slot {
                        tag: INT,
                        bits: negated as u64,
                    });
                }
                _ => {
                    let Some(value) = truth(value) else { return 0 };
                    c.stack.add(c.sp - 1).write(Slot {
                        tag: BOOL,
                        bits: u64::from(!value),
                    });
                }
            }
            1
        }
        FOR_ITER => {
            // Capacity must be checked before advancing the real iterator.
            if c.sp <= c.locals || c.sp >= c.capacity {
                return 0;
            }
            let value = (c.range_next)(c.opaque, c.stack.add(c.sp - 1).read());
            if value.tag == BLOCKED {
                return 0;
            }
            c.range_steps += 1;
            if value.tag == NONE {
                c.sp -= 1;
                2
            } else {
                push(c, value);
                1
            }
        }
        GETITEM => {
            if c.sp < c.locals + 2 {
                return 0;
            }
            let object = c.stack.add(c.sp - 2).read();
            let key = c.stack.add(c.sp - 1).read();
            let value = (c.lookup)(c.opaque, object, key);
            if value.tag == BLOCKED {
                return 0;
            }
            c.stack.add(c.sp - 2).write(value);
            c.sp -= 1;
            c.dictionary_reads += 1;
            1
        }
        JUMP | NOP => 1,
        _ => return 0,
    };
    c.remaining -= 1;
    direction
}

macro_rules! stencil {
    ($name:ident, $op:ident) => {
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn $name(context: *mut Context, patch: *const Patch) -> u64 {
            let c = &mut *context;
            let p = &*patch;
            let previous = c.ip;
            c.ip = p.ip;
            let direction = step::<$op>(c, p);
            if direction == 0 {
                return previous as u64;
            }
            let (address, patch) = if direction == 2 {
                (p.alternate, p.alternate_data)
            } else {
                (p.next, p.next_data)
            };
            let next: Entry = std::mem::transmute(address);
            next(context, patch as *const Patch)
        }
    };
}

stencil!(monty_stencil_exit, EXIT);
stencil!(monty_stencil_const, CONST);
stencil!(monty_stencil_small_int, SMALL_INT);
stencil!(monty_stencil_local, LOCAL);
stencil!(monty_stencil_store_local, STORE_LOCAL);
stencil!(monty_stencil_global, GLOBAL);
stencil!(monty_stencil_store_global, STORE_GLOBAL);
stencil!(monty_stencil_add, ADD);
stencil!(monty_stencil_sub, SUB);
stencil!(monty_stencil_mul, MUL);
stencil!(monty_stencil_eq, EQ);
stencil!(monty_stencil_ne, NE);
stencil!(monty_stencil_lt, LT);
stencil!(monty_stencil_le, LE);
stencil!(monty_stencil_gt, GT);
stencil!(monty_stencil_ge, GE);
stencil!(monty_stencil_jump, JUMP);
stencil!(monty_stencil_false, FALSE);
stencil!(monty_stencil_true, TRUE);
stencil!(monty_stencil_pop, POP);
stencil!(monty_stencil_dup, DUP);
stencil!(monty_stencil_neg, NEG);
stencil!(monty_stencil_not, NOT);
stencil!(monty_stencil_and, AND);
stencil!(monty_stencil_or, OR);
stencil!(monty_stencil_xor, XOR);
stencil!(monty_stencil_floor, FLOOR);
stencil!(monty_stencil_mod, MOD);
stencil!(monty_stencil_getitem, GETITEM);
stencil!(monty_stencil_none, NONE_VALUE);
stencil!(monty_stencil_true_value, TRUE_VALUE);
stencil!(monty_stencil_false_value, FALSE_VALUE);
stencil!(monty_stencil_nop, NOP);
stencil!(monty_stencil_for_iter, FOR_ITER);
