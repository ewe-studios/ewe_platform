use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::io;

use crate::shared::traits::{ReplDisplay, ReplInput};

pub struct NativeInput {
    buffer: String,
    had_eof: bool,
}

impl ReplInput for NativeInput {
    fn new() -> Self {
        Self {
            buffer: String::new(),
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
        mut history: Option<&mut crate::shared::history::ReplHistory>,
    ) -> io::Result<String> {
        crossterm::terminal::enable_raw_mode()?;

        self.buffer.clear();
        self.had_eof = false;

        display.draw_prompt(prompt, &self.buffer);

        let mut line_count = 0;

        loop {
            if !event::poll(std::time::Duration::from_millis(16))? {
                continue;
            }

            let Event::Key(key) = event::read()? else {
                continue;
            };

            if key.kind != KeyEventKind::Press {
                continue;
            }

            match (key.modifiers, key.code) {
                // Ctrl+C — interrupt
                (_, KeyCode::Char('c'))
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    crossterm::terminal::disable_raw_mode()?;
                    display.newline();
                    return Err(io::Error::new(
                        io::ErrorKind::Interrupted,
                        "interrupted",
                    ));
                }

                // Ctrl+D — EOF
                (_, KeyCode::Char('d'))
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.had_eof = true;
                    crossterm::terminal::disable_raw_mode()?;
                    display.newline();
                    return Ok(self.buffer.clone());
                }

                // Enter (no modifiers) — submit
                (m, KeyCode::Enter)
                    if !m.contains(KeyModifiers::SHIFT)
                        && !m.contains(KeyModifiers::CONTROL) =>
                {
                    crossterm::terminal::disable_raw_mode()?;
                    display.newline();
                    return Ok(self.buffer.clone());
                }

                // Shift+Enter or Ctrl+Enter — insert newline
                (_, KeyCode::Enter) => {
                    self.buffer.push('\n');
                    line_count += 1;
                    let cp = if line_count > 0 {
                        continuation_prompt
                    } else {
                        prompt
                    };
                    display.redraw_with_newline(cp, &self.buffer);
                }

                // Ctrl+U — clear line (must be before general Char match)
                (_, KeyCode::Char('u'))
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    self.buffer.clear();
                    display.clear_line(prompt);
                }

                // Printable characters
                (_, KeyCode::Char(c)) => {
                    if max_len.map_or(false, |n| self.buffer.len() >= n) {
                        continue;
                    }
                    self.buffer.push(c);
                    display.append_char(c, &self.buffer);
                }

                // Backspace
                (_, KeyCode::Backspace) => {
                    if !self.buffer.is_empty() {
                        self.buffer.pop();
                        display.backspace(&self.buffer);
                    }
                }

                // Left/Right arrows
                (_, KeyCode::Left) => display.move_cursor_left(),
                (_, KeyCode::Right) => display.move_cursor_right(),

                // Up/Down — history navigation
                (_, KeyCode::Up) => {
                    if let Some(hist) = history.as_mut() {
                        if let Some(entry) = hist.up() {
                            self.buffer = entry.to_string();
                            display.replace_line(&self.buffer, prompt);
                        }
                    }
                }
                (_, KeyCode::Down) => {
                    if let Some(hist) = history.as_mut() {
                        match hist.down() {
                            Some(entry) => {
                                self.buffer = entry.to_string();
                                display.replace_line(&self.buffer, prompt);
                            }
                            None => {
                                self.buffer.clear();
                                display.replace_line("", prompt);
                            }
                        }
                    }
                }

                _ => {}
            }
        }
    }
}
