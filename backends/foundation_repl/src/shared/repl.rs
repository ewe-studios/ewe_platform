//! Main REPL loop and builder.
//!
//! The [`Repl`] struct owns input, display, commands, and history.
//! Call [`Repl::messages()`] to get an iterator that blocks on user input,
//! [`Repl::reply()`] to print responses, and [`Repl::animation()`] to show that
//! work is happening while a response is being produced.

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::shared::activity::Activity;
use crate::shared::commands::{CommandRegistry, CommandResult};
use crate::shared::config::ReplConfig;
use crate::shared::history::ReplHistory;
use crate::shared::theme::ReplTheme;
use crate::shared::traits::{BoxedDisplay, ReplInput, SharedDisplay};

/// Main REPL handle.
///
/// Uses interior mutability so that [`Repl::reply()`] and
/// [`Repl::register_command()`] can be called from inside a `for input in
/// repl.messages()` loop without conflicting with the mutable borrow.
///
/// The display is behind a mutex rather than a `RefCell` because the activity
/// indicator animates on its own thread while the caller's thread is busy — see
/// [`Repl::animation()`].
pub struct Repl {
    config: ReplConfig,
    input: RefCell<Box<dyn ReplInput>>,
    display: SharedDisplay,
    commands: RefCell<CommandRegistry>,
    history: RefCell<ReplHistory>,
    exited: Cell<bool>,
}

impl Repl {
    /// Create a new REPL with default config, using the platform-native backend.
    #[must_use]
    pub fn new() -> Self {
        Self::with_config(ReplConfig::default())
    }

    /// Create a new REPL with custom config.
    #[must_use]
    pub fn with_config(config: ReplConfig) -> Self {
        let (input, display) = Self::make_backend(config.theme.clone());
        Self {
            config,
            input: RefCell::new(Box::new(input)),
            display: Arc::new(Mutex::new(Box::new(display))),
            commands: RefCell::new(CommandRegistry::new()),
            history: RefCell::new(ReplHistory::new(1000)),
            exited: Cell::new(false),
        }
    }

    #[cfg(all(feature = "native", not(feature = "wasm")))]
    fn make_backend(
        theme: ReplTheme,
    ) -> (
        crate::native::input::NativeInput,
        crate::native::display::NativeDisplay,
    ) {
        (
            crate::native::input::NativeInput::new(),
            crate::native::display::native_display(theme),
        )
    }

    #[cfg(feature = "wasm")]
    fn make_backend(
        theme: ReplTheme,
    ) -> (crate::wasm::input::WasmInput, crate::wasm::display::WasmDisplay) {
        (
            crate::wasm::input::WasmInput::new(),
            crate::wasm::display::WasmDisplay::new(theme),
        )
    }

    /// Start building a custom REPL.
    #[must_use]
    pub fn builder() -> ReplBuilder {
        ReplBuilder::default()
    }

    /// Returns an iterator over user messages.
    pub fn messages(&self) -> ReplMessageIter<'_> {
        if let Some(ref banner) = self.config.banner {
            self.display().print_banner(banner);
        }
        ReplMessageIter { repl: self }
    }

    /// Display a response from the caller.
    pub fn reply(&self, response: impl AsRef<str>) {
        self.display().print_response(response.as_ref());
    }

    /// Display an error from the caller.
    pub fn report_error(&self, message: impl AsRef<str>) {
        self.display().print_error(message.as_ref());
    }

    /// Show an animated indicator until the returned guard is dropped.
    ///
    /// WHY: work that takes seconds leaves the terminal looking hung. The
    /// indicator says the session is alive, and doubles as the sink for results
    /// that arrive a piece at a time.
    ///
    /// WHAT: replaces the input box with an animated status box carrying
    /// `label`. Text pushed into the guard streams into the output region above
    /// it as it arrives; reporting progress turns the spinner into a bar.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use foundation_repl::Repl;
    ///
    /// let repl = Repl::new();
    /// for input in repl.messages() {
    ///     let stream = repl.animation("thinking");
    ///     for chunk in answer(&input) {
    ///         stream.push(chunk);
    ///     }
    ///     stream.finish();
    /// }
    /// # fn answer(input: &str) -> Vec<String> { vec![input.to_string()] }
    /// ```
    #[must_use]
    pub fn animation(&self, label: impl Into<String>) -> Activity {
        Activity::start(
            Arc::clone(&self.display),
            self.config.theme.activity.clone(),
            label.into(),
        )
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

    /// The configuration this REPL was built with.
    #[must_use]
    pub fn config(&self) -> &ReplConfig {
        &self.config
    }

    /// Lock the display, recovering it even if a previous holder panicked.
    fn display(&self) -> MutexGuard<'_, BoxedDisplay> {
        self.display
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for Repl {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for Repl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Repl")
            .field("config", &self.config)
            .field("exited", &self.exited.get())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Display for Repl {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Repl(prompt {:?})", self.config.prompt)
    }
}

impl Drop for Repl {
    fn drop(&mut self) {
        // The terminal must leave raw mode before anything is printed
        // normally, or the goodbye line would be laid out by a terminal still
        // in raw mode and start halfway across the screen.
        self.display().shutdown();

        if let Some(ref msg) = self.config.goodbye {
            println!("{msg}");
        }
    }
}

/// Iterator that yields complete user messages.
pub struct ReplMessageIter<'a> {
    repl: &'a Repl,
}

