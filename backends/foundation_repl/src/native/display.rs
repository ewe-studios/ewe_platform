//! Native terminal rendering: a bordered, padded input box drawn inline below
//! the session's output.
//!
//! WHY: painting the prompt and the typed text as one flat line makes the two
//! impossible to tell apart, and any incremental repaint scheme eventually
//! disagrees with the buffer — that is how the prompt used to get erased on the
//! first keystroke. Drawing the whole area from the buffer every time removes
//! the class of bug entirely.
//!
//! WHAT: an inline viewport pinned below normal terminal output. The live input
//! sits inside a bordered box with a lighter background; banners, submitted
//! input and responses are pushed above it, responses in a darker shade painted
//! the full width of the terminal.
//!
//! HOW: ratatui's [`Viewport::Inline`] owns the box's rows and diffs each
//! frame, while `insert_before` emits the lines that scroll up into the
//! terminal's own scrollback. Everything is written to stderr, leaving stdout
//! free for a caller's real output. The viewport has a fixed height, so growing
//! the box for wrapped or multiline input means rebuilding the terminal at the
//! new height, anchored to the row the old box started on.

use std::io::{self, Stderr, Write};

use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Widget};
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::shared::layout::{layout_input, visible_window, InputView};
use crate::shared::theme::{BorderKind, ReplColor, ReplPadding, ReplTheme};
use crate::shared::traits::ReplDisplay;

/// Rows the box falls back to before anything has been drawn.
const INITIAL_VIEWPORT_HEIGHT: u16 = 1;

/// Columns assumed when the terminal will not report its size.
const FALLBACK_TERMINAL_WIDTH: u16 = 80;

/// Rows assumed when the terminal will not report its size.
const FALLBACK_TERMINAL_HEIGHT: u16 = 24;

/// Spaces a tab is expanded to before rendering.
///
/// Terminals expand a literal tab to their own tab stops, which would slide
/// text out from under the box border, so tabs never reach the screen.
const TAB_WIDTH: usize = 4;

/// Draws the REPL into a real terminal.
pub struct NativeDisplay {
    theme: ReplTheme,
    terminal: Option<Terminal<CrosstermBackend<Stderr>>>,
    viewport_height: u16,
    raw_mode: bool,
}

impl core::fmt::Debug for NativeDisplay {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NativeDisplay")
            .field("theme", &self.theme)
            .field("attached", &self.terminal.is_some())
            .field("viewport_height", &self.viewport_height)
            .field("raw_mode", &self.raw_mode)
            .finish()
    }
}

impl core::fmt::Display for NativeDisplay {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "NativeDisplay({} rows, raw mode {})",
            self.viewport_height, self.raw_mode
        )
    }
}

impl ReplDisplay for NativeDisplay {
    fn new(theme: ReplTheme) -> Self {
        Self {
            theme,
            terminal: None,
            viewport_height: INITIAL_VIEWPORT_HEIGHT,
            raw_mode: false,
        }
    }

    fn print_banner(&mut self, banner: &str) {
        let style = style_for(self.theme.banner_foreground, self.theme.output_background);
        let mut lines = vec![Line::default()];
        lines.extend(
            banner
                .lines()
                .map(|line| self.indented_line(line, self.theme.banner_foreground)),
        );
        lines.push(Line::default());
        self.insert_above(lines, style);
    }

    fn render_input(&mut self, view: &InputView<'_>) {
        let (width, height) = terminal_size();
        let frame = InputFrame::measure(&self.theme, view, width, height);

        if let Err(error) = self.attach(frame.box_height) {
            tracing::error!(%error, "repl: could not prepare the input viewport");
            return;
        }

        let theme = self.theme.clone();
        let Some(terminal) = self.terminal.as_mut() else {
            return;
        };

        let outcome = terminal.draw(|target| {
            let area = target.area();
            frame.render(&theme, target.buffer_mut(), area);
            let (column, row) = frame.caret(&theme, area);
            target.set_cursor_position(Position::new(column, row));
        });

        if let Err(error) = outcome {
            tracing::error!(%error, "repl: could not draw the input box");
        }
    }

