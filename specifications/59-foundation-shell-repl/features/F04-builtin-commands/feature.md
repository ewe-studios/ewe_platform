---
workspace_name: "ewe_platform"
spec_directory: "specifications/59-foundation-shell-repl"
feature_directory: "specifications/59-foundation-shell-repl/features/F04-builtin-commands"
this_file: "specifications/59-foundation-shell-repl/features/F04-builtin-commands/feature.md"

status: planned
priority: medium
created: 2026-07-20

depends_on: ["F01-core-repl"]

tasks:
  completed: 0
  uncompleted: 0
  total: 0
  completion_percentage: 0%
---

# F04 — Builtin commands: help, clear, exit, version — caller-extensible command registration

## Overview

The REPL intercepts lines starting with `!` as commands. Four are built in;
callers can register additional ones. Commands are handled by the REPL itself
— they never reach the `messages()` iterator.

[spec](../spec.md).

---

## Part A — Command registry

```rust
// foundation_shell_repl/src/commands.rs

use std::collections::BTreeMap;
use std::fmt;

/// A command handler — receives the full command text (including `!name`)
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
        reg.register_builtin();
        reg
    }

    fn register_builtin(&mut self) {
        self.commands.insert(
            "help".into(),
            Box::new(|_| {
                "Available commands:\n  !help     Show this help\n  !clear    Clear the screen\n  !exit     Exit the REPL\n  !version  Show version".into()
            }),
        );

        self.commands.insert(
            "clear".into(),
            Box::new(|_| {
                // Special marker that tells the display to clear
                "\x1b[2J\x1b[H".into()
            }),
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

    /// Register a custom command. Name must not start with `!` (it's stripped).
    pub fn register(&mut self, name: impl Into<String>, handler: impl Fn(&str) -> String + Send + 'static) {
        let name = name.into();
        self.commands.insert(name, Box::new(handler));
    }

    /// Check if a line is a command and handle it.
    /// Returns Some(response_text) if it was a command, None otherwise.
    /// The special "__EXIT__" response signals the REPL should terminate.
    pub fn try_handle(&self, line: &str) -> Option<CommandResult> {
        let line = line.trim();
        if !line.starts_with('!') {
            return None;
        }

        let cmd_name = line
            .strip_prefix('!')
            .and_then(|s| s.split_whitespace().next())
            .unwrap_or("")
            .to_lowercase();

        match self.commands.get(&cmd_name) {
            Some(handler) => {
                let output = handler(line);
                if output == "__EXIT__" {
                    Some(CommandResult::Exit)
                } else if output.contains('\x1b') {
                    // Escape-sequence output: raw terminal command
                    Some(CommandResult::Terminal(output))
                } else {
                    Some(CommandResult::Output(output))
                }
            }
            None => Some(CommandResult::Output(format!("Unknown command: {cmd_name}. Type !help for available commands."))),
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
```

## Part B — Integration in the message loop

Commands are checked **before** yielding to the iterator. If a line starts
with `!`, it's intercepted:

```rust
// In ReplMessageIter::next():

let msg = self.repl.input.read_message(...)?;

// Check if it's a command
if let Some(result) = self.repl.commands.try_handle(&msg) {
    match result {
        CommandResult::Output(text) => {
            self.repl.display.print_response(&text);
            return Some(msg); // Still yield the original — caller may want to log
        }
        CommandResult::Terminal(escapes) => {
            // Write raw escape sequences to terminal
            let _ = write!(self.repl.stderr, "{escapes}");
            return Some(msg);
        }
        CommandResult::Exit => {
            self.repl.exit();
            return None;
        }
    }
}

Some(msg)
```

## Part C — Caller registration

```rust
impl Repl {
    /// Register a custom `!command`. The handler receives the full command
    /// text (e.g. `!greet Alice`) and returns the response string.
    pub fn register_command(
        &mut self,
        name: impl Into<String>,
        handler: impl Fn(&str) -> String + Send + 'static,
    ) {
        self.commands.register(name, handler);
    }
}
```

## Acceptance criteria

1. Typing `!help` prints the available commands list.
2. Typing `!clear` clears the terminal screen.
3. Typing `!exit` ends the REPL session.
4. Typing `!version` prints `foundation_shell_repl 0.1.0`.
5. Typing `!unknown` prints "Unknown command: unknown".
6. `repl.register_command("greet", |cmd| { format!("Hello, {}!", cmd.strip_prefix("!greet ").unwrap_or("world")) })`
   makes `!greet Alice` print `Hello, Alice!`.
7. Commands do NOT reach the `messages()` iterator consumer (except `!exit`
   which ends it).
