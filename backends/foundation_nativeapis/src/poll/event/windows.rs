/// Windows `Event` — represents a completion from `GetQueuedCompletionStatus`.

use std::io::ErrorKind;

/// A single readiness event from IOCP.
#[derive(Clone, Copy)]
pub struct Event {
    /// The completion key (token) associated with this event.
    key: usize,
    /// Number of bytes transferred (for overlapped I/O).
    bytes: u32,
    /// Whether the operation failed.
    error: Option<std::io::Error>,
    /// Bitmask of readiness flags.
    flags: u8,
}

const READABLE: u8 = 0b00001;
const WRITABLE: u8 = 0b00010;
const READ_CLOSED: u8 = 0b00100;
const WRITE_CLOSED: u8 = 0b01000;
const ERROR: u8 = 0b10000;

impl Event {
    /// Create a new event from IOCP completion parameters.
    pub fn from_completion(key: usize, bytes: u32, error: Option<std::io::Error>) -> Self {
        let flags = if error.is_some() {
            ERROR
        } else {
            // On IOCP, a successful completion means the fd is ready
            READABLE | WRITABLE
        };

        Self {
            key,
            bytes,
            error,
            flags,
        }
    }

    /// Set readiness flags explicitly.
    pub fn with_flags(mut self, flags: u8) -> Self {
        self.flags = flags;
        self
    }

    /// Which token was registered for this event.
    pub fn token(&self) -> crate::poll::Token {
        crate::poll::Token(self.key)
    }

    /// Returns `true` if the event indicates readability.
    pub fn is_readable(&self) -> bool {
        self.flags & READABLE != 0
    }

    /// Returns `true` if the event indicates writability.
    pub fn is_writable(&self) -> bool {
        self.flags & WRITABLE != 0
    }

    /// Returns `true` if the event indicates an error condition.
    pub fn is_error(&self) -> bool {
        self.flags & ERROR != 0 || self.error.is_some()
    }

    /// Returns `true` if the read side is closed.
    pub fn is_read_closed(&self) -> bool {
        self.flags & READ_CLOSED != 0
    }

    /// Returns `true` if the write side is closed.
    pub fn is_write_closed(&self) -> bool {
        self.flags & WRITE_CLOSED != 0
    }

    /// Returns `true` if the event indicates priority/out-of-band data.
    /// Not applicable to IOCP.
    pub fn is_priority(&self) -> bool {
        false
    }

    /// Number of bytes transferred (for overlapped I/O completions).
    pub fn bytes(&self) -> u32 {
        self.bytes
    }

    /// The underlying I/O error, if any.
    pub fn error(&self) -> Option<&std::io::Error> {
        self.error.as_ref()
    }
}

impl Default for Event {
    fn default() -> Self {
        Self {
            key: 0,
            bytes: 0,
            error: None,
            flags: 0,
        }
    }
}

impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Event")
            .field("token", &self.token())
            .field("readable", &self.is_readable())
            .field("writable", &self.is_writable())
            .field("error", &self.is_error())
            .field("read_closed", &self.is_read_closed())
            .field("write_closed", &self.is_write_closed())
            .field("priority", &self.is_priority())
            .finish()
    }
}