    fn finish_input(&mut self, view: &InputView<'_>) {
        let (width, height) = terminal_size();
        let frame = InputFrame::measure(&self.theme, view, width, height);

        if let Err(error) = self.attach(self.viewport_height) {
            tracing::error!(%error, "repl: could not prepare the input viewport");
            return;
        }

        let theme = self.theme.clone();
        let Some(terminal) = self.terminal.as_mut() else {
            return;
        };

        // Re-emitting the box above the viewport moves it into the terminal's
        // scrollback, so the submitted message stays on screen with its border,
        // padding and shading intact while the response is printed beneath it.
        let outcome = terminal.insert_before(frame.box_height, |buffer| {
            let area = buffer.area;
            frame.render(&theme, buffer, area);
        });

        if let Err(error) = outcome {
            tracing::error!(%error, "repl: could not commit the submitted input");
        }
    }

    fn print_response(&mut self, response: &str) {
        let style = style_for(self.theme.output_foreground, self.theme.output_background);
        let lines = self.body_lines(response, self.theme.output_foreground);
        self.insert_above(lines, style);
    }

    fn print_error(&mut self, msg: &str) {
        let style = style_for(self.theme.error_foreground, self.theme.output_background);
        let lines = self.body_lines(msg, self.theme.error_foreground);
        self.insert_above(lines, style);
    }

    fn clear_screen(&mut self) {
        // The viewport is anchored to a row that is about to stop existing, so
        // the terminal is rebuilt from the top of the cleared screen.
        self.terminal = None;

        let mut stderr = io::stderr();
        let cleared = crossterm::execute!(
            stderr,
            crossterm::terminal::Clear(crossterm::terminal::ClearType::All),
            crossterm::cursor::MoveTo(0, 0),
        )
        .and_then(|()| stderr.flush());

        if let Err(error) = cleared {
            tracing::error!(%error, "repl: could not clear the screen");
        }
    }

    fn shutdown(&mut self) {
        if let Some(mut terminal) = self.terminal.take() {
            // Wipe the live input box, then park the caret on the row it
            // started at so whatever prints next lands there.
            let anchor = terminal.get_frame().area().y;
            if let Err(error) = terminal
                .clear()
                .and_then(|()| terminal.set_cursor_position(Position::new(0, anchor)))
                .and_then(|()| terminal.show_cursor())
            {
                tracing::warn!(%error, "repl: could not restore the cursor");
            }
        }

        if self.raw_mode {
            if let Err(error) = crossterm::terminal::disable_raw_mode() {
                tracing::error!(%error, "repl: could not leave raw mode");
            }
            self.raw_mode = false;
        }

        if let Err(error) = io::stderr().flush() {
            tracing::warn!(%error, "repl: could not flush the terminal");
        }
    }
}

impl NativeDisplay {
    /// Make sure a terminal exists whose inline viewport is `height` rows tall.
    ///
    /// The viewport height is fixed when the terminal is built, so a box that
    /// has grown or shrunk needs a new one. The old box is cleared and the
    /// caret returned to the row it started on first, so the replacement
    /// anchors in the same place instead of walking down the screen.
    fn attach(&mut self, height: u16) -> io::Result<()> {
        if !self.raw_mode {
            crossterm::terminal::enable_raw_mode()?;
            self.raw_mode = true;
        }

        if let Some(terminal) = self.terminal.as_mut() {
            if self.viewport_height == height {
                return Ok(());
            }
            let anchor = terminal.get_frame().area().y;
            terminal.clear()?;
            terminal.set_cursor_position(Position::new(0, anchor))?;
        }

        self.terminal = None;
        self.viewport_height = height;
        self.terminal = Some(Terminal::with_options(
            CrosstermBackend::new(io::stderr()),
            TerminalOptions {
                viewport: Viewport::Inline(height),
            },
        )?);

        Ok(())
    }

