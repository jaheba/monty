//! Synchronous native entry: the VM owns every reference throughout the invocation.
#![allow(unsafe_code)]

use std::{ffi::c_void, mem, ptr};

use super::{HeapReadOutput, VM, Value};
pub(super) use crate::bytecode::native::enabled;
use crate::{
    bytecode::native::{
        self, NativeCode,
        abi::{self, Context, Slot},
    },
    heap::Heap,
};

impl VM<'_> {
    pub(super) fn try_native(&mut self, countdown: &mut u8) {
        // Executable mappings are outside the allocator's accounting in this prototype.
        if self.heap.tracker.max_memory().is_some() {
            return;
        }
        let code = self.current_frame.code;
        let Some(artifact) = code.native.get_or_init(|| NativeCode::compile(code)) else {
            return;
        };
        let Some((entry, patch)) = artifact.entry(self.current_frame.ip) else {
            return;
        };
        let base = self.current_frame.stack_base();
        let frame_len = self.stack.len() - base;
        if usize::from(self.current_frame.locals_count) > frame_len {
            return;
        }
        if frame_len + self.globals.len() > 8192 || *countdown == 0 {
            return;
        }

        let mut scratch = native::take_scratch();
        scratch.stack.clear();
        scratch.stack.extend(self.stack[base..].iter().map(native::encode));
        scratch
            .stack
            .resize(frame_len + 2 * usize::from(*countdown), Slot::default());
        scratch.globals.clear();
        scratch.globals.extend(self.globals.iter().map(native::encode));
        let mut context = Context {
            stack: scratch.stack.as_mut_ptr(),
            capacity: scratch.stack.len(),
            sp: frame_len,
            locals: usize::from(self.current_frame.locals_count),
            globals: scratch.globals.as_mut_ptr(),
            globals_len: scratch.globals.len(),
            constants: artifact.constants.as_ptr(),
            constants_len: artifact.constants.len(),
            remaining: usize::from(*countdown),
            ip: self.current_frame.ip,
            opaque: ptr::from_mut(self).cast(),
            lookup,
            range_next,
            dictionary_reads: 0,
            range_steps: 0,
        };
        // SAFETY: the artifact and fixed buffers outlive this synchronous call. Stencils access
        // only checked slots; callbacks cannot allocate or execute Python. Only range cursors mutate.
        let last_ip = unsafe { entry(&raw mut context, patch) };
        let executed = usize::from(*countdown) - context.remaining;
        if executed != 0 {
            // Decode both images before changing either source: borrowed slots may alias any
            // original local, global or constant, including values overwritten in the native image.
            for slot in &scratch.stack[..context.sp] {
                scratch.stack_values.push(decode(*slot, self.heap));
            }
            for slot in &scratch.globals {
                scratch.global_values.push(decode(*slot, self.heap));
            }
            for old in self.stack.drain(base..) {
                old.drop_with(self.heap);
            }
            self.stack.append(&mut scratch.stack_values);
            for (old, value) in self.globals.iter_mut().zip(scratch.global_values.drain(..)) {
                mem::replace(old, value).drop_with(self.heap);
            }
            self.current_frame.ip = context.ip;
            self.instruction_ip = usize::try_from(last_ip).expect("native bytecode offset");
            *countdown = u8::try_from(context.remaining).expect("native budget only decreases");
        }
        native::record(|stats| {
            stats.entries += 1;
            stats.instructions += executed as u64;
            stats.dictionary_reads += context.dictionary_reads;
            stats.range_steps += context.range_steps;
        });
        native::put_scratch(scratch);
    }
}

unsafe extern "C" fn range_next(opaque: *mut c_void, iterator: Slot) -> Slot {
    let miss = Slot {
        tag: abi::BLOCKED,
        bits: 0,
    };
    if iterator.tag != abi::BORROWED {
        return miss;
    }
    // SAFETY: the synchronous caller owns the unchanged VM buffers. This callback only advances
    // an exact builtin range iterator and retains no borrow or pointer after returning.
    let (vm, value) = unsafe {
        (
            &mut *opaque.cast::<VM<'_>>(),
            &*(usize::try_from(iterator.bits).expect("native pointer width") as *const Value),
        )
    };
    let Value::Ref(id) = value else { return miss };
    let HeapReadOutput::RangeIterator(mut iterator) = vm.heap.read(*id) else {
        return miss;
    };
    match iterator.get_mut(vm.heap).next_integer() {
        Some(value) => Slot {
            tag: abi::INT,
            bits: value.cast_unsigned(),
        },
        None => Slot {
            tag: abi::NONE,
            bits: 0,
        },
    }
}

fn decode(slot: Slot, heap: &mut Heap) -> Value {
    match slot.tag {
        abi::UNBOUND => Value::Undefined,
        abi::INT => Value::Int(slot.bits.cast_signed()),
        abi::FLOAT => Value::Float(f64::from_bits(slot.bits)),
        abi::BOOL => Value::Bool(slot.bits != 0),
        abi::NONE => Value::None,
        abi::BORROWED => {
            // SAFETY: only encode creates borrowed slots, from the unchanged VM arrays or
            // immutable constants. All sources remain alive until both images are decoded.
            unsafe {
                (&*(usize::try_from(slot.bits).expect("native pointer width") as *const Value)).clone_with_heap(heap)
            }
        }
        _ => unreachable!("non-materializable native slot"),
    }
}

unsafe extern "C" fn lookup(opaque: *mut c_void, object: Slot, key: Slot) -> Slot {
    let miss = Slot {
        tag: abi::BLOCKED,
        bits: 0,
    };
    if object.tag != abi::BORROWED || key.tag != abi::BORROWED {
        return miss;
    }
    // SAFETY: the native caller temporarily borrows its original VM and source Values. This
    // callback creates no owned heap references and changes neither the VM nor its buffers.
    let (vm, object, key) = unsafe {
        (
            &*opaque.cast::<VM<'_>>(),
            &*(usize::try_from(object.bits).expect("native pointer width") as *const Value),
            &*(usize::try_from(key.bits).expect("native pointer width") as *const Value),
        )
    };
    let Value::Ref(id) = object else { return miss };
    let HeapReadOutput::Dict(dict) = vm.heap.read(*id) else {
        return miss;
    };
    let key = match key {
        Value::InternString(id) => vm.interns.get_str(*id),
        Value::Ref(id) => match vm.heap.get(*id) {
            super::HeapData::Str(value) => value.as_str(),
            _ => return miss,
        },
        _ => return miss,
    };
    let Some(value) = dict.get(vm.heap).native_string_get(key, vm.heap, vm.interns) else {
        return miss;
    };
    match value {
        Value::Int(_) | Value::Float(_) | Value::Bool(_) | Value::None => native::encode(value),
        _ => miss,
    }
}
