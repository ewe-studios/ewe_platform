use std::collections::BTreeMap;

/// A command handler — receives the full command text (e.g. `/greet Alice`)
/// and returns the text to display.
pub type CommandHandler = Box<dyn Fn(&str) -> String + Send + 'static>;

/// Registered commands in the REPL.
pub struct CommandRegistry {
    commands: BTreeMap<String, CommandHandler>,
}

impl CommandRegistry {
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

        self.commands.insert(
            "clear".into(),
            Box::new(|_| "\x1b[2J\x1b[H".into()),
        );

        self.commands.insert(
            "exit".into(),
            Box::new(|_| "__EXIT__".into()),
        );

        self.commands.insert(
            "version".into(),
            Box::new(|_| format!("foundation_shell_repl {}", env!("CARGO_PKG_VERSION"))),
        );
    }

    /// Register a custom command. Name must not start with `/` (it's stripped).
    pub fn register(
        &mut self,
        name: impl Into<String>,
        handler: impl Fn(&str) -> String + Send + 'static,
    ) {
        let name = name.into();
        self.commands.insert(name, Box::new(handler));
    }

    /// Check if a line is a command and handle it.
    /// Returns Some(CommandResult) if it was a command, None otherwise.
    pub fn try_handle(&self, line: &str) -> Option<CommandResult> {
        let line = line.trim();
        if !line.starts_with('/') {
            return None;
        }

        let cmd_name = line
            .strip_prefix('/')
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("")
            .to_lowercase();

        match self.commands.get(&cmd_name) {
            Some(handler) => {
                let output = handler(line);
                if output == "__EXIT__" {
                    Some(CommandResult::Exit)
                } else if output.contains('\x1b') {
                    Some(CommandResult::Terminal(output))
                } else {
                    Some(CommandResult::Output(output))
                }
            }
            None => Some(CommandResult::Output(format!(
                "Unknown command: {cmd_name}. Type /help for available commands."
            ))),
        }
    }
}

/// Result of attempting to handle a command.
pub enum CommandResult {
    /// Print this text as a normal response.
    Output(String),
    /// Send raw escape sequences to the terminal (e.g. clear screen).
    Terminal(String),
    /// Exit the REPL.
    Exit,
}
