#[cfg(feature = "colors")]
use crossterm::style::Color;

/// Color configuration for the REPL display.
#[cfg(feature = "colors")]
#[derive(Debug, Clone)]
pub struct ReplColors {
    /// Prompt foreground color. Default: DarkGreen.
    pub prompt_color: Color,
    /// Continuation prompt foreground color. Default: DarkYellow.
    pub continuation_prompt_color: Color,
    /// Response text foreground color. Default: None (no change).
    pub response_color: Option<Color>,
    /// Error text foreground color. Default: Red.
    pub error_color: Color,
}

#[cfg(feature = "colors")]
impl Default for ReplColors {
    fn default() -> Self {
        Self {
            prompt_color: Color::DarkGreen,
            continuation_prompt_color: Color::DarkYellow,
            response_color: None,
            error_color: Color::Red,
        }
    }
}

/// Full configuration for a REPL session.
#[derive(Debug, Clone)]
pub struct ReplConfig {
    /// Primary prompt string (e.g. ">>> ").
    pub prompt: String,
    /// Continuation prompt for multiline input (e.g. "... ").
    pub continuation_prompt: String,
    /// Welcome banner shown on startup.
    pub banner: Option<String>,
    /// Goodbye message shown on exit.
    pub goodbye: Option<String>,
    /// Maximum input length (chars). None = unlimited.
    pub max_input_length: Option<usize>,
    #[cfg(feature = "colors")]
    pub colors: ReplColors,
}

impl Default for ReplConfig {
    fn default() -> Self {
        Self {
            prompt: ">>> ".into(),
            continuation_prompt: "... ".into(),
            banner: None,
            goodbye: None,
            max_input_length: Some(64 * 1024),
            #[cfg(feature = "colors")]
            colors: ReplColors::default(),
        }
    }
}
