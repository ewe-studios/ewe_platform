//! A browser surface, built on any [ratzilla] backend.
//!
//! WHY: the REPL's drawing is already backend-neutral — it measures into a cell
//! buffer and flushes through ratatui's `Backend`. ratzilla implements that
//! trait against the DOM, a `<canvas>` and WebGL2, so the box, the padding, the
//! wrapping and the activity indicator all render in a browser without a line
//! of them changing.
//!
//! WHAT: [`RatzillaHost`] adapts a ratzilla backend to [`ReplHost`], plus
//! [`ratzilla_display`] to build the renderer in one call.
//!
//! HOW: a browser has no scrollback to push finished lines into, so this host
//! reports [`ViewportMode::Fullscreen`] and the renderer keeps the transcript
//! itself, redrawing it above the box each frame.
//!
//! # What this does and does not give you
//!
//! This supplies the **rendering** half. It does not turn
//! [`Repl::messages()`](crate::Repl::messages) into something a browser can
//! drive: that iterator blocks waiting on a keystroke, and a browser tab has no
//! blocking read — input arrives as `keydown` callbacks, and frames are painted
//! from `requestAnimationFrame`. Drive this renderer from ratzilla's
//! `on_key_event` and call [`ReplDisplay`](crate::ReplDisplay) methods yourself.
//!
//! [ratzilla]: https://github.com/ratatui/ratzilla
//!
//! # Examples
//!
//! ```ignore
//! use foundation_repl::{ratzilla_display, InputView, ReplDisplay, ReplTheme};
//! use ratzilla::DomBackend;
//!
//! let backend = DomBackend::new()?;
//! let mut display = ratzilla_display(backend, ReplTheme::default())?;
//!
//! display.render_input(&InputView {
//!     prompt: "| ",
//!     continuation: "|... ",
//!     buffer: "hello",
//!     cursor: 5,
//! });
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::io;

use ratatui::backend::Backend;

use crate::shared::host::{ReplHost, ViewportMode};
use crate::shared::render::ReplRenderer;
use crate::shared::theme::ReplTheme;

/// Adapts a ratzilla backend — or any other browser [`Backend`] — to the REPL.
///
/// WHY it holds the backend rather than building one on demand: a browser
/// backend owns real DOM nodes or a GPU context, so it is created once by the
/// caller and handed over, unlike a terminal backend that is a cheap wrapper
/// around a file descriptor.
#[derive(Debug)]
pub struct RatzillaHost<B: Backend> {
    backend: Option<B>,
    size: (u16, u16),
}

impl<B: Backend> RatzillaHost<B> {
    /// Wrap an already-constructed browser backend.
    ///
    /// # Errors
    /// Returns an error if the backend will not report its size.
    pub fn new(backend: B) -> io::Result<Self> {
        let size = backend
            .size()
            .map_err(|error| io::Error::other(error.to_string()))?;

        Ok(Self {
            backend: Some(backend),
            size: (size.width.max(1), size.height.max(1)),
        })
    }

    /// Tell the host the surface changed size.
    ///
    /// A browser has no `SIGWINCH`; the page has to forward its resize events,
    /// or the REPL keeps wrapping to the size the surface started at.
    pub fn set_size(&mut self, width: u16, height: u16) {
        self.size = (width.max(1), height.max(1));
    }
}

impl<B: Backend> core::fmt::Display for RatzillaHost<B> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "RatzillaHost({}x{}, {})",
            self.size.0,
            self.size.1,
            if self.backend.is_some() {
                "unused"
            } else {
                "in use"
            }
        )
    }
}

impl<B: Backend> ReplHost for RatzillaHost<B> {
    type Backend = B;

    fn create_backend(&mut self) -> io::Result<Self::Backend> {
        // Only ever called once: a fullscreen viewport is built a single time,
        // unlike an inline one that is rebuilt whenever the box changes height.
        self.backend.take().ok_or_else(|| {
            io::Error::other(
                "the browser backend was already handed to a renderer; \
                 build a new RatzillaHost for a second renderer",
            )
        })
    }

    fn size(&mut self) -> io::Result<(u16, u16)> {
        Ok(self.size)
    }

    fn viewport_mode(&self) -> ViewportMode {
        // A browser surface has no scrollback of its own, so the renderer keeps
        // the transcript and redraws it.
        ViewportMode::Fullscreen
    }
}

/// Build a REPL renderer that draws into a browser backend.
///
/// # Errors
/// Returns an error if the backend will not report its size.
pub fn ratzilla_display<B: Backend>(
    backend: B,
    theme: ReplTheme,
) -> io::Result<ReplRenderer<RatzillaHost<B>>> {
    Ok(ReplRenderer::new(RatzillaHost::new(backend)?, theme))
}
