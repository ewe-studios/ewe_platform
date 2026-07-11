//! Per-stream and per-connection flow-control window arithmetic (RFC 7540 §6.9).
//!
//! WHY: HTTP/2 flow control is window-based — each side advertises how many bytes
//! it is willing to receive, and the sender must not exceed that window. A
//! `SETTINGS_INITIAL_WINDOW_SIZE` change can push a window negative (the peer has
//! already used capacity at the old, larger window), so the internal accumulator
//! is signed. The wire type (`WindowSize`) is u31 (0 ..= 2³¹−1).
//!
//! WHAT: [`Window`] (signed accumulator), [`FlowControl`] (send-side tracker with
//! `WINDOW_UPDATE` threshold), and the `WindowSize` / `MAX_WINDOW_SIZE` constants.
//!
//! HOW: Pure arithmetic — no I/O, no allocation. The `WINDOW_UPDATE` threshold uses
//! integer ratio (1/2) to avoid float math; a window increment is sent only when
//! the unclaimed capacity reaches half the peer-known window.

/// The maximum flow-control window size (RFC 7540 §6.9.2: 2³¹−1).
pub const MAX_WINDOW_SIZE: u32 = 2_147_483_647;

/// Wire-level window increment — always a u31 (0 ..= `MAX_WINDOW_SIZE`).
pub type WindowSize = u32;

/// The ratio applied to decide when to emit a `WINDOW_UPDATE` frame.
///
/// We aggregate small increments: a `WINDOW_UPDATE` is sent only when the
/// unclaimed capacity reaches `window_size * NUMERATOR / DENOMINATOR`.
const UNCLAIMED_NUMERATOR: i32 = 1;
const UNCLAIMED_DENOMINATOR: i32 = 2;

/// An HTTP/2 flow-control error — returned when arithmetic overflows or a
/// window bound is violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlowControlError;

impl std::fmt::Display for FlowControlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("flow-control error")
    }
}

impl std::error::Error for FlowControlError {}

// ── Window ──────────────────────────────────────────────────────────────────

/// A signed window accumulator (RFC 7540 §6.9).
///
/// WHY: The internal value is `i32` because a `SETTINGS_INITIAL_WINDOW_SIZE`
/// reduction can push a window negative — the peer already consumed bytes at the
/// old, larger window, so the effective window is `old - used - reduction`.
/// Externally the window is clamped to non-negative (`as_size`).
///
/// WHAT: Bounded arithmetic that returns [`FlowControlError`] on overflow or
/// underflow instead of panicking.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd)]
pub struct Window(i32);

impl Window {
    /// Create a new window initialised to zero.
    #[must_use]
    pub fn new() -> Self {
        Self(0)
    }

    /// The window as a non-negative [`WindowSize`] — negative values are clamped to 0.
    #[must_use]
    pub fn as_size(self) -> WindowSize {
        if self.0 < 0 {
            0
        } else {
            self.0 as WindowSize
        }
    }

    /// The window as a [`WindowSize`], panicking if negative.
    ///
    /// # Panics
    /// Panics if the window is negative.
    #[must_use]
    pub fn checked_size(self) -> WindowSize {
        assert!(self.0 >= 0, "negative Window");
        self.0 as WindowSize
    }

    /// Decrease the window by `other`, returning an error on underflow.
    pub fn decrease_by(&mut self, other: WindowSize) -> Result<(), FlowControlError> {
        self.0 = self.0.checked_sub(other as i32).ok_or(FlowControlError)?;
        Ok(())
    }

    /// Increase the window by `other`, returning an error on overflow.
    pub fn increase_by(&mut self, other: WindowSize) -> Result<(), FlowControlError> {
        self.0 = self.checked_add(other)?.0;
        Ok(())
    }

    /// Add `other` to this window, returning the new [`Window`] or an error on overflow.
    #[must_use]
    pub fn checked_add(self, other: WindowSize) -> Result<Self, FlowControlError> {
        let v = self.0.checked_add(other as i32).ok_or(FlowControlError)?;
        Ok(Self(v))
    }

    /// The raw signed value — for tests and diagnostics.
    #[must_use]
    pub fn raw(self) -> i32 {
        self.0
    }
}

impl std::fmt::Display for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl PartialEq<usize> for Window {
    fn eq(&self, other: &usize) -> bool {
        if self.0 < 0 {
            false
        } else {
            (self.0 as usize).eq(other)
        }
    }
}

impl PartialOrd<usize> for Window {
    fn partial_cmp(&self, other: &usize) -> Option<std::cmp::Ordering> {
        if self.0 < 0 {
            Some(std::cmp::Ordering::Less)
        } else {
            (self.0 as usize).partial_cmp(other)
        }
    }
}

// ── FlowControl ─────────────────────────────────────────────────────────────

