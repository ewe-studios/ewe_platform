//! Main REPL loop and builder.
//!
//! The [`Repl`] struct owns input, display, commands, and history.
//! Call [`Repl::messages()`] to get an iterator that blocks on user input,
//! and [`Repl::reply()`] to print responses.

use std::cell::{Cell, RefCell};
use std::io::Write;
use crate::shared::config::ReplConfig;
use crate::shared::traits::{ReplInput, ReplDisplay};
use crate::shared::commands::{CommandRegistry, CommandResult};
use crate::shared::history::ReplHistory;

/// Main REPL handle.
///
/// Uses interior mutability so that [`Repl::reply()`] and
/// [`Repl::register_command()`] can be called from inside a `for input in
/// repl.messages()` loop without conflicting with the mutable borrow.
pub struct Repl {
    config: ReplConfig,
    input: RefCell<Box<dyn ReplInput>>,
    display: RefCell<Box<dyn ReplDisplay>>,
    commands: RefCell<CommandRegistry>,
    history: RefCell<ReplHistory>,
    exited: Cell<bool>,
}

impl Repl {
    /// Create a new REPL with default config, using the platform-native backend.
    pub fn new() -> Self {
        Self::with_config(ReplConfig::default())
    }

    /// Create a new REPL with custom config.
    pub fn with_config(config: ReplConfig) -> Self {
        let (input, display) = Self::make_backend();
        Self {
            config,
            input: RefCell::new(Box::new(input)),
            display: RefCell::new(Box::new(display)),
            commands: RefCell::new(CommandRegistry::new()),
            history: RefCell::new(ReplHistory::new(1000)),
            exited: Cell::new(false),
        }
    }

    #[cfg(all(feature = "native", not(feature = "wasm")))]
    fn make_backend() -> (crate::native::input::NativeInput, crate::native::display::NativeDisplay) {
        (
            crate::native::input::NativeInput::new(),
            crate::native::display::NativeDisplay::new(),
        )
    }

    #[cfg(feature = "wasm")]
    fn make_backend() -> (crate::wasm::input::WasmInput, crate::wasm::display::WasmDisplay) {
        (
            crate::wasm::input::WasmInput::new(),
            crate::wasm::display::WasmDisplay::new(),
        )
    }

    /// Start building a custom REPL.
    pub fn builder() -> ReplBuilder {
        ReplBuilder::default()
    }

    /// Returns an iterator over user messages.
    pub fn messages(&self) -> ReplMessageIter<'_> {
        if let Some(ref banner) = self.config.banner {
            self.display.borrow_mut().print_banner(banner);
        }
        ReplMessageIter { repl: self }
    }

    /// Display a response from the caller.
    pub fn reply(&self, response: impl AsRef<str>) {
        self.display.borrow_mut().print_response(response.as_ref());
    }

    /// Register a custom `/command`.
    pub fn register_command(
        &self,
        name: impl Into<String>,
        handler: impl Fn(&str) -> String + Send + 'static,
    ) {
        self.commands.borrow_mut().register(name, handler);
    }

    /// Signal that the REPL should exit after the current message.
    pub fn exit(&self) {
        self.exited.set(true);
    }
}

impl Default for Repl {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Repl {
    fn drop(&mut self) {
        if let Some(ref msg) = self.config.goodbye {
            eprintln!("{msg}");
        }
    }
}

/// Iterator that yields complete user messages.
pub struct ReplMessageIter<'a> {
    repl: &'a Repl,
}

impl Iterator for ReplMessageIter<'_> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        if self.repl.exited.get() {
            return None;
        }

        let mut input = self.repl.input.borrow_mut();
        let mut display = self.repl.display.borrow_mut();
        let mut history = self.repl.history.borrow_mut();

        let result = input.read_message(
            &self.repl.config.prompt,
            &self.repl.config.continuation_prompt,
            &mut **display,
            self.repl.config.max_input_length,
            Some(&mut history),
        );

        // Drop borrows before processing command results
        drop(history);
        drop(display);
        drop(input);

        match result {
            Ok(msg) if msg.is_empty() && self.repl.input.borrow().eof() => None,
            Ok(msg) => {
                // Check for commands before yielding
                if let Some(cmd_result) = self.repl.commands.borrow().try_handle(&msg) {
                    match cmd_result {
                        CommandResult::Output(text) => {
                            self.repl.display.borrow_mut().print_response(&text);
                            if !text.starts_with("Unknown command") {
                                return None;
                            }
                            return Some(msg);
                        }
                        CommandResult::Terminal(escapes) => {
                            print!("{escapes}");
                            let _ = std::io::stdout().flush();
                            return None;
                        }
                        CommandResult::Exit => {
                            self.repl.exit();
                            return None;
                        }
                    }
                }
                // Push to history
                self.repl.history.borrow_mut().push(msg.clone());
                Some(msg)
            }
            Err(_) => None,
        }
    }
}

/// Builder for customizing a REPL.
#[derive(Default)]
pub struct ReplBuilder {
    config: ReplConfig,
}

impl ReplBuilder {
    pub fn prompt(mut self, s: impl Into<String>) -> Self {
        self.config.prompt = s.into();
        self
    }

    pub fn continuation_prompt(mut self, s: impl Into<String>) -> Self {
        self.config.continuation_prompt = s.into();
        self
    }

    pub fn banner(mut self, s: impl Into<String>) -> Self {
        self.config.banner = Some(s.into());
        self
    }

    pub fn goodbye(mut self, s: impl Into<String>) -> Self {
        self.config.goodbye = Some(s.into());
        self
    }

    pub fn max_input_length(mut self, n: usize) -> Self {
        self.config.max_input_length = Some(n);
        self
    }

    #[cfg(feature = "colors")]
    pub fn colors(mut self, colors: crate::shared::config::ReplColors) -> Self {
        self.config.colors = colors;
        self
    }

    pub fn build(self) -> Repl {
        Repl::with_config(self.config)
    }
}
