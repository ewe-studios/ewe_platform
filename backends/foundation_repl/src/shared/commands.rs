//! Command registry and dispatch.
//!
//! Lines starting with `/` are intercepted by [`CommandRegistry`] and
//! handled before reaching the [`ReplMessageIter`](super::repl::ReplMessageIter).
//! Built-in commands: `/help`, `/clear`, `/exit`, `/version`.
//! Custom commands are registered via
//! [`Repl::register_command()`](crate::Repl::register_command).

use core::fmt;
use std::collections::BTreeMap;

/// Sentinel a handler returns to ask the REPL to shut down.
const EXIT_SENTINEL: &str = "__EXIT__";

/// Sentinel a handler returns to ask the REPL to clear the screen.
const CLEAR_SENTINEL: &str = "__CLEAR__";

/// A command handler — receives the full command text (e.g. `/greet Alice`)
/// and returns the text to display.
pub type CommandHandler = Box<dyn Fn(&str) -> String + Send + 'static>;

/// Registered commands in the REPL.
pub struct CommandRegistry {
    commands: BTreeMap<String, CommandHandler>,
}

impl CommandRegistry {
    /// Create a registry holding only the built-in commands.
    #[must_use]
    pub fn new() -> Self {
        let mut reg = Self {
            commands: BTreeMap::new(),
        };
        reg.register_builtins();
        reg
    }

    fn register_builtins(&mut self) {
        self.commands.insert(
            "help".into(),
            Box::new(|_| {
                "Available commands:\n  /help     Show this help\n  /clear    Clear the screen\n  /exit     Exit the REPL\n  /version  Show version".into()
            }),
        );

        self.commands
            .insert("clear".into(), Box::new(|_| CLEAR_SENTINEL.into()));

        self.commands
            .insert("exit".into(), Box::new(|_| EXIT_SENTINEL.into()));

        self.commands.insert(
            "version".into(),
            Box::new(|_| format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))),
        );
    }

    /// Register a custom command. The name is used without its leading `/`.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        handler: impl Fn(&str) -> String + Send + 'static,
    ) {
        self.commands.insert(name.into(), Box::new(handler));
    }

    /// Names of every registered command, in order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.commands.keys().map(String::as_str)
    }

    /// Check whether a line is a command, and handle it if so.
    ///
    /// Returns `None` for anything that is not a command, so the caller can
    /// treat it as ordinary input.
    #[must_use]
    pub fn try_handle(&self, line: &str) -> Option<CommandResult> {
        let line = line.trim();
        if !line.starts_with('/') {
            return None;
        }

        let name = line
            .strip_prefix('/')
            .and_then(|rest| rest.split_whitespace().next())
            .unwrap_or_default()
            .to_lowercase();

        let Some(handler) = self.commands.get(&name) else {
            return Some(CommandResult::Unknown);
        };

        Some(match handler(line).as_str() {
            EXIT_SENTINEL => CommandResult::Exit,
            CLEAR_SENTINEL => CommandResult::Clear,
            output => CommandResult::Output(output.to_string()),
        })
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for CommandRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommandRegistry")
            .field("commands", &self.commands.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl fmt::Display for CommandRegistry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CommandRegistry({} commands)", self.commands.len())
    }
}

/// Result of attempting to handle a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandResult {
    /// Print this text as a normal response.
    Output(String),
    /// The line looked like a command, but nothing is registered under that
    /// name.
    ///
    /// WHY this is not an error message: an application may implement its own
    /// slash commands on top of the registry, so only the caller knows whether
    /// the name means anything. The REPL hands the line on rather than
    /// answering for it.
    Unknown,
    /// Clear the screen.
    ///
    /// WHY this is not raw escape codes: the renderer owns an inline viewport
    /// anchored to a screen row, so a clear it did not perform itself would
    /// leave that anchor pointing at a row that no longer exists.
    Clear,
    /// Exit the REPL.
    Exit,
}

impl fmt::Display for CommandResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Output(text) => write!(f, "output({} bytes)", text.len()),
            Self::Unknown => f.write_str("unknown"),
            Self::Clear => f.write_str("clear"),
            Self::Exit => f.write_str("exit"),
        }
    }
}
