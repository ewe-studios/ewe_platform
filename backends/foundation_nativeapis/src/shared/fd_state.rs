/// File descriptor readiness state.
///
/// Indicates which readiness states were observed on a file descriptor.

/// Which readiness state was signaled on a file descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FdState {
    Readable,
    Writable,
    Both,
}

impl FdState {
    /// Construct from a `Ready` bitmask (from `native::fd`), keeping only
    /// readable/writable states.
    ///
    /// Returns `None` if the readiness contains neither readable nor writable
    /// (e.g. only closed/error flags).
    #[inline]
    pub fn from_ready(readable: bool, writable: bool) -> Option<Self> {
        match (readable, writable) {
            (true, true) => Some(FdState::Both),
            (true, false) => Some(FdState::Readable),
            (false, true) => Some(FdState::Writable),
            (false, false) => None,
        }
    }
}
