//! The terminal host: crossterm raw mode over stderr.
//!
//! WHY: a terminal is one surface the REPL can draw on, not the only one. Every
//! part of the drawing lives in [`ReplRenderer`], so all this file supplies is
//! the three things that are genuinely specific to a terminal — opening a
//! backend over stderr, reading the window size, and entering raw mode.
//!
//! WHAT: [`CrosstermHost`], plus [`NativeDisplay`] as the renderer built on it.
//!
//! HOW: output goes to stderr so stdout stays free for a caller's real output
//! being piped somewhere. The terminal has its own scrollback, so finished
//! lines are handed to it rather than retained.

use std::io::{self, Stderr};

use ratatui::backend::CrosstermBackend;

use crate::shared::host::{ReplHost, ViewportMode};
use crate::shared::render::ReplRenderer;
use crate::shared::theme::ReplTheme;

/// Draws the REPL into a terminal via crossterm.
#[derive(Debug, Default, Clone, Copy)]
pub struct CrosstermHost {
    raw_mode: bool,
}

impl CrosstermHost {
    /// A host writing to stderr.
    #[must_use]
    pub const fn new() -> Self {
        Self { raw_mode: false }
    }
}

impl core::fmt::Display for CrosstermHost {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "CrosstermHost(raw mode {})", self.raw_mode)
    }
}

impl ReplHost for CrosstermHost {
    type Backend = CrosstermBackend<Stderr>;

    fn create_backend(&mut self) -> io::Result<Self::Backend> {
        Ok(CrosstermBackend::new(io::stderr()))
    }

    fn size(&mut self) -> io::Result<(u16, u16)> {
        crossterm::terminal::size()
    }

    fn viewport_mode(&self) -> ViewportMode {
        // A terminal owns a scrollback, so finished lines are given to it and
        // stay selectable and searchable like any other output.
        ViewportMode::Inline
    }

    fn enter(&mut self) -> io::Result<()> {
        if !self.raw_mode {
            crossterm::terminal::enable_raw_mode()?;
            self.raw_mode = true;
        }
        Ok(())
    }

    fn leave(&mut self) -> io::Result<()> {
        if self.raw_mode {
            crossterm::terminal::disable_raw_mode()?;
            self.raw_mode = false;
        }
        Ok(())
    }
}

/// The REPL renderer wired to a terminal.
pub type NativeDisplay = ReplRenderer<CrosstermHost>;

/// Build the terminal renderer for `theme`.
#[must_use]
pub fn native_display(theme: ReplTheme) -> NativeDisplay {
    ReplRenderer::new(CrosstermHost::new(), theme)
}
