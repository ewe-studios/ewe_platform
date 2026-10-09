//! The REPL's renderer, generic over whatever surface it draws on.
//!
//! WHY: painting the prompt and the typed text as one flat line makes the two
//! impossible to tell apart, and any incremental repaint scheme eventually
//! disagrees with the buffer — that is how the prompt used to get erased on the
//! first keystroke. Drawing the whole area from the buffer every time removes
//! the class of bug entirely. Doing it against a cell buffer rather than a
//! terminal means the same code renders in a terminal, a browser or a test.
//!
//! WHAT: the live input sits inside a bordered box with a lighter background;
//! banners, submitted input and responses go above it, painted the full width
//! of the surface in a darker shade. While the caller works, the box is
//! replaced by an animated indicator that streamed results flow out of.
//!
//! HOW: everything is measured into a [`Buffer`] and flushed through the host's
//! [`Backend`](ratatui::backend::Backend). Finished lines either go to the
//! surface's own scrollback ([`ViewportMode::Inline`]) or into a transcript the
//! renderer redraws itself ([`ViewportMode::Fullscreen`]).

use std::io;

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Borders, LineGauge, Padding, Paragraph, Widget};
use ratatui::{Terminal, TerminalOptions, Viewport};

use crate::shared::activity::ActivityView;
use crate::shared::host::{ReplHost, ViewportMode};
use crate::shared::layout::{layout_input, split_committed, visible_window, wrap_text, InputView};
use crate::shared::theme::{BorderKind, ReplColor, ReplPadding, ReplTheme};
use crate::shared::traits::ReplDisplay;

/// Rows the box falls back to before anything has been drawn.
const INITIAL_VIEWPORT_HEIGHT: u16 = 1;

/// Size assumed when the surface will not report its own.
const FALLBACK_SIZE: (u16, u16) = (80, 24);

/// Spaces a tab is expanded to before rendering.
///
/// Terminals expand a literal tab to their own tab stops, which would slide
/// text out from under the box border, so tabs never reach the screen.
const TAB_WIDTH: usize = 4;

/// Transcript rows kept in [`ViewportMode::Fullscreen`] before the oldest are
/// dropped.
///
/// A surface with no scrollback of its own cannot page back through history, so
/// holding an unbounded transcript would only grow memory for text nobody can
/// reach.
const MAX_RETAINED_ROWS: usize = 10_000;

/// Draws the REPL onto the surface its host provides.
pub struct ReplRenderer<H: ReplHost> {
    host: H,
    theme: ReplTheme,
    terminal: Option<Terminal<H::Backend>>,
    viewport_height: u16,
    entered: bool,
    /// The indicator currently being animated, if any.
    activity: Option<ActivitySnapshot>,
    /// Streamed text that has not filled a row yet, so may still change.
    stream_tail: String,
    /// Whether this activity has streamed anything, so its trailing blank row
    /// is only printed when there is something for it to trail.
    streamed: bool,
    /// Everything printed so far, kept only when the surface has no scrollback.
    transcript: Vec<Line<'static>>,
}

/// The last state the animation thread asked for.
///
/// WHY: text streamed in between animation ticks has to be redrawn straight
/// away, and that redraw needs the indicator to keep whatever frame, label and
/// progress it was already showing.
#[derive(Debug, Clone, Default)]
struct ActivitySnapshot {
    frame: String,
    label: String,
    elapsed: Option<core::time::Duration>,
    progress: Option<f64>,
}

impl<H: ReplHost> ReplRenderer<H> {
    /// Build a renderer that draws through `host` in `theme`.
    pub fn new(host: H, theme: ReplTheme) -> Self {
        Self {
            host,
            theme,
            terminal: None,
            viewport_height: INITIAL_VIEWPORT_HEIGHT,
            entered: false,
            activity: None,
            stream_tail: String::new(),
            streamed: false,
            transcript: Vec::new(),
        }
    }

    /// The theme this renderer draws in.
    pub fn theme(&self) -> &ReplTheme {
        &self.theme
    }

