//! `Ready` — bitmask of readiness and lifecycle states on a file descriptor.

/// Bitmask of readiness and lifecycle states observed on a file descriptor.
///
/// ## Why CLOSED and ERROR flags are needed
///
/// On edge-triggered pollers, when a pipe's write end closes, epoll returns
/// `EPOLLIN | EPOLLHUP`. The `EPOLLIN` sets READABLE, but `EPOLLHUP` means
/// the connection is permanently broken — it will NEVER transition back to
/// "not ready". Without a CLOSED flag, the fd appears permanently readable,
/// causing a busy-wait loop.
///
/// Similarly, `EPOLLERR` can arrive without `EPOLLIN` or `EPOLLOUT`. Without
/// an ERROR flag, the fd would appear "not ready" when it actually has an
/// error condition that needs handling.
///
/// ## Platform mapping
/// | Flag        | epoll                              | kqueue                          |
/// |-------------|-------------------------------------|----------------------------------|
/// | READABLE    | EPOLLIN                             | EVFILT_READ                      |
/// | WRITABLE    | EPOLLOUT                            | EVFILT_WRITE                     |
/// | READ_CLOSED | EPOLLHUP or (EPOLLIN+EPOLLRDHUP)    | EVFILT_READ + EV_EOF             |
/// | WRITE_CLOSED| EPOLLHUP or (EPOLLOUT+EPOLLERR)     | EVFILT_WRITE + EV_EOF            |
/// | ERROR       | EPOLLERR                            | EVFILT_READ/WRITE + EV_EOF+fflags |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ready(u8);

impl Ready {
    pub const READABLE:    Ready = Ready(0b00001);
    pub const WRITABLE:    Ready = Ready(0b00010);
    pub const READ_CLOSED: Ready = Ready(0b00100);
    pub const WRITE_CLOSED: Ready = Ready(0b01000);
    pub const ERROR:       Ready = Ready(0b10000);
    pub const EMPTY:       Ready = Ready(0b00000);

    pub fn is_readable(self)     -> bool { self.0 & Self::READABLE.0 != 0 }
    pub fn is_writable(self)     -> bool { self.0 & Self::WRITABLE.0 != 0 }
    pub fn is_read_closed(self)  -> bool { self.0 & Self::READ_CLOSED.0 != 0 }
    pub fn is_write_closed(self) -> bool { self.0 & Self::WRITE_CLOSED.0 != 0 }
    pub fn is_error(self)        -> bool { self.0 & Self::ERROR.0 != 0 }
    pub fn is_empty(self)        -> bool { self.0 == 0 }

    /// Combine readiness states.
    pub fn union(self, other: Ready) -> Ready { Ready(self.0 | other.0) }

    /// Remove specific readiness states.
    pub fn difference(self, other: Ready) -> Ready { Ready(self.0 & !other.0) }

    /// Intersection — readiness flags present in both.
    pub fn intersection(self, other: Ready) -> Ready { Ready(self.0 & other.0) }

    /// Check if this readiness contains all flags in `other`.
    pub fn contains(self, other: Ready) -> bool { self.0 & other.0 == other.0 }

    /// WHY: the shared reactor stores readiness in an `AtomicU8` so the drain
    /// thread can set bits and consumers can clear them without taking a write
    /// lock on the registration map.
    ///
    /// WHAT: the raw bit pattern backing this `Ready`.
    ///
    /// HOW: direct field read; the bit layout is the `Ready::*` constants.
    ///
    /// # Panics
    /// Never panics.
    pub const fn bits(self) -> u8 { self.0 }

    /// WHY: counterpart to [`Ready::bits`] for reading back an atomically
    /// stored readiness set.
    ///
    /// WHAT: rebuild a `Ready` from a raw bit pattern.
    ///
    /// HOW: unknown high bits are masked off, so a corrupted or future-widened
    /// value can never produce a `Ready` claiming flags this build doesn't know.
    ///
    /// # Panics
    /// Never panics.
    pub const fn from_bits(bits: u8) -> Ready { Ready(bits & Self::ALL.0) }

    /// Every readiness flag this build understands.
    pub const ALL: Ready = Ready(0b11111);
}

impl std::fmt::Display for Ready {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.is_empty() {
            return write!(f, "EMPTY");
        }
        let mut first = true;
        for (flag, name) in [
            (Self::READABLE, "READABLE"),
            (Self::WRITABLE, "WRITABLE"),
            (Self::READ_CLOSED, "READ_CLOSED"),
            (Self::WRITE_CLOSED, "WRITE_CLOSED"),
            (Self::ERROR, "ERROR"),
        ] {
            if self.contains(flag) {
                if !first {
                    write!(f, "|")?;
                }
                write!(f, "{name}")?;
                first = false;
            }
        }
        Ok(())
    }
}
