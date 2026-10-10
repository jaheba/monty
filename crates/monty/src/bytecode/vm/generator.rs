use std::mem;

use super::{CallFrame, SerializedFrame, VM, frame_code, stack_index};
use crate::{
    args::{ArgValues, FromArgs},
    bytecode::{CallResult, FrameNamespace},
    defer_drop,
    exception_private::{ExcType, ExcTypeExt, ExceptionValue, RunResult, SimpleException},
    heap::{ContainsHeap, DropWithContext, HeapData, HeapId, HeapItem, HeapReadOutput, HeapReader},
    intern::FunctionId,
    value::Value,
};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Generator {
    #[serde(rename = "S")]
    state: GeneratorState,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
enum GeneratorState {
    #[serde(rename = "C")]
    Created(SuspendedGenerator),
    #[serde(rename = "S")]
    Suspended(SuspendedGenerator),
    #[serde(rename = "R")]
    Running,
    #[serde(rename = "D")]
    Completed,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct SuspendedGenerator {
    #[serde(rename = "F")]
    frame: SerializedFrame,
    #[serde(rename = "S")]
    stack: Vec<Value>,
    #[serde(rename = "E")]
    exceptions: Vec<Value>,
}

/// Owns the running generator and the pending iterator consumer's state.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct GeneratorBoundary {
    #[serde(rename = "I")]
    id: HeapId,
    #[serde(rename = "C")]
    consumer: GeneratorConsumer,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(crate) enum GeneratorConsumer {
    #[serde(rename = "N")]
    Next(Option<Value>),
    #[serde(rename = "F")]
    For { end_ip: usize },
}

#[derive(FromArgs)]
#[from_args(name = "send", style = unpack)]
struct SendArgs {
    #[from_args(pos_only)]
    value: Value,
}

pub(super) enum GeneratorReturn {
    Continue,
    Stop(Value),
    Raise { exception: Value, stop: bool },
}

impl<C: ContainsHeap> DropWithContext<C> for Box<GeneratorBoundary> {
    fn drop_with(self, heap: &mut C) {
        HeapReader::with(heap.heap_mut(), &mut (), |heap, ()| {
            if let HeapReadOutput::Generator(mut generator) = heap.read(self.id) {
                generator.get_mut(heap).state = GeneratorState::Completed;
            }
        });
        if let GeneratorConsumer::Next(default) = self.consumer {
            default.drop_with(heap);
        }
        heap.heap_mut().dec_ref(self.id);
    }
}

impl Generator {
    pub(super) fn new(function_id: FunctionId, stack: Vec<Value>, namespace: Option<Box<FrameNamespace>>) -> Self {
        let locals_count = u16::try_from(stack.len()).expect("function namespace exceeds u16");
        Self {
            state: GeneratorState::Created(SuspendedGenerator {
                frame: SerializedFrame {
                    function_id: Some(function_id),
                    ip: 0,
                    stack_base: 0,
                    locals_count,
                    exception_stack_base: 0,
                    call_offset: None,
                    is_initializer: false,
                    namespace,
                    generator: None,
                },
                stack,
                exceptions: Vec::new(),
            }),
        }
    }

    pub(crate) fn for_each_child(&self, mut visit: impl FnMut(HeapId)) {
        if let GeneratorState::Created(saved) | GeneratorState::Suspended(saved) = &self.state {
            for value in saved.stack.iter().chain(&saved.exceptions) {
                if let Value::Ref(id) = value {
                    visit(*id);
                }
            }
            if let Some(namespace) = &saved.frame.namespace {
                namespace.for_each_child(visit);
            }
        }
    }
}

impl HeapItem for Generator {
    fn py_dec_ref_ids(&mut self, ids: &mut Vec<HeapId>) {
        if let GeneratorState::Created(saved) | GeneratorState::Suspended(saved) = &mut self.state {
            for value in saved.stack.iter_mut().chain(&mut saved.exceptions) {
                value.py_dec_ref_ids(ids);
            }
            if let Some(namespace) = &saved.frame.namespace {
                namespace.for_each_child(|id| ids.push(id));
            }
        }
    }
}

impl VM<'_> {
    pub(crate) fn send_generator(&mut self, id: HeapId, args: ArgValues) -> RunResult<CallResult> {
        let SendArgs { value } = SendArgs::from_args(args, self)?;
        let vm = self;
        defer_drop!(value, vm);
        if !matches!(value, Value::None) {
            return Err(ExcType::not_implemented("generator.send() only supports None").into());
        }
        vm.resume_generator(id, GeneratorConsumer::Next(None))
    }

    pub(crate) fn resume_generator(&mut self, id: HeapId, consumer: GeneratorConsumer) -> RunResult<CallResult> {
        let HeapReadOutput::Generator(mut generator) = self.heap.read(id) else {
            unreachable!("resume requires a generator")
        };
        match &generator.get(self.heap).state {
            GeneratorState::Running => {
                if let GeneratorConsumer::Next(default) = consumer {
                    default.drop_with(self);
                }
                return Err(ExcType::value_error("generator already executing"));
            }
            GeneratorState::Completed => {
                drop(generator);
                return self.generator_exhausted(consumer);
            }
            _ => {}
        }
        if let Err(error) = self.incr_recursion() {
            if let GeneratorConsumer::Next(default) = consumer {
                default.drop_with(self);
            }
            return Err(error.into());
        }
        let state = mem::replace(&mut generator.get_mut(self.heap).state, GeneratorState::Running);
        let (mut saved, resumed) = match state {
            GeneratorState::Created(saved) => (saved, false),
            GeneratorState::Suspended(saved) => (saved, true),
            _ => unreachable!(),
        };
        let stack_base = self.stack.len();
        let exception_stack_base = self.exception_stack.len();
        self.stack.append(&mut saved.stack);
        self.exception_stack.append(&mut saved.exceptions);
        if resumed {
            self.push(Value::None);
        }
        let sf = saved.frame;
        let code = frame_code(self.interns, self.module_code, sf.function_id);
        let frame = CallFrame {
            code,
            bytecode: code.bytecode(),
            ip: sf.ip,
            stack_base: stack_index(stack_base),
            locals_count: sf.locals_count,
            exception_stack_base: stack_index(exception_stack_base),
            function_id: sf.function_id,
            call_offset: self.current_offset(),
            should_return: false,
            is_parked: false,
            namespace: sf.namespace,
            is_initializer: false,
            generator: Some(Box::new(GeneratorBoundary { id, consumer })),
        };
        self.heap.inc_ref(id);
        self.push_admitted_frame(frame);
        Ok(CallResult::FramePushed)
    }

    fn generator_exhausted(&mut self, consumer: GeneratorConsumer) -> RunResult<CallResult> {
        match consumer {
            GeneratorConsumer::Next(Some(default)) => Ok(CallResult::Value(default)),
            GeneratorConsumer::Next(None) => Err(ExcType::stop_iteration()),
            GeneratorConsumer::For { end_ip } => {
                let iterator = self.pop();
                iterator.drop_with(self);
                self.current_frame.ip = end_ip;
                Ok(CallResult::FramePushed)
            }
        }
    }

    pub(super) fn yield_generator(&mut self) -> Option<Value> {
        let value = self.pop();
        let boundary = self
            .current_frame
            .generator
            .take()
            .expect("yield outside generator frame");
        let caller = self.suspended_frames.pop().expect("generator has no caller");
        let mut frame = mem::replace(&mut self.current_frame, caller);
        let stack = self.stack.split_off(frame.stack_base());
        let exceptions = self.exception_stack.split_off(frame.exception_stack_base());
        let stop = frame.should_return;
        frame.should_return = false;
        let mut saved_frame = frame.serialize();
        saved_frame.stack_base = 0;
        saved_frame.exception_stack_base = 0;
        saved_frame.call_offset = None;
        let HeapReadOutput::Generator(mut generator) = self.heap.read(boundary.id) else {
            unreachable!()
        };
        generator.get_mut(self.heap).state = GeneratorState::Suspended(SuspendedGenerator {
            frame: saved_frame,
            stack,
            exceptions,
        });
        drop(generator);
        self.decr_recursion();
        self.instruction_ip = self.current_frame.ip;
        self.release_generator_boundary(*boundary);
        if stop {
            Some(value)
        } else {
            self.push(value);
            None
        }
    }

    fn release_generator_boundary(&mut self, boundary: GeneratorBoundary) {
        if let GeneratorConsumer::Next(default) = boundary.consumer {
            default.drop_with(self);
        }
        self.heap.dec_ref(boundary.id);
    }

    pub(super) fn abandon_generator(&mut self, boundary: GeneratorBoundary) {
        let HeapReadOutput::Generator(mut generator) = self.heap.read(boundary.id) else {
            unreachable!()
        };
        generator.get_mut(self.heap).state = GeneratorState::Completed;
        drop(generator);
        self.release_generator_boundary(boundary);
    }

    pub(super) fn return_generator(&mut self, value: Value) -> GeneratorReturn {
        let boundary = self.current_frame.generator.take().expect("generator boundary missing");
        let HeapReadOutput::Generator(mut generator) = self.heap.read(boundary.id) else {
            unreachable!()
        };
        generator.get_mut(self.heap).state = GeneratorState::Completed;
        drop(generator);
        self.exception_stack
            .drain(self.current_frame.exception_stack_base()..)
            .drop_with(self.heap);
        let stop = self.pop_frame();
        self.heap.dec_ref(boundary.id);
        match boundary.consumer {
            GeneratorConsumer::Next(None) => {
                let exception = SimpleException::new_none(ExcType::StopIteration);
                let value = if matches!(value, Value::None) {
                    value.drop_with(self);
                    None
                } else {
                    Some(value)
                };
                let exception = ExceptionValue { exception, value };
                let exception = Value::Ref(self.heap.allocate(HeapData::Exception(exception)));
                GeneratorReturn::Raise { exception, stop }
            }
            GeneratorConsumer::Next(Some(default)) => {
                value.drop_with(self);
                if stop {
                    GeneratorReturn::Stop(default)
                } else {
                    self.push(default);
                    GeneratorReturn::Continue
                }
            }
            GeneratorConsumer::For { end_ip } => {
                value.drop_with(self);
                let iterator = self.pop();
                iterator.drop_with(self);
                self.current_frame.ip = end_ip;
                GeneratorReturn::Continue
            }
        }
    }
}