    /// The surface's size, falling back to a conventional 80x24.
    fn size(&mut self) -> (u16, u16) {
        match self.host.size() {
            Ok((width, height)) => (width.max(1), height.max(1)),
            Err(error) => {
                tracing::debug!(%error, "repl: surface size unavailable, assuming 80x24");
                FALLBACK_SIZE
            }
        }
    }

    /// Whether finished lines go to the surface's scrollback or our transcript.
    fn keeps_own_transcript(&self) -> bool {
        self.host.viewport_mode() == ViewportMode::Fullscreen
    }

    /// Make sure a terminal exists whose viewport is `height` rows tall.
    ///
    /// An inline viewport's height is fixed when its terminal is built, so a
    /// box that has grown or shrunk needs a new one. The old box is cleared and
    /// the caret returned to the row it started on first, so the replacement
    /// anchors in the same place instead of walking down the screen. A
    /// fullscreen viewport owns the surface and is built once.
    fn attach(&mut self, height: u16) -> io::Result<()> {
        if !self.entered {
            self.host.enter()?;
            self.entered = true;
        }

        let fullscreen = self.keeps_own_transcript();

        if let Some(terminal) = self.terminal.as_mut() {
            if fullscreen || self.viewport_height == height {
                return Ok(());
            }
            let anchor = terminal.get_frame().area().y;
            terminal.clear().map_err(backend_error)?;
            terminal
                .set_cursor_position(Position::new(0, anchor))
                .map_err(backend_error)?;
        }

        let viewport = if fullscreen {
            Viewport::Fullscreen
        } else {
            Viewport::Inline(height)
        };

        self.terminal = None;
        self.viewport_height = height;
        let backend = self.host.create_backend()?;
        self.terminal = Some(
            Terminal::with_options(backend, TerminalOptions { viewport })
                .map_err(backend_error)?,
        );

        Ok(())
    }