/// Per-stream or per-connection flow-control state.
///
/// Tracks two views of the window:
/// - `window_size` — what the **peer** knows (the window we've advertised to them).
/// - `available` — what **we** know is available for the peer to consume.
///
/// Both can go negative after a `SETTINGS_INITIAL_WINDOW_SIZE` reduction.
#[derive(Clone, Copy, Debug)]
pub struct FlowControl {
    /// The window as known by the peer (the receive window we advertised).
    window_size: Window,
    /// The window available to the consumer (the send window the peer gave us).
    available: Window,
}

impl FlowControl {
    /// Create a new flow-control tracker with both windows at zero.
    #[must_use]
    pub fn new() -> Self {
        Self {
            window_size: Window::new(),
            available: Window::new(),
        }
    }

    /// The window size the peer knows about (non-negative).
    #[must_use]
    pub fn window_size(&self) -> WindowSize {
        self.window_size.as_size()
    }

    /// The available capacity.
    #[must_use]
    pub fn available(&self) -> Window {
        self.available
    }

    /// True when the peer-known window exceeds available capacity — we have
    /// bytes the peer hasn't been told about yet (`WINDOW_UPDATE` pending).
    #[must_use]
    pub fn has_unavailable(&self) -> bool {
        if self.window_size.0 < 0 {
            false
        } else {
            self.window_size > self.available
        }
    }

    /// Claim capacity from the available window before sending data.
    ///
    /// # Errors
    /// [`FlowControlError`] if the window would underflow.
    pub fn claim_capacity(&mut self, capacity: WindowSize) -> Result<(), FlowControlError> {
        self.available.decrease_by(capacity)
    }

    /// Return capacity to the available window (e.g. after receiving a `WINDOW_UPDATE`).
    ///
    /// # Errors
    /// [`FlowControlError`] if the window would overflow.
    pub fn assign_capacity(&mut self, capacity: WindowSize) -> Result<(), FlowControlError> {
        self.available.increase_by(capacity)
    }

    /// How much capacity should be sent in a `WINDOW_UPDATE` frame, if any.
    ///
    /// Returns `Some(increment)` when the unclaimed capacity reaches the
    /// threshold (1/2 of the peer-known window), signalling a `WINDOW_UPDATE`
    /// should be emitted. Returns `None` otherwise — small increments are
    /// aggregated to avoid flooding the peer with tiny updates.
    #[must_use]
    pub fn unclaimed_capacity(&self) -> Option<WindowSize> {
        let available = self.available;

        if self.window_size >= available {
            return None;
        }

        let unclaimed = available.0 - self.window_size.0;
        let threshold = self.window_size.0 / UNCLAIMED_DENOMINATOR * UNCLAIMED_NUMERATOR;

        if unclaimed < threshold {
            None
        } else {
            Some(unclaimed as WindowSize)
        }
    }

    /// Increase the peer-known window (called after receiving a `WINDOW_UPDATE`).
    ///
    /// # Errors
    /// [`FlowControlError`] on overflow or if the result exceeds [`MAX_WINDOW_SIZE`].
    pub fn inc_window(&mut self, sz: WindowSize) -> Result<(), FlowControlError> {
        let (val, overflow) = self.window_size.0.overflowing_add(sz as i32);
        if overflow || val > MAX_WINDOW_SIZE as i32 {
            return Err(FlowControlError);
        }
        self.window_size = Window(val);
        Ok(())
    }

    /// Decrease the send-side window (called when a lower `SETTINGS_INITIAL_WINDOW_SIZE`
    /// is received). Only `window_size` is adjusted — the peer is reducing what it
    /// advertised.
    ///
    /// # Errors
    /// [`FlowControlError`] on underflow.
    pub fn dec_send_window(&mut self, sz: WindowSize) -> Result<(), FlowControlError> {
        self.window_size.decrease_by(sz)
    }

    /// Decrease the receive-side window (called when our own SETTINGS ACK with a
    /// lower `INITIAL_WINDOW_SIZE` is sent). Both `window_size` and `available` are
    /// reduced — we are shrinking what we told the peer.
    ///
    /// # Errors
    /// [`FlowControlError`] on underflow.
    pub fn dec_recv_window(&mut self, sz: WindowSize) -> Result<(), FlowControlError> {
        self.window_size.decrease_by(sz)?;
        self.available.decrease_by(sz)?;
        Ok(())
    }

    /// Record that `sz` bytes have been sent. Both windows are reduced.
    ///
    /// Zero-sized sends are a no-op.
    ///
    /// # Panics
    /// Panics if `sz > window_size` — the caller must ensure capacity.
    ///
    /// # Errors
    /// [`FlowControlError`] on underflow.
    pub fn send_data(&mut self, sz: WindowSize) -> Result<(), FlowControlError> {
        if sz == 0 {
            return Ok(());
        }
        assert!(
            self.window_size.0 >= sz as i32,
            "send_data({sz}) exceeds window_size {}",
            self.window_size
        );
        self.window_size.decrease_by(sz)?;
        self.available.decrease_by(sz)?;
        Ok(())
    }
}
