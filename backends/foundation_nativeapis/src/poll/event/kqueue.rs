/// macOS/BSD `Event` — wraps `libc::kevent`.

/// A single readiness event from `kevent()`.
#[derive(Clone, Copy)]
pub struct Event {
    event: libc::kevent,
}

impl Event {
    /// Create an Event from a raw kevent.
    #[inline]
    pub fn from_kevent(event: libc::kevent) -> Self {
        Self { event }
    }

    /// Which token was registered for this event.
    /// On kqueue, the token is stored in `udata` (cast from *const c_void).
    pub fn token(&self) -> crate::poll::Token {
        crate::poll::Token(self.event.udata as usize)
    }

    /// Returns `true` if the event indicates readability (`EVFILT_READ`).
    pub fn is_readable(&self) -> bool {
        self.event.filter == libc::EVFILT_READ as _
    }

    /// Returns `true` if the event indicates writability (`EVFILT_WRITE`).
    pub fn is_writable(&self) -> bool {
        self.event.filter == libc::EVFILT_WRITE as _
    }

    /// Returns `true` if the event contains an error.
    /// This fires on `EV_ERROR` flag, or on `EV_EOF` with non-zero `fflags`.
    pub fn is_error(&self) -> bool {
        (self.event.flags & libc::EV_ERROR as u16) != 0
            || ((self.event.flags & libc::EV_EOF as u16) != 0 && self.event.fflags != 0)
    }

    /// Returns `true` if the read side is closed.
    /// On kqueue: `EVFILT_READ` filter with `EV_EOF` flag set.
    pub fn is_read_closed(&self) -> bool {
        self.event.filter == libc::EVFILT_READ as _
            && (self.event.flags & libc::EV_EOF as u16) != 0
    }

    /// Returns `true` if the write side is closed.
    /// On kqueue: `EVFILT_WRITE` filter with `EV_EOF` flag set.
    pub fn is_write_closed(&self) -> bool {
        self.event.filter == libc::EVFILT_WRITE as _
            && (self.event.flags & libc::EV_EOF as u16) != 0
    }

    /// Returns `true` if the event indicates priority/out-of-band data.
    /// kqueue doesn't have a priority indicator, so this always returns `false`.
    pub fn is_priority(&self) -> bool {
        false
    }
}

impl Default for Event {
    fn default() -> Self {
        Self {
            event: unsafe { std::mem::zeroed() },
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
