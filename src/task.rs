//! Sprint 2 — Multitasking: Kernel Threads and Preemptive Round-Robin Scheduler.
//!
//! Provides a simple preemptive scheduler based on the Round-Robin algorithm.
//! Tasks are created with a private kernel-mode stack allocated on the heap;
//! context is saved/restored through the `Context` register snapshot.

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use spin::Mutex;

/// Opaque numeric identifier for a kernel task.
pub type TaskId = usize;

/// Callee-saved register snapshot used for software context switches (x86_64 System V ABI).
#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct Context {
    pub rbx: u64,
    pub rbp: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    /// Stack pointer at the moment the task was descheduled.
    pub rsp: u64,
    /// Instruction pointer to resume execution from.
    pub rip: u64,
}

/// Lifecycle state of a kernel task.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Exited,
}

/// Size of the private kernel stack allocated per task (16 KiB).
pub const KERNEL_STACK_SIZE: usize = 4096 * 4;

/// A single kernel-space thread of execution.
pub struct Task {
    pub id: TaskId,
    pub state: TaskState,
    pub context: Context,
    /// Heap-allocated kernel stack; kept alive for the lifetime of the task.
    _stack: Box<[u8]>,
}

impl Task {
    /// Create a new task that will begin executing at `entry_point`.
    ///
    /// The stack is allocated on the kernel heap so this must be called *after*
    /// `memory::init_heap` has succeeded.
    pub fn new(id: TaskId, entry_point: fn() -> !) -> Self {
        let stack = alloc::vec![0u8; KERNEL_STACK_SIZE].into_boxed_slice();
        // x86_64 stacks grow downward: RSP starts at the *top* of the allocation.
        let stack_top = (stack.as_ptr() as usize + KERNEL_STACK_SIZE) as u64;

        let mut context = Context::default();
        context.rsp = stack_top;
        context.rip = entry_point as u64;

        Task {
            id,
            state: TaskState::Ready,
            context,
            _stack: stack,
        }
    }
}

/// Preemptive Round-Robin kernel scheduler.
///
/// The task list acts as a circular queue: after each scheduling tick the
/// running task is moved to the back of the queue and the next `Ready` task
/// at the front is selected.
pub struct Scheduler {
    tasks: Vec<Task>,
    /// Index of the currently running task inside `tasks`, if any.
    running_idx: Option<usize>,
    /// Monotonically increasing counter used to generate unique `TaskId`s.
    next_id: TaskId,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            tasks: Vec::new(),
            running_idx: None,
            next_id: 0,
        }
    }

    /// Spawn a new kernel thread and return its `TaskId`.
    pub fn spawn(&mut self, entry_point: fn() -> !) -> TaskId {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(Task::new(id, entry_point));
        id
    }

    /// Perform one Round-Robin scheduling step.
    ///
    /// Marks the currently running task as `Ready`, then walks the queue
    /// to find the next `Ready` task and marks it `Running`.
    /// Returns the `TaskId` that should now run, or `None` if no task is ready.
    pub fn schedule(&mut self) -> Option<TaskId> {
        // Deschedule the current task.
        if let Some(idx) = self.running_idx.take() {
            if let Some(task) = self.tasks.get_mut(idx) {
                if task.state == TaskState::Running {
                    task.state = TaskState::Ready;
                }
            }
        }

        let len = self.tasks.len();
        if len == 0 {
            return None;
        }

        // Find the next Ready task in round-robin order starting after the
        // last running position (or from 0 if there was none).
        let start = self.running_idx.map(|i| (i + 1) % len).unwrap_or(0);
        for offset in 0..len {
            let idx = (start + offset) % len;
            if self.tasks[idx].state == TaskState::Ready {
                self.tasks[idx].state = TaskState::Running;
                let id = self.tasks[idx].id;
                self.running_idx = Some(idx);
                return Some(id);
            }
        }
        None
    }

    /// Block the task with the given `id` (e.g. it is waiting for I/O or IPC).
    pub fn block_task(&mut self, id: TaskId) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            if task.state != TaskState::Exited {
                task.state = TaskState::Blocked;
            }
        }
    }

    /// Unblock a previously blocked task, making it eligible to run again.
    pub fn unblock_task(&mut self, id: TaskId) {
        if let Some(task) = self.tasks.iter_mut().find(|t| t.id == id) {
            if task.state == TaskState::Blocked {
                task.state = TaskState::Ready;
            }
        }
    }

    /// Mark a task as exited and remove it from the scheduler queue.
    pub fn exit_task(&mut self, id: TaskId) {
        if let Some(pos) = self.tasks.iter().position(|t| t.id == id) {
            self.tasks.remove(pos);
            // Adjust running_idx if it pointed at or past the removed slot.
            if let Some(idx) = self.running_idx {
                if idx == pos {
                    self.running_idx = None;
                } else if idx > pos {
                    self.running_idx = Some(idx - 1);
                }
            }
        }
    }

    /// Return the number of live tasks currently tracked by the scheduler.
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

/// Global scheduler instance, initialised after kernel heap setup.
/// Access is serialised via a spinlock.
pub static SCHEDULER: Mutex<Option<Scheduler>> = Mutex::new(None);

/// Initialise the global scheduler. **Must** be called after `memory::init_heap`.
pub fn init_scheduler() {
    *SCHEDULER.lock() = Some(Scheduler::new());
    log::info!("Kernel task scheduler initialized (Round-Robin).");
}