    /// Push `lines` above the input box.
    ///
    /// In [`ViewportMode::Inline`] they are handed to the surface's own
    /// scrollback; otherwise they join the transcript this renderer redraws.
    fn insert_above(&mut self, lines: Vec<Line<'static>>, style: Style) {
        if lines.is_empty() {
            return;
        }

        if self.keeps_own_transcript() {
            self.transcript
                .extend(lines.into_iter().map(|line| line.style(style)));
            if self.transcript.len() > MAX_RETAINED_ROWS {
                let excess = self.transcript.len() - MAX_RETAINED_ROWS;
                self.transcript.drain(..excess);
            }
            self.redraw();
            return;
        }

        let Ok(height) = u16::try_from(lines.len()) else {
            tracing::warn!(
                rows = lines.len(),
                "repl: refusing to print an oversized block"
            );
            return;
        };

        if let Err(error) = self.attach(self.viewport_height) {
            tracing::error!(%error, "repl: could not prepare the viewport");
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

    /// Repaint whatever the box is currently showing.
    ///
    /// Only a fullscreen surface needs this: an inline viewport is repainted by
    /// the surface itself when lines are inserted above it.
    fn redraw(&mut self) {
        if let Some(activity) = self.activity.clone() {
            self.draw_activity(&activity);
        }
    }

    /// Draw `body` — the box or the indicator — with the transcript above it.
    ///
    /// `body_height` is what the box wants; the transcript takes whatever is
    /// left, which is nothing at all in inline mode.
    fn draw(&mut self, body_height: u16, render_body: impl FnOnce(&ReplTheme, &mut Buffer, Rect)) {
        let (_, surface_height) = self.size();
        let fullscreen = self.keeps_own_transcript();

        let requested = if fullscreen { surface_height } else { body_height };
        if let Err(error) = self.attach(requested.min(surface_height).max(1)) {
            tracing::error!(%error, "repl: could not prepare the viewport");
            return;
        }

        let theme = self.theme.clone();
        let transcript = if fullscreen {
            self.transcript.clone()
        } else {
            Vec::new()
        };
        let output_style = style_for(theme.output_foreground, theme.output_background);

        let Some(terminal) = self.terminal.as_mut() else {
            return;
        };

        let outcome = terminal.draw(|target| {
            let area = target.area();
            let body_rows = body_height.min(area.height);
            let body_area = Rect {
                y: area.bottom().saturating_sub(body_rows),
                height: body_rows,
                ..area
            };

            if fullscreen {
                let history_area = Rect {
                    height: area.height.saturating_sub(body_rows),
                    ..area
                };
                // Only the tail fits, and the tail is what matters — the rest
                // has already scrolled out of reach.
                let skip = transcript.len().saturating_sub(usize::from(history_area.height));
                Paragraph::new(Text::from(transcript[skip..].to_vec()))
                    .style(output_style)
                    .render(history_area, target.buffer_mut());
            }

            render_body(&theme, target.buffer_mut(), body_area);
        });

        if let Err(error) = outcome {
            tracing::error!(%error, "repl: could not draw");
        }
    }

    /// Draw the activity indicator, with any live streamed tail above it.
    fn draw_activity(&mut self, activity: &ActivitySnapshot) {
        let (width, height) = self.size();
        let tail = self.tail_line();
        let chrome = border_thickness(&self.theme) * 2 + self.theme.padding.vertical();
        let tail_rows = u16::from(tail.is_some());
        let box_height = (chrome + 1).min(height.saturating_sub(tail_rows).max(1));
        let total = box_height.saturating_add(tail_rows).min(height);

        let activity = activity.clone();
        let output_style = style_for(self.theme.output_foreground, self.theme.output_background);

        self.draw(total, move |theme, buffer, area| {
            let box_area = if let Some(tail) = tail {
                let tail_area = Rect {
                    height: tail_rows,
                    ..area
                };
                Paragraph::new(tail).style(output_style).render(tail_area, buffer);
                Rect {
                    y: area.y.saturating_add(tail_rows),
                    height: area.height.saturating_sub(tail_rows),
                    ..area
                }
            } else {
                area
            };

            render_activity_box(theme, &activity, buffer, box_area, width);
        });
    }

    /// The streamed tail as a renderable line, if there is one.
    fn tail_line(&self) -> Option<Line<'static>> {
        if self.stream_tail.is_empty() {
            return None;
        }
        Some(self.indented_line(&self.stream_tail, self.theme.output_foreground))
    }

    /// Commit every streamed row that can no longer change.
    fn commit_full_rows(&mut self) {
        let width = self.output_width();
        let tail = core::mem::take(&mut self.stream_tail);
        let (committed, remainder) = split_committed(&tail, width);

        if !committed.is_empty() {
            let lines = committed
                .iter()
                .map(|row| self.indented_line(row, self.theme.output_foreground))
                .collect::<Vec<_>>();
            let style = style_for(self.theme.output_foreground, self.theme.output_background);
            self.insert_above(lines, style);
        }

        self.stream_tail = remainder;
    }

    /// Columns available to output text once its indent is taken off.
    fn output_width(&mut self) -> usize {
        let (width, _) = self.size();
        usize::from(width.saturating_sub(self.theme.output_indent.saturating_mul(2))).max(1)
    }

    /// Wrap `text` to the surface width and surround it with blank shaded rows.
    fn body_lines(&mut self, text: &str, foreground: ReplColor) -> Vec<Line<'static>> {
        let available = self.output_width();
        let margin = usize::from(self.theme.output_margin);

        let mut lines = vec![Line::default(); margin];
        for row in wrap_text(&expand_tabs(text), available) {
            lines.push(self.indented_line(&row, foreground));
        }
        lines.extend(vec![Line::default(); margin]);

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

impl<H: ReplHost> ReplDisplay for ReplRenderer<H> {
    fn print_banner(&mut self, banner: &str) {
        let style = style_for(self.theme.banner_foreground, self.theme.output_background);
        let mut lines = vec![Line::default()];
        for line in banner.lines() {
            lines.push(self.indented_line(line, self.theme.banner_foreground));
        }
        lines.push(Line::default());
        self.insert_above(lines, style);
    }

    fn render_input(&mut self, view: &InputView<'_>) {
        let (width, height) = self.size();
        let frame = InputFrame::measure(&self.theme, view, width, height);
        let box_height = frame.box_height;

        self.draw(box_height, move |theme, buffer, area| {
            frame.render(theme, buffer, area);
        });

        // The caret is set after the body render so it survives the transcript
        // being drawn in fullscreen mode.
        let (width, height) = self.size();
        let frame = InputFrame::measure(&self.theme, view, width, height);
        let theme = self.theme.clone();
        if let Some(terminal) = self.terminal.as_mut() {
            let area = terminal.get_frame().area();
            let body = Rect {
                y: area.bottom().saturating_sub(box_height.min(area.height)),
                height: box_height.min(area.height),
                ..area
            };
            let (column, row) = frame.caret(&theme, body);
            if let Err(error) = terminal.set_cursor_position(Position::new(column, row)) {
                tracing::debug!(%error, "repl: could not place the caret");
            }
            if let Err(error) = terminal.show_cursor() {
                tracing::debug!(%error, "repl: could not show the caret");
            }
        }
    }

    fn finish_input(&mut self, view: &InputView<'_>) {
        let (width, height) = self.size();
        let frame = InputFrame::measure(&self.theme, view, width, height);
        let theme = self.theme.clone();

        if self.keeps_own_transcript() {
            // No surface scrollback to push into, so the finished box is
            // rendered to rows and kept in the transcript.
            let rows = frame.to_lines(&theme);
            let style = style_for(theme.input_foreground, theme.input_background);
            self.insert_above(rows, style);
            return;
        }

        if let Err(error) = self.attach(self.viewport_height) {
            tracing::error!(%error, "repl: could not prepare the viewport");
            return;
        }

        let Some(terminal) = self.terminal.as_mut() else {
            return;
        };

        // Re-emitting the box above the viewport moves it into the surface's
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

    fn render_activity(&mut self, view: &ActivityView<'_>) {
        let snapshot = ActivitySnapshot {
            frame: view.frame.to_string(),
            label: view.label.to_string(),
            elapsed: view.elapsed,
            progress: view.progress,
        };
        self.activity = Some(snapshot.clone());
        self.draw_activity(&snapshot);
    }

    fn stream_push(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        if !self.streamed {
            self.streamed = true;
            let blank = vec![Line::default(); usize::from(self.theme.output_margin)];
            let style = style_for(self.theme.output_foreground, self.theme.output_background);
            self.insert_above(blank, style);
        }

        self.stream_tail.push_str(&expand_tabs(text));
        self.commit_full_rows();

        if let Some(activity) = self.activity.clone() {
            self.draw_activity(&activity);
        }
    }

    fn end_activity(&mut self) {
        // Whatever is left in the tail never filled a row, so it is committed
        // as its own final row rather than being dropped.
        let tail = core::mem::take(&mut self.stream_tail);
        let style = style_for(self.theme.output_foreground, self.theme.output_background);

        if !tail.trim_end().is_empty() {
            let line = self.indented_line(tail.trim_end(), self.theme.output_foreground);
            self.insert_above(vec![line], style);
        }

        if self.streamed {
            let blank = vec![Line::default(); usize::from(self.theme.output_margin)];
            self.insert_above(blank, style);
        }

        self.activity = None;
        self.streamed = false;
    }

    fn clear_screen(&mut self) {
        self.transcript.clear();

        // An inline viewport is anchored to a row that is about to stop
        // existing, so the terminal is rebuilt from the top of a cleared
        // surface rather than reused.
        if let Some(terminal) = self.terminal.as_mut() {
            let cleared = terminal
                .clear()
                .and_then(|()| terminal.set_cursor_position(Position::new(0, 0)));
            if let Err(error) = cleared {
                tracing::error!(%error, "repl: could not clear the surface");
            }
        }

        if !self.keeps_own_transcript() {
            self.terminal = None;
        }
    }

    fn shutdown(&mut self) {
        if let Some(mut terminal) = self.terminal.take() {
            // Wipe the live box, then park the caret on the row it started at
            // so whatever prints next lands there.
            let anchor = terminal.get_frame().area().y;
            let restored = terminal
                .clear()
                .and_then(|()| terminal.set_cursor_position(Position::new(0, anchor)))
                .and_then(|()| terminal.show_cursor())
                .and_then(|()| terminal.flush());
            if let Err(error) = restored {
                tracing::warn!(%error, "repl: could not restore the surface");
            }
        }

        if self.entered {
            if let Err(error) = self.host.leave() {
                tracing::error!(%error, "repl: could not restore the host");
            }
            self.entered = false;
        }
    }
}

impl<H: ReplHost> Drop for ReplRenderer<H> {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl<H: ReplHost> core::fmt::Debug for ReplRenderer<H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReplRenderer")
            .field("mode", &self.host.viewport_mode())
            .field("attached", &self.terminal.is_some())
            .field("viewport_height", &self.viewport_height)
            .field("transcript_rows", &self.transcript.len())
            .finish_non_exhaustive()
    }
}

impl<H: ReplHost> core::fmt::Display for ReplRenderer<H> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "ReplRenderer({}, {} rows)",
            self.host.viewport_mode(),
            self.viewport_height
        )
    }
}

