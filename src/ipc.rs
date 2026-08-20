//! Sprint 3 — IPC Engine: ultra-fast message passing between Ring 0 and Ring 3.
//!
//! Implements a fixed-capacity synchronous channel based on a circular FIFO
//! queue protected by a spinlock.  Each `Channel` holds up to `IPC_QUEUE_CAP`
//! `Message` objects; senders block-spin until space is available and receivers
//! block-spin until a message arrives.

use spin::Mutex;

/// Maximum payload size in bytes per IPC message.
pub const IPC_MSG_SIZE: usize = 256;

/// Maximum number of messages that can be buffered inside one channel.
pub const IPC_QUEUE_CAP: usize = 16;

/// Operation code carried inside an IPC message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Opcode {
    /// No-operation / ping to verify the channel is alive.
    Ping = 0,
    /// Request the service to read data and return it in a reply.
    Read = 1,
    /// Request the service to write the supplied payload.
    Write = 2,
    /// Request the service to shut down gracefully.
    Shutdown = 3,
    /// Generic reply from a service back to its client.
    Reply = 4,
    /// Error reply: payload contains a UTF-8 encoded error description.
    Error = 5,
}

/// A single IPC message exchanged between two tasks.
#[derive(Clone, Copy)]
pub struct Message {
    /// Task ID of the sender.
    pub sender: u32,
    /// Task ID of the intended receiver.
    pub receiver: u32,
    /// Semantic meaning of the message.
    pub opcode: Opcode,
    /// Inline payload bytes.
    pub data: [u8; IPC_MSG_SIZE],
    /// Number of valid bytes in `data`.
    pub data_len: usize,
}

impl Message {
    /// Construct a new message with a zeroed payload.
    pub const fn new(sender: u32, receiver: u32, opcode: Opcode) -> Self {
        Self {
            sender,
            receiver,
            opcode,
            data: [0u8; IPC_MSG_SIZE],
            data_len: 0,
        }
    }

    /// Copy `payload` into the message body (truncated to `IPC_MSG_SIZE`).
    pub fn write_payload(&mut self, payload: &[u8]) {
        let len = payload.len().min(IPC_MSG_SIZE);
        self.data[..len].copy_from_slice(&payload[..len]);
        self.data_len = len;
    }
}

impl core::fmt::Debug for Message {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Message")
            .field("sender", &self.sender)
            .field("receiver", &self.receiver)
            .field("opcode", &self.opcode)
            .field("data_len", &self.data_len)
            .finish()
    }
}

/// Possible errors returned by IPC operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpcError {
    /// The channel queue was full when `try_send` was called.
    QueueFull,
    /// The channel queue was empty when `try_recv` was called.
    QueueEmpty,
}

/// A fixed-capacity FIFO channel for passing `Message` objects between tasks.
///
/// The internal buffer is a circular array guarded by a spinlock.
pub struct Channel {
    buffer: [Option<Message>; IPC_QUEUE_CAP],
    head: usize,
    tail: usize,
    count: usize,
}

impl Channel {
    /// Create an empty channel.
    pub const fn new() -> Self {
        Self {
            buffer: [None; IPC_QUEUE_CAP],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    /// Attempt to enqueue `msg` without blocking.
    /// Returns `IpcError::QueueFull` if the buffer has no room.
    pub fn try_send(&mut self, msg: Message) -> Result<(), IpcError> {
        if self.count == IPC_QUEUE_CAP {
            return Err(IpcError::QueueFull);
        }
        self.buffer[self.tail] = Some(msg);
        self.tail = (self.tail + 1) % IPC_QUEUE_CAP;
        self.count += 1;
        Ok(())
    }

    /// Attempt to dequeue a message without blocking.
    /// Returns `IpcError::QueueEmpty` if the buffer contains no messages.
    pub fn try_recv(&mut self) -> Result<Message, IpcError> {
        if self.count == 0 {
            return Err(IpcError::QueueEmpty);
        }
        let msg = self.buffer[self.head].take().expect("IPC buffer slot must be Some");
        self.head = (self.head + 1) % IPC_QUEUE_CAP;
        self.count -= 1;
        Ok(msg)
    }

    /// Spin-wait until a message can be enqueued, then send it.
    pub fn send(&mut self, msg: Message) {
        loop {
            if self.try_send(msg).is_ok() {
                return;
            }
            core::hint::spin_loop();
        }
    }

    /// Spin-wait until a message is available, then return it.
    pub fn recv(&mut self) -> Message {
        loop {
            if let Ok(msg) = self.try_recv() {
                return msg;
            }
            core::hint::spin_loop();
        }
    }

    /// Number of messages currently buffered in the channel.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns `true` when the channel has no buffered messages.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// System-wide IPC bus: a single shared channel accessible to all kernel components.
/// In a full microkernel each service pair would have its own dedicated channel;
/// this global channel serves as the prototype for Ring-0 ↔ Ring-3 messaging.
pub static SYSTEM_CHANNEL: Mutex<Channel> = Mutex::new(Channel::new());

/// Initialise the IPC subsystem.
pub fn init_ipc() {
    // The channel is already initialised via its `const fn new()`.
    // This function exists as a hook for future per-service channel registration.
    log::info!("IPC Engine initialized (fixed-capacity channel, {} slots).", IPC_QUEUE_CAP);
}