    /// Push `lines` into the scrollback above the input box.
    fn insert_above(&mut self, lines: Vec<Line<'static>>, style: Style) {
        let Ok(height) = u16::try_from(lines.len()) else {
            tracing::warn!(rows = lines.len(), "repl: refusing to print an oversized block");
            return;
        };
        if height == 0 {
            return;
        }

        if let Err(error) = self.attach(self.viewport_height) {
            tracing::error!(%error, "repl: could not prepare the input viewport");
            return;
        }

        let Some(terminal) = self.terminal.as_mut() else {
            return;
        };

        let outcome = terminal.insert_before(height, move |buffer| {
            let area = buffer.area;
            Paragraph::new(Text::from(lines))
                .style(style)
                .render(area, buffer);
        });

        if let Err(error) = outcome {
            tracing::error!(%error, "repl: could not print above the input box");
        }
    }

    /// Wrap `text` to the terminal width and surround it with the configured
    /// blank shaded rows.
    fn body_lines(&self, text: &str, foreground: ReplColor) -> Vec<Line<'static>> {
        let (width, _) = terminal_size();
        let indent = self.theme.output_indent;
        let available = usize::from(width.saturating_sub(indent.saturating_mul(2))).max(1);

        let mut lines = vec![Line::default(); usize::from(self.theme.output_margin)];
        for source in text.lines() {
            let expanded = expand_tabs(source);
            let view = InputView {
                prompt: "",
                continuation: "",
                buffer: &expanded,
                cursor: 0,
            };
            for row in layout_input(&view, available).rows {
                lines.push(self.indented_line(&row.text, foreground));
            }
        }
        lines.extend(vec![Line::default(); usize::from(self.theme.output_margin)]);

        lines
    }

    /// One output line, indented inside its painted background.
    fn indented_line(&self, text: &str, foreground: ReplColor) -> Line<'static> {
        Line::from(vec![
            Span::raw(" ".repeat(usize::from(self.theme.output_indent))),
            Span::styled(
                text.to_string(),
                style_for(foreground, self.theme.output_background),
            ),
        ])
    }

}

impl Drop for NativeDisplay {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Everything needed to draw one input box, measured but not yet rendered.
///
/// WHY: the box's height has to be known before the terminal can be built at
/// that height, but the height depends on how the text wrapped — so measuring
/// and drawing are separate steps over the same value.
struct InputFrame {
    /// Gutter and text of each row that will be shown.
    rows: Vec<(String, String)>,
    /// Total rows the box occupies, chrome included.
    box_height: u16,
    /// Caret row, relative to the first visible text row.
    caret_row: u16,
    /// Caret column, measured from the left edge of the gutter.
    caret_col: u16,
}

impl InputFrame {
    /// Measure the box needed for `view` in a terminal of the given size.
    fn measure(
        theme: &ReplTheme,
        view: &InputView<'_>,
        terminal_width: u16,
        terminal_height: u16,
    ) -> Self {
        let chrome_columns = border_thickness(theme) * 2 + theme.padding.horizontal();
        let inner_width = usize::from(terminal_width.saturating_sub(chrome_columns)).max(1);

        let layout = layout_input(view, inner_width);

        let chrome_rows = border_thickness(theme) * 2 + theme.padding.vertical();
        // The box never takes the whole screen: leaving a row means output
        // inserted above it still has somewhere to land.
        let room_for_text = terminal_height
            .saturating_sub(chrome_rows)
            .saturating_sub(1)
            .max(1);
        let max_rows = usize::from(theme.max_input_rows.max(1).min(room_for_text));

        let (first, count) = visible_window(layout.rows.len(), layout.cursor_row, max_rows);
        let rows = layout
            .rows
            .iter()
            .skip(first)
            .take(count)
            .map(|row| (row.gutter.clone(), row.text.clone()))
            .collect::<Vec<_>>();

        let visible_rows = u16::try_from(rows.len()).unwrap_or(u16::MAX);

        Self {
            rows,
            box_height: visible_rows.saturating_add(chrome_rows),
            caret_row: u16::try_from(layout.cursor_row.saturating_sub(first)).unwrap_or(0),
            caret_col: u16::try_from(layout.cursor_col).unwrap_or(0),
        }
    }