/// Everything needed to draw one input box, measured but not yet rendered.
///
/// WHY: the box's height has to be known before the viewport can be sized to
/// it, but the height depends on how the text wrapped — so measuring and
/// drawing are separate steps over the same value.
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
    /// Measure the box needed for `view` on a surface of the given size.
    fn measure(theme: &ReplTheme, view: &InputView<'_>, width: u16, height: u16) -> Self {
        let chrome_columns = border_thickness(theme) * 2 + theme.padding.horizontal();
        let inner_width = usize::from(width.saturating_sub(chrome_columns)).max(1);

        let layout = layout_input(view, inner_width);

        let chrome_rows =
            border_thickness(theme) * 2 + theme.padding.vertical() + hint_rows(theme);
        // The box never takes the whole surface: leaving a row means output
        // inserted above it still has somewhere to land.
        let room_for_text = height
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
            theme.hint.as_deref(),
        );
        let inner = block.inner(area);
        block.render(area, buffer);

        Paragraph::new(Text::from(self.text_lines(theme)))
            .style(surface)
            .render(inner, buffer);
    }

    /// The box's text rows as styled lines.
    fn text_lines(&self, theme: &ReplTheme) -> Vec<Line<'static>> {
        let surface = style_for(theme.input_foreground, theme.input_background);
        let prompt_style = style_for(theme.prompt_foreground, theme.input_background);

        self.rows
            .iter()
            .map(|(gutter, text)| {
                Line::from(vec![
                    Span::styled(gutter.clone(), prompt_style),
                    Span::styled(text.clone(), surface),
                ])
            })
            .collect()
    }

    /// The whole box — borders included — as lines, for surfaces that have to
    /// keep their own transcript.
    fn to_lines(&self, theme: &ReplTheme) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        for _ in 0..theme.padding.top {
            lines.push(Line::default());
        }
        let pad = " ".repeat(usize::from(theme.padding.left));
        for line in self.text_lines(theme) {
            let mut spans = vec![Span::raw(pad.clone())];
            spans.extend(line.spans);
            lines.push(Line::from(spans));
        }
        for _ in 0..theme.padding.bottom {
            lines.push(Line::default());
        }
        lines
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

