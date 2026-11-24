//! Draw a keyboard form using the native terminal host.
use super::display::CrosstermHost;
use crate::{ReplColor, ReplHost, ReplTheme};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    style::{Color, ResetColor, SetForegroundColor},
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};

pub enum FormKey {
    Text(char),
    Backspace,
    Up,
    Down,
    Next,
    Back,
    Enter,
}
pub enum FormStep<T> {
    Continue,
    Accept(T),
}
pub trait TerminalForm {
    type Output;
    fn lines(&self) -> Vec<String>;
    fn key(&mut self, key: FormKey) -> FormStep<Self::Output>;
}
struct Session(CrosstermHost);
impl Drop for Session {
    fn drop(&mut self) {
        let _ = execute!(io::stderr(), ResetColor, cursor::Show, LeaveAlternateScreen);
        let _ = self.0.leave();
    }
}
/// Show a form until the user accepts or cancels it.
///
/// # Errors
/// Returns a terminal error when input or drawing fails.
pub fn run_form<T: TerminalForm>(form: &mut T, theme: &ReplTheme) -> io::Result<Option<T::Output>> {
    let mut session = Session(CrosstermHost::new());
    session.0.enter()?;
    let mut out = io::stderr();
    execute!(out, EnterAlternateScreen, cursor::Hide)?;
    let color = match theme.prompt_foreground {
        ReplColor::Rgb(r, g, b) => Color::Rgb { r, g, b },
        _ => Color::Reset,
    };
    loop {
        let (width, height) = session.0.size()?;
        execute!(
            out,
            cursor::MoveTo(0, 0),
            terminal::Clear(terminal::ClearType::All),
            SetForegroundColor(color)
        )?;
        for (row, line) in form
            .lines()
            .into_iter()
            .take(usize::from(height))
            .enumerate()
        {
            execute!(
                out,
                cursor::MoveTo(0, u16::try_from(row).map_err(io::Error::other)?)
            )?;
            write!(
                out,
                "{}",
                line.chars().take(usize::from(width)).collect::<String>()
            )?;
        }
        out.flush()?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }
        let key = match key.code {
            KeyCode::Esc => return Ok(None),
            KeyCode::Char('c' | 'd') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(None)
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                FormKey::Text(c)
            }
            KeyCode::Backspace => FormKey::Backspace,
            KeyCode::Up => FormKey::Up,
            KeyCode::Down => FormKey::Down,
            KeyCode::Tab => FormKey::Next,
            KeyCode::BackTab => FormKey::Back,
            KeyCode::Enter => FormKey::Enter,
            _ => continue,
        };
        if let FormStep::Accept(value) = form.key(key) {
            return Ok(Some(value));
        }
    }
}