impl core::fmt::Debug for ReplMessageIter<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReplMessageIter").finish_non_exhaustive()
    }
}

impl Iterator for ReplMessageIter<'_> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        // A `/command` is handled here and does not end the session, so the
        // read is retried until there is something for the caller to act on.
        loop {
            if self.repl.exited.get() {
                return None;
            }

            let result = {
                let mut input = self.repl.input.borrow_mut();
                let mut display = self.repl.display();
                let mut history = self.repl.history.borrow_mut();

                input.read_message(
                    &self.repl.config.prompt,
                    &self.repl.config.continuation_prompt,
                    &mut **display,
                    self.repl.config.max_input_length,
                    Some(&mut history),
                )
            };

            let message = match result {
                Ok(message) => message,
                // Ctrl+C abandons the line rather than the session; every other
                // failure means the terminal is gone.
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    tracing::debug!(%error, "repl: input ended");
                    return None;
                }
            };

            if message.is_empty() && self.repl.input.borrow().eof() {
                return None;
            }

            match self.repl.commands.borrow().try_handle(&message) {
                Some(CommandResult::Output(text)) => {
                    self.repl.display().print_response(&text);
                }
                Some(CommandResult::Clear) => self.repl.display().clear_screen(),
                Some(CommandResult::Exit) => {
                    self.repl.exit();
                    return None;
                }
                // `Unknown` joins `None` deliberately: a `/name` nobody
                // registered is handed to the caller rather than reported here.
                // An application that wants its own slash syntax handles it,
                // and one that does not can say so itself — reporting it here
                // as well would print an error for a line the application then
                // went on to answer.
                None | Some(CommandResult::Unknown) => {
                    self.repl.history.borrow_mut().push(message.clone());
                    return Some(message);
                }
            }
        }
    }
}

/// Builder for customizing a REPL.
#[derive(Debug, Default)]
pub struct ReplBuilder {
    config: ReplConfig,
}

impl ReplBuilder {
    /// Set the primary prompt.
    #[must_use]
    pub fn prompt(mut self, s: impl Into<String>) -> Self {
        self.config.prompt = s.into();
        self
    }

    /// Set the prompt shown on continuation lines of multiline input.
    #[must_use]
    pub fn continuation_prompt(mut self, s: impl Into<String>) -> Self {
        self.config.continuation_prompt = s.into();
        self
    }

    /// Set the banner printed when the session starts.
    #[must_use]
    pub fn banner(mut self, s: impl Into<String>) -> Self {
        self.config.banner = Some(s.into());
        self
    }

    /// Set the message printed when the session ends.
    #[must_use]
    pub fn goodbye(mut self, s: impl Into<String>) -> Self {
        self.config.goodbye = Some(s.into());
        self
    }

    /// Cap how much text one message may contain, in bytes.
    #[must_use]
    pub fn max_input_length(mut self, n: usize) -> Self {
        self.config.max_input_length = Some(n);
        self
    }

    /// Replace the whole visual theme.
    #[must_use]
    pub fn theme(mut self, theme: ReplTheme) -> Self {
        self.config.theme = theme;
        self
    }

    /// Build the REPL.
    #[must_use]
    pub fn build(self) -> Repl {
        Repl::with_config(self.config)
    }
}