/// Draw the indicator itself into `area`.
///
/// WHY: an indeterminate wait and a measurable one want different shapes — a
/// spinner says "still working", a bar says "this far through". The caller
/// decides which by reporting progress or not.
fn render_activity_box(
    theme: &ReplTheme,
    activity: &ActivitySnapshot,
    buffer: &mut Buffer,
    area: Rect,
    _width: u16,
) {
    let surface = style_for(theme.input_foreground, theme.input_background);
    // No hint on the indicator: none of those keys do anything while the
    // caller is working, so advertising them there would be a lie.
    let block = input_block(
        theme,
        style_for(theme.border, theme.input_background),
        surface,
        None,
    );
    let inner = block.inner(area);
    block.render(area, buffer);

    let label_style = style_for(theme.activity.label_foreground, theme.input_background);
    let elapsed_style = style_for(theme.activity.elapsed_foreground, theme.input_background);
    let elapsed = activity
        .elapsed
        .map(|elapsed| format!("  {:.1}s", elapsed.as_secs_f64()));

    if let Some(ratio) = activity.progress {
        let mut label = activity.label.clone();
        if let Some(elapsed) = elapsed {
            label.push_str(&elapsed);
        }

        LineGauge::default()
            .ratio(ratio.clamp(0.0, 1.0))
            .filled_style(style_for(theme.activity.foreground, theme.input_background))
            .unfilled_style(style_for(theme.border, theme.input_background))
            .label(Span::styled(label, label_style))
            .style(surface)
            .render(inner, buffer);
        return;
    }

    let mut spans = Vec::new();
    if !activity.frame.is_empty() {
        spans.push(Span::styled(
            activity.frame.clone(),
            style_for(theme.activity.foreground, theme.input_background),
        ));
        spans.push(Span::styled(" ", surface));
    }
    spans.push(Span::styled(activity.label.clone(), label_style));
    if let Some(elapsed) = elapsed {
        spans.push(Span::styled(elapsed, elapsed_style));
    }

    Paragraph::new(Line::from(spans))
        .style(surface)
        .render(inner, buffer);
}

