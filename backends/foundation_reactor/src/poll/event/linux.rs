
/// A single readiness event from `epoll_wait()`.
#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Event {
    event: libc::epoll_event,
}

impl Event {
    /// Which token was registered for this event.
    pub fn token(&self) -> super::super::Token {
        super::super::Token(self.event.u64 as usize)
    }

    /// Returns `true` if the event indicates readability.
    pub fn is_readable(&self) -> bool {
        (self.event.events as libc::c_int & libc::EPOLLIN) != 0
            || (self.event.events as libc::c_int & libc::EPOLLPRI) != 0
    }

    /// Returns `true` if the event indicates writability.
    pub fn is_writable(&self) -> bool {
        (self.event.events as libc::c_int & libc::EPOLLOUT) != 0
    }

    /// Returns `true` if the event indicates an error condition (`EPOLLERR`).
    pub fn is_error(&self) -> bool {
        (self.event.events as libc::c_int & libc::EPOLLERR) != 0
    }

    /// Returns `true` if the read side is closed.
    /// This fires on `EPOLLHUP` or on `EPOLLIN | EPOLLRDHUP` (peer shutdown).
    pub fn is_read_closed(&self) -> bool {
        (self.event.events as libc::c_int & libc::EPOLLHUP) != 0
            || ((self.event.events as libc::c_int & libc::EPOLLIN) != 0
                && (self.event.events as libc::c_int & libc::EPOLLRDHUP) != 0)
    }

    /// Returns `true` if the write side is closed.
    /// This fires on `EPOLLHUP`, or on `EPOLLOUT | EPOLLERR`,
    /// or on `EPOLLERR` alone (pipe read end closed).
    pub fn is_write_closed(&self) -> bool {
        (self.event.events as libc::c_int & libc::EPOLLHUP) != 0
            || ((self.event.events as libc::c_int & libc::EPOLLOUT) != 0
                && (self.event.events as libc::c_int & libc::EPOLLERR) != 0)
            || (self.event.events as libc::c_int == libc::EPOLLERR)
    }

    /// Returns `true` if the event indicates priority/out-of-band data (`EPOLLPRI`).
    pub fn is_priority(&self) -> bool {
        (self.event.events as libc::c_int & libc::EPOLLPRI) != 0
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
            event: libc::epoll_event {
                events: 0,
                u64: 0,
            },
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
