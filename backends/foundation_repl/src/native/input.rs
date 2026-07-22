//! Native keystroke handling: turns key events into edits on a buffer.
//!
//! WHY: the caret has to live with the text it is editing. Keeping the buffer
//! here and the caret in the renderer is what let the two drift apart, so
//! arrow keys moved a cursor that typing then ignored.
//!
//! WHAT: a line editor over a `String` — insert, delete, word delete, caret
//! movement, history recall and multiline entry via Shift/Ctrl+Enter.
//!
//! HOW: every event that changes the buffer or the caret redraws the whole
//! input area through [`ReplDisplay::render_input`], so what is on screen is
//! always a function of the current buffer.

use std::io;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::shared::history::ReplHistory;
use crate::shared::layout::InputView;
use crate::shared::traits::{ReplDisplay, ReplInput};

/// Spaces inserted when the Tab key is pressed.
///
/// A literal tab would be expanded by the terminal to its own tab stops, which
/// does not agree with the box's geometry, so tabs never enter the buffer.
const TAB_SPACES: usize = 4;

/// Reads keystrokes into a buffer, redrawing the input box as it goes.
#[derive(Debug)]
pub struct NativeInput {
    buffer: String,
    cursor: usize,
    had_eof: bool,
}

impl core::fmt::Display for NativeInput {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "NativeInput({} bytes, caret {})",
            self.buffer.len(),
            self.cursor
        )
    }
}

impl ReplInput for NativeInput {
    fn new() -> Self {
        Self {
            buffer: String::new(),
            cursor: 0,
            had_eof: false,
        }
    }

    fn eof(&self) -> bool {
        self.had_eof
    }

    fn read_message(
        &mut self,
        prompt: &str,
        continuation_prompt: &str,
        display: &mut dyn ReplDisplay,
        max_len: Option<usize>,
        mut history: Option<&mut ReplHistory>,
    ) -> io::Result<String> {
        self.buffer.clear();
        self.cursor = 0;
        self.had_eof = false;

        display.render_input(&self.view(prompt, continuation_prompt));

        loop {
            let outcome = match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    self.on_key(key, max_len, history.as_deref_mut())
                }
                // A resize changes how the text wraps, so the box is remeasured.
                Event::Resize(_, _) => Step::Redraw,
                _ => Step::Ignore,
            };

            match outcome {
                Step::Ignore => {}
                Step::Redraw => display.render_input(&self.view(prompt, continuation_prompt)),
                Step::Submit => {
                    let view = self.view(prompt, continuation_prompt);
                    display.finish_input(&view);
                    return Ok(self.buffer.clone());
                }
                Step::Interrupt => {
                    display.finish_input(&self.view(prompt, continuation_prompt));
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "interrupted"));
                }
            }
        }
    }
}