/// The block that draws the border, padding and background of the input box.
///
/// `hint` rides in the bottom border when there is one, so the reminder costs
/// no rows of its own.
fn input_block<'a>(
    theme: &ReplTheme,
    border: Style,
    surface: Style,
    hint: Option<&str>,
) -> Block<'a> {
    let mut block = Block::default()
        .style(surface)
        .padding(to_padding(theme.padding));

    block = match theme.border_kind {
        BorderKind::None => block,
        kind => block
            .borders(Borders::ALL)
            .border_type(to_border_type(kind))
            .border_style(border),
    };

    match hint {
        Some(hint) if !hint.is_empty() => block.title_bottom(
            Line::from(Span::styled(
                hint.to_string(),
                style_for(theme.hint_foreground, theme.input_background),
            ))
            .right_aligned(),
        ),
        _ => block,
    }
}

/// Rows and columns a border consumes on each side.
fn border_thickness(theme: &ReplTheme) -> u16 {
    u16::from(theme.border_kind != BorderKind::None)
}

/// Extra rows the hint needs beyond the chrome already being drawn.
///
/// WHY this is not always zero: a bottom title rides free inside a bottom
/// border, but with no border to sit in it takes a row from the content
/// instead — which silently collapsed the text area to nothing. Borderless
/// themes therefore pay one row for the hint rather than losing the text.
fn hint_rows(theme: &ReplTheme) -> u16 {
    let wanted = theme.hint.as_deref().is_some_and(|hint| !hint.is_empty());
    u16::from(wanted && theme.border_kind == BorderKind::None)
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
/// drawn — only the colouring is dropped, so the REPL stays usable on surfaces
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

/// Flatten a backend's own error type into an [`io::Error`].
///
/// WHY: backends disagree about their error type, and the renderer only ever
/// reports these — it never inspects or recovers from them. Carrying the
/// message across is all that is needed, and it keeps the renderer usable with
/// backends whose errors are not `'static`.
fn backend_error(error: impl core::fmt::Display) -> io::Error {
    io::Error::other(error.to_string())
}

/// Replace tabs with spaces so text cannot escape the painted region.
fn expand_tabs(text: &str) -> String {
    if !text.contains('\t') {
        return text.to_string();
    }
    text.replace('\t', &" ".repeat(TAB_WIDTH))
}
