/// Container for readiness events returned by `Poll::poll()`.
///
/// This is a reusable buffer — the same `Events` instance can be passed to
/// multiple `poll()` calls. Its capacity is set at creation time and cannot
/// be changed.

use crate::poll::event::Event;

/// Holds readiness events from the poll selector.
pub struct Events {
    /// Platform-specific event storage.
    events: Vec<Event>,
}

impl Events {
    /// Create a new `Events` container with the given capacity.
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            events: Vec::with_capacity(cap),
        }
    }

    /// Number of events currently in the container.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether the container is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Iterate over the events.
    pub fn iter(&self) -> impl Iterator<Item = Event> + use<'_> {
        self.events.iter().copied()
    }

    /// Clear all events from the container.
    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Returns a mutable slice of the underlying event storage.
    /// This is used internally by the poll layer to write events.
    #[doc(hidden)]
    pub fn as_mut_slice(&mut self) -> &mut [Event] {
        &mut self.events
    }

    /// Set the length of events (used internally by platform selectors).
    #[doc(hidden)]
    pub fn set_len(&mut self, n: usize) {
        assert!(n <= self.events.capacity());
        unsafe {
            self.events.set_len(n);
        }
    }
}
