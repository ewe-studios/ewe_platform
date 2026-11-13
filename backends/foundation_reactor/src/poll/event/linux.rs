/// A single readiness event from `epoll_wait()`.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Event {
    event: libc::epoll_event,
}

impl Event {
    /// The event mask as a `c_int`.
    ///
    /// The kernel and the `io_uring` adapter set only event bits below bit 31.
    /// Thus the conversion cannot fail. A zero fallback keeps every predicate
    /// false if it ever did.
    fn mask(&self) -> libc::c_int {
        libc::c_int::try_from(self.event.events).unwrap_or(0)
    }

    /// Which token was registered for this event.
    #[must_use]
    pub fn token(&self) -> super::super::Token {
        // The token is stored from a `usize` at registration, so the
        // conversion cannot fail. The fallback is unreachable.
        super::super::Token(usize::try_from(self.event.u64).unwrap_or(usize::MAX))
    }

    /// Returns `true` if the event indicates readability.
    #[must_use]
    pub fn is_readable(&self) -> bool {
        (self.mask() & libc::EPOLLIN) != 0 || (self.mask() & libc::EPOLLPRI) != 0
    }

    /// Returns `true` if the event indicates writability.
    #[must_use]
    pub fn is_writable(&self) -> bool {
        (self.mask() & libc::EPOLLOUT) != 0
    }

    /// Returns `true` if the event indicates an error condition (`EPOLLERR`).
    #[must_use]
    pub fn is_error(&self) -> bool {
        (self.mask() & libc::EPOLLERR) != 0
    }

    /// Returns `true` if the read side is closed.
    /// This fires on `EPOLLHUP` or on `EPOLLIN | EPOLLRDHUP` (peer shutdown).
    #[must_use]
    pub fn is_read_closed(&self) -> bool {
        (self.mask() & libc::EPOLLHUP) != 0
            || ((self.mask() & libc::EPOLLIN) != 0 && (self.mask() & libc::EPOLLRDHUP) != 0)
    }

    /// Returns `true` if the write side is closed.
    /// This fires on `EPOLLHUP`, or on `EPOLLOUT | EPOLLERR`,
    /// or on `EPOLLERR` alone (pipe read end closed).
    #[must_use]
    pub fn is_write_closed(&self) -> bool {
        (self.mask() & libc::EPOLLHUP) != 0
            || ((self.mask() & libc::EPOLLOUT) != 0 && (self.mask() & libc::EPOLLERR) != 0)
            || (self.mask() == libc::EPOLLERR)
    }

    /// Returns `true` if the event indicates priority/out-of-band data (`EPOLLPRI`).
    #[must_use]
    pub fn is_priority(&self) -> bool {
        (self.mask() & libc::EPOLLPRI) != 0
    }
}

impl Event {
    /// WHY: the io_uring selector synthesises events from CQE poll masks rather
    /// than receiving them from `epoll_wait`. Without this it would have to
    /// `transmute` a `libc::epoll_event` into an `Event`.
    ///
    /// WHAT: build an `Event` from an epoll-style event bitmask and a token.
    ///
    /// HOW: `Event` is `repr(transparent)` over `libc::epoll_event`, so the
    /// bitmask and token populate that struct directly. `events` must use the
    /// `EPOLL*` bit values; the `POLL*` values from a poll mask coincide with
    /// them for IN/OUT/ERR/HUP/PRI/RDHUP on Linux.
    ///
    /// # Panics
    /// Never panics.
    #[cfg(feature = "uring")]
    pub(crate) fn from_parts(events: u32, token: super::super::Token) -> Self {
        Self {
            event: libc::epoll_event {
                events,
                u64: token.0 as u64,
            },
        }
    }
}

impl Default for Event {
    fn default() -> Self {
        Self {
            event: libc::epoll_event { events: 0, u64: 0 },
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
