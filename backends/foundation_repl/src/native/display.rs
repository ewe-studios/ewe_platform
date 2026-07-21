use crossterm::{
    cursor::MoveToColumn,
    style::Print,
    terminal::{Clear, ClearType},
    QueueableCommand,
};
#[cfg(feature = "colors")]
use crossterm::style::{ResetColor, SetForegroundColor};
use std::io::{self, Write};

use crate::shared::traits::ReplDisplay;

pub struct NativeDisplay {
    cursor_col: u16,
    #[cfg(feature = "colors")]
    colors: crate::shared::config::ReplColors,
}

impl ReplDisplay for NativeDisplay {
    fn new() -> Self {
        Self {
            cursor_col: 0,
            #[cfg(feature = "colors")]
            colors: crate::shared::config::ReplColors::default(),
        }
    }

    fn print_banner(&mut self, banner: &str) {
        let _ = writeln!(io::stderr(), "\n{banner}\n");
    }

    fn draw_prompt(&mut self, prompt: &str, _buffer: &str) {
        let mut w = io::stderr().lock();
        #[cfg(feature = "colors")]
        let _ = w
            .queue(SetForegroundColor(self.colors.prompt_color))
            .and_then(|w| w.queue(Print(prompt)))
            .and_then(|w| w.queue(ResetColor));
        #[cfg(not(feature = "colors"))]
        let _ = write!(w, "{prompt}");
        let _ = w.flush();
        self.cursor_col = prompt.len() as u16;
    }

    fn redraw_with_newline(&mut self, prompt: &str, _buffer: &str) {
        let mut w = io::stderr().lock();
        let _ = writeln!(w);
        #[cfg(feature = "colors")]
        let _ = w
            .queue(SetForegroundColor(self.colors.continuation_prompt_color))
            .and_then(|w| w.queue(Print(prompt)))
            .and_then(|w| w.queue(ResetColor));
        #[cfg(not(feature = "colors"))]
        let _ = write!(w, "{prompt}");
        let _ = w.flush();
        self.cursor_col = prompt.len() as u16;
    }

    fn append_char(&mut self, _c: char, buffer: &str) {
        let mut w = io::stderr().lock();
        let _ = w
            .queue(MoveToColumn(0))
            .and_then(|w| w.queue(Clear(ClearType::UntilNewLine)))
            .and_then(|w| w.queue(Print(buffer)));
        let _ = w.flush();
    }

    fn backspace(&mut self, buffer: &str) {
        let mut w = io::stderr().lock();
        let _ = w
            .queue(MoveToColumn(0))
            .and_then(|w| w.queue(Clear(ClearType::UntilNewLine)))
            .and_then(|w| w.queue(Print(buffer)));
        let _ = w.flush();
    }

    fn clear_line(&mut self, prompt: &str) {
        let mut w = io::stderr().lock();
        let _ = w
            .queue(MoveToColumn(0))
            .and_then(|w| w.queue(Clear(ClearType::UntilNewLine)));
        #[cfg(feature = "colors")]
        let _ = w
            .queue(SetForegroundColor(self.colors.prompt_color))
            .and_then(|w| w.queue(Print(prompt)))
            .and_then(|w| w.queue(ResetColor));
        #[cfg(not(feature = "colors"))]
        let _ = write!(w, "{prompt}");
        let _ = w.flush();
        self.cursor_col = prompt.len() as u16;
    }

    fn newline(&mut self) {
        let _ = writeln!(io::stderr());
    }

    fn print_response(&mut self, response: &str) {
        let _ = writeln!(io::stderr());
        for line in response.lines() {
            let _ = writeln!(io::stderr(), "  {line}");
        }
        let _ = writeln!(io::stderr());
    }

    fn print_error(&mut self, msg: &str) {
        let mut w = io::stderr().lock();
        #[cfg(feature = "colors")]
        let _ = w
            .queue(SetForegroundColor(self.colors.error_color))
            .and_then(|w| w.queue(Print(msg)))
            .and_then(|w| w.queue(ResetColor));
        #[cfg(not(feature = "colors"))]
        let _ = write!(w, "{msg}");
        let _ = w.flush();
        let _ = writeln!(w);
    }

    fn move_cursor_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
            let mut w = io::stderr().lock();
            let _ = w.queue(MoveToColumn(self.cursor_col));
            let _ = w.flush();
        }
    }

    fn move_cursor_right(&mut self) {
        self.cursor_col += 1;
        let mut w = io::stderr().lock();
        let _ = w.queue(MoveToColumn(self.cursor_col));
        let _ = w.flush();
    }

    fn replace_line(&mut self, content: &str, prompt: &str) {
        let mut w = io::stderr().lock();
        let _ = w
            .queue(MoveToColumn(0))
            .and_then(|w| w.queue(Clear(ClearType::UntilNewLine)));
        #[cfg(feature = "colors")]
        let _ = w
            .queue(SetForegroundColor(self.colors.prompt_color))
            .and_then(|w| w.queue(Print(prompt)))
            .and_then(|w| w.queue(ResetColor))
            .and_then(|w| w.queue(Print(content)));
        #[cfg(not(feature = "colors"))]
        let _ = write!(w, "{prompt}{content}");
        let _ = w.flush();
        self.cursor_col = (prompt.len() + content.len()) as u16;
    }
}