    /// Draw the box into `buffer` at `area`.
    fn render(&self, theme: &ReplTheme, buffer: &mut Buffer, area: Rect) {
        let surface = style_for(theme.input_foreground, theme.input_background);
        let block = input_block(
            theme,
            style_for(theme.border, theme.input_background),
            surface,
        );
        let inner = block.inner(area);
        block.render(area, buffer);

        let prompt_style = style_for(theme.prompt_foreground, theme.input_background);
        let lines = self
            .rows
            .iter()
            .map(|(gutter, text)| {
                Line::from(vec![
                    Span::styled(gutter.clone(), prompt_style),
                    Span::styled(text.clone(), surface),
                ])
            })
            .collect::<Vec<_>>();

        Paragraph::new(Text::from(lines))
            .style(surface)
            .render(inner, buffer);
    }

    /// Absolute caret position for a box drawn at `area`.
    fn caret(&self, theme: &ReplTheme, area: Rect) -> (u16, u16) {
        let offset = border_thickness(theme);
        let column = area
            .x
            .saturating_add(offset)
            .saturating_add(theme.padding.left)
            .saturating_add(self.caret_col)
            .min(area.right().saturating_sub(1));
        let row = area
            .y
            .saturating_add(offset)
            .saturating_add(theme.padding.top)
            .saturating_add(self.caret_row)
            .min(area.bottom().saturating_sub(1));
        (column, row)
    }
}

/// The block that draws the border, padding and background of the input box.
fn input_block<'a>(theme: &ReplTheme, border: Style, surface: Style) -> Block<'a> {
    let padding = to_padding(theme.padding);
    let block = Block::default().style(surface).padding(padding);

    match theme.border_kind {
        BorderKind::None => block,
        kind => block
            .borders(Borders::ALL)
            .border_type(to_border_type(kind))
            .border_style(border),
    }
}

/// Rows and columns a border consumes on each side.
fn border_thickness(theme: &ReplTheme) -> u16 {
    u16::from(theme.border_kind != BorderKind::None)
}

fn to_border_type(kind: BorderKind) -> BorderType {
    match kind {
        // `None` never reaches here: `input_block` drops the borders instead.
        BorderKind::None | BorderKind::Plain => BorderType::Plain,
        BorderKind::Rounded => BorderType::Rounded,
        BorderKind::Thick => BorderType::Thick,
        BorderKind::Double => BorderType::Double,
    }
}

fn to_padding(padding: ReplPadding) -> Padding {
    Padding::new(padding.left, padding.right, padding.top, padding.bottom)
}

/// Turn a pair of theme colours into a render style.
///
/// With the `colors` feature off the box, its padding and the layout are still
/// drawn — only the colouring is dropped, so the REPL stays usable on terminals
/// that cannot do colour and in output that is being captured.
#[cfg(feature = "colors")]
fn style_for(foreground: ReplColor, background: ReplColor) -> Style {
    Style::default()
        .fg(to_color(foreground))
        .bg(to_color(background))
}

/// Turn a pair of theme colours into a render style.
///
/// See the `colors`-enabled variant: without that feature every style is empty.
#[cfg(not(feature = "colors"))]
fn style_for(_foreground: ReplColor, _background: ReplColor) -> Style {
    Style::default()
}

#[cfg(feature = "colors")]
fn to_color(color: ReplColor) -> ratatui::style::Color {
    use ratatui::style::Color;

    match color {
        ReplColor::Default => Color::Reset,
        ReplColor::Ansi(index) => Color::Indexed(index),
        ReplColor::Rgb(red, green, blue) => Color::Rgb(red, green, blue),
    }
}

/// The terminal's size, falling back to a conventional 80x24 when it cannot be
/// queried (a pipe, or a terminal that does not answer).
fn terminal_size() -> (u16, u16) {
    match crossterm::terminal::size() {
        Ok((width, height)) => (width.max(1), height.max(1)),
        Err(error) => {
            tracing::debug!(%error, "repl: terminal size unavailable, assuming 80x24");
            (FALLBACK_TERMINAL_WIDTH, FALLBACK_TERMINAL_HEIGHT)
        }
    }
}

/// Replace tabs with spaces so text cannot escape the painted region.
fn expand_tabs(text: &str) -> String {
    if !text.contains('\t') {
        return text.to_string();
    }
    text.replace('\t', &" ".repeat(TAB_WIDTH))
}