impl NativeInput {
    /// The current buffer and caret, ready to be drawn.
    fn view<'a>(&'a self, prompt: &'a str, continuation: &'a str) -> InputView<'a> {
        InputView {
            prompt,
            continuation,
            buffer: &self.buffer,
            cursor: self.cursor,
        }
    }

    /// Apply one key press, reporting what the caller should do next.
    fn on_key(
        &mut self,
        key: KeyEvent,
        max_len: Option<usize>,
        history: Option<&mut ReplHistory>,
    ) -> Step {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Char('c') if control => Step::Interrupt,

            // Ctrl+D ends the session only on an empty line; with text in the
            // buffer it is the readline "delete forward" it is everywhere else.
            KeyCode::Char('d') if control => {
                if self.buffer.is_empty() {
                    self.had_eof = true;
                    Step::Submit
                } else {
                    self.delete_forward()
                }
            }

            KeyCode::Char('u') if control => {
                self.buffer.clear();
                self.cursor = 0;
                Step::Redraw
            }
            KeyCode::Char('k') if control => {
                self.buffer.truncate(self.cursor);
                Step::Redraw
            }
            KeyCode::Char('w') if control => self.delete_previous_word(),
            KeyCode::Char('a') if control => self.move_to_start(),
            KeyCode::Char('e') if control => self.move_to_end(),

            // Plain Enter submits; any modifier makes it a newline, which is
            // how a multiline message is entered.
            KeyCode::Enter if key.modifiers.is_empty() => Step::Submit,
            KeyCode::Enter => self.insert_str("\n", max_len),

            KeyCode::Tab => self.insert_str(&" ".repeat(TAB_SPACES), max_len),
            KeyCode::Char(character) => {
                let mut encoded = [0_u8; 4];
                self.insert_str(character.encode_utf8(&mut encoded), max_len)
            }

            KeyCode::Backspace => self.delete_backward(),
            KeyCode::Delete => self.delete_forward(),
            KeyCode::Left => self.move_left(),
            KeyCode::Right => self.move_right(),
            KeyCode::Home => self.move_to_start(),
            KeyCode::End => self.move_to_end(),
            KeyCode::Up => self.recall(history, Direction::Older),
            KeyCode::Down => self.recall(history, Direction::Newer),

            _ => Step::Ignore,
        }
    }

    /// Insert text at the caret, respecting the configured length ceiling.
    fn insert_str(&mut self, text: &str, max_len: Option<usize>) -> Step {
        if max_len.is_some_and(|limit| self.buffer.len() + text.len() > limit) {
            return Step::Ignore;
        }
        self.buffer.insert_str(self.cursor, text);
        self.cursor += text.len();
        Step::Redraw
    }

    fn delete_backward(&mut self) -> Step {
        let Some(previous) = self.previous_boundary() else {
            return Step::Ignore;
        };
        self.buffer.replace_range(previous..self.cursor, "");
        self.cursor = previous;
        Step::Redraw
    }

    fn delete_forward(&mut self) -> Step {
        let Some(next) = self.next_boundary() else {
            return Step::Ignore;
        };
        self.buffer.replace_range(self.cursor..next, "");
        Step::Redraw
    }

    /// Delete back to the start of the word before the caret, taking any
    /// whitespace between the caret and that word with it.
    fn delete_previous_word(&mut self) -> Step {
        if self.cursor == 0 {
            return Step::Ignore;
        }

        let head = &self.buffer[..self.cursor];
        let trimmed = head.trim_end_matches(char::is_whitespace);
        let start = trimmed
            .rfind(char::is_whitespace)
            .map_or(0, |index| index + 1);

        self.buffer.replace_range(start..self.cursor, "");
        self.cursor = start;
        Step::Redraw
    }

    fn move_left(&mut self) -> Step {
        match self.previous_boundary() {
            Some(previous) => {
                self.cursor = previous;
                Step::Redraw
            }
            None => Step::Ignore,
        }
    }

    fn move_right(&mut self) -> Step {
        match self.next_boundary() {
            Some(next) => {
                self.cursor = next;
                Step::Redraw
            }
            None => Step::Ignore,
        }
    }

    fn move_to_start(&mut self) -> Step {
        if self.cursor == 0 {
            return Step::Ignore;
        }
        self.cursor = 0;
        Step::Redraw
    }

    fn move_to_end(&mut self) -> Step {
        if self.cursor == self.buffer.len() {
            return Step::Ignore;
        }
        self.cursor = self.buffer.len();
        Step::Redraw
    }

    /// Replace the buffer with a history entry, or clear it when walking off
    /// the newest end of the history.
    fn recall(&mut self, history: Option<&mut ReplHistory>, direction: Direction) -> Step {
        let Some(history) = history else {
            return Step::Ignore;
        };

        let entry = match direction {
            Direction::Older => history.up(),
            Direction::Newer => history.down(),
        };

        self.buffer = entry.unwrap_or("").to_string();
        self.cursor = self.buffer.len();
        Step::Redraw
    }

    /// Byte offset of the character before the caret.
    fn previous_boundary(&self) -> Option<usize> {
        self.buffer[..self.cursor]
            .char_indices()
            .next_back()
            .map(|(index, _)| index)
    }

    /// Byte offset just past the character after the caret.
    fn next_boundary(&self) -> Option<usize> {
        self.buffer[self.cursor..]
            .chars()
            .next()
            .map(|character| self.cursor + character.len_utf8())
    }
}

/// Which way through the history a recall moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    /// Towards older entries.
    Older,
    /// Towards newer entries.
    Newer,
}

/// What the read loop should do after handling an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    /// Nothing changed; do not repaint.
    Ignore,
    /// The buffer or caret moved; repaint the input box.
    Redraw,
    /// The message is complete.
    Submit,
    /// The user interrupted the read.
    Interrupt,
}
