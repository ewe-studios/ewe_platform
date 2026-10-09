//! The seam between the REPL's drawing and whatever surface it draws on.
//!
//! WHY: everything the REPL renders — the box, its padding, the wrapped text,
//! the activity indicator — is computed into a cell buffer and never touches a
//! terminal directly. The only genuinely surface-specific parts are creating
//! the thing that flushes those cells, putting it into the mode the REPL needs,
//! and knowing whether the surface has a scrollback of its own. A host supplies
//! exactly those three, and nothing else.
//!
//! WHAT: [`ReplHost`] is implemented once per surface — a terminal via
//! crossterm, a browser via ratzilla, or a test double over ratatui's
//! `TestBackend`. [`ViewportMode`] tells the renderer which of the two output
//! strategies the surface supports.
//!
//! HOW: the renderer is generic over the host, so adding a surface means
//! implementing this trait and nothing else. See
//! [`ReplRenderer`](crate::ReplRenderer).

use std::io;

use ratatui::backend::Backend;

/// Where a host's output goes once it scrolls off the input box.
///
/// WHY: a terminal already owns a scrollback buffer and can be asked to push
/// lines into it. A browser canvas owns nothing — if the renderer does not keep
/// the history itself, the text is simply gone. The two need different
/// strategies, and only the host knows which it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewportMode {
    /// The surface has its own scrollback.
    ///
    /// The input box is pinned below existing output and finished lines are
    /// handed to the surface, which scrolls them away exactly as it scrolls
    /// anything else. Selection, scrollback search and copy all keep working
    /// because the text really is in the terminal.
    #[default]
    Inline,
    /// The renderer owns the whole surface and its history.
    ///
    /// Finished lines are kept in the renderer and redrawn above the box each
    /// frame. Needed for surfaces with no scrollback of their own, at the cost
    /// of the renderer having to hold the transcript in memory.
    Fullscreen,
}

impl core::fmt::Display for ViewportMode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Inline => f.write_str("inline"),
            Self::Fullscreen => f.write_str("fullscreen"),
        }
    }
}

/// A surface the REPL can be drawn on.
///
/// WHY: keeps [`ReplRenderer`](crate::ReplRenderer) free of any one backend, so
/// the box, the wrapping and the activity indicator are written once and work
/// anywhere ratatui can draw.
///
/// # Examples
///
/// A host over ratatui's own test backend, which is all it takes to render the
/// REPL off-screen and assert on the result:
///
/// ```
/// use std::io;
/// use foundation_repl::{ReplHost, ViewportMode};
/// use ratatui::backend::TestBackend;
///
/// struct OffScreen {
///     width: u16,
///     height: u16,
/// }
///
/// impl ReplHost for OffScreen {
///     type Backend = TestBackend;
///
///     fn create_backend(&mut self) -> io::Result<Self::Backend> {
///         Ok(TestBackend::new(self.width, self.height))
///     }
///
///     fn size(&mut self) -> io::Result<(u16, u16)> {
///         Ok((self.width, self.height))
///     }
///
///     fn viewport_mode(&self) -> ViewportMode {
///         ViewportMode::Fullscreen
///     }
/// }
/// ```
pub trait ReplHost {
    /// The ratatui backend this host draws through.
    ///
    /// Deliberately unconstrained beyond `Backend`: real backends disagree
    /// about their error type — crossterm and ratzilla report `io::Error`,
    /// while ratatui's own `TestBackend` cannot fail at all and reports
    /// `Infallible`. Pinning one would lock out the test backend, and with it
    /// the ability to render the REPL off-screen and assert on the result, so
    /// the renderer flattens whatever comes back through its `Display`.
    type Backend: Backend;

    /// Build a backend for the renderer to draw into.
    ///
    /// Called again whenever the input box changes height in
    /// [`ViewportMode::Inline`], because an inline viewport's height is fixed
    /// when its terminal is built. Implementations must be cheap and must keep
    /// writing to the same destination each time.
    ///
    /// # Errors
    /// Returns whatever the surface reports when it cannot be opened.
    fn create_backend(&mut self) -> io::Result<Self::Backend>;

    /// The surface's current size, as `(columns, rows)`.
    ///
    /// Queried before a backend exists, so it cannot be answered by one.
    ///
    /// # Errors
    /// Returns whatever the surface reports when its size is unavailable.
    fn size(&mut self) -> io::Result<(u16, u16)>;

    /// Which output strategy this surface supports.
    fn viewport_mode(&self) -> ViewportMode;

    /// Put the surface into the mode the REPL needs.
    ///
    /// Called once, before anything is drawn. A terminal enters raw mode here;
    /// a browser has nothing to do.
    ///
    /// # Errors
    /// Returns whatever the surface reports when it cannot be reconfigured.
    fn enter(&mut self) -> io::Result<()> {
        Ok(())
    }

    /// Undo whatever [`ReplHost::enter`] changed.
    ///
    /// Called when the REPL shuts down, and must be safe to call when `enter`
    /// was never reached or has already been undone.
    ///
    /// # Errors
    /// Returns whatever the surface reports when it cannot be restored.
    fn leave(&mut self) -> io::Result<()> {
        Ok(())
    }
}
