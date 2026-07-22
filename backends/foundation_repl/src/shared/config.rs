//! Configuration types for customizing the REPL.
//!
//! [`ReplConfig`] holds prompt strings, banners, limits and the visual
//! [`ReplTheme`](crate::ReplTheme) the input box and output region are drawn in.

use core::fmt;

use crate::shared::theme::ReplTheme;

/// Full configuration for a REPL session.
///
/// WHY: everything a caller is likely to want to change — what the prompt says,
/// what the box looks like, how much text is accepted — lives in one value that
/// can be built once and reused.
///
/// WHAT: prompt strings, the optional banner and goodbye lines, an input length
/// ceiling, and the theme.
///
/// HOW: build one with [`Repl::builder()`](crate::Repl::builder), or start from
/// [`ReplConfig::default()`] and adjust fields directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplConfig {
    /// Primary prompt string (e.g. `"| "`).
    pub prompt: String,
    /// Continuation prompt for multiline input (e.g. `"|... "`).
    pub continuation_prompt: String,
    /// Welcome banner shown on startup.
    pub banner: Option<String>,
    /// Goodbye message shown on exit.
    pub goodbye: Option<String>,
    /// Maximum input length in bytes. `None` means unlimited.
    pub max_input_length: Option<usize>,
    /// Colours, border and padding used to draw the REPL.
    pub theme: ReplTheme,
}

impl Default for ReplConfig {
    fn default() -> Self {
        Self {
            prompt: "| ".into(),
            continuation_prompt: "|... ".into(),
            banner: None,
            goodbye: None,
            max_input_length: Some(64 * 1024),
            theme: ReplTheme::default(),
        }
    }
}

impl fmt::Display for ReplConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ReplConfig(prompt {:?}, max input {}, {})",
            self.prompt,
            self.max_input_length
                .map_or_else(|| "unlimited".to_string(), |limit| limit.to_string()),
            self.theme
        )
    }
}
