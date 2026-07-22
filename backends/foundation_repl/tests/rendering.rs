//! Renders the REPL through a second, non-terminal backend and asserts on the
//! cells it produces.
//!
//! WHY this exists: the claim that the renderer is backend-independent is only
//! worth anything if something other than a terminal actually drives it. These
//! tests run the real [`ReplRenderer`] against ratatui's `TestBackend` — no
//! terminal, no raw mode, no pty — and read the resulting glyphs back.

use std::cell::RefCell;
use std::io;
use std::rc::Rc;

use foundation_repl::{
    ActivityView, BorderKind, InputView, ReplDisplay, ReplHost, ReplRenderer, ReplTheme,
    ViewportMode,
};
use ratatui::backend::TestBackend;

/// A host that renders off-screen and hands back what was drawn.
///
/// The backend is shared with the test so the buffer survives the renderer
/// taking ownership of it.
struct OffScreen {
    width: u16,
    height: u16,
    last: Rc<RefCell<Option<TestBackend>>>,
}

impl ReplHost for OffScreen {
    type Backend = SharedTestBackend;

    fn create_backend(&mut self) -> io::Result<Self::Backend> {
        Ok(SharedTestBackend {
            inner: TestBackend::new(self.width, self.height),
            sink: Rc::clone(&self.last),
        })
    }

    fn size(&mut self) -> io::Result<(u16, u16)> {
        Ok((self.width, self.height))
    }

    fn viewport_mode(&self) -> ViewportMode {
        // TestBackend has no scrollback of its own, so the renderer keeps the
        // transcript — which is exactly the browser-style path.
        ViewportMode::Fullscreen
    }
}

/// A `TestBackend` that publishes itself on drop and on flush, so assertions
/// can see the buffer the renderer drew into.
struct SharedTestBackend {
    inner: TestBackend,
    sink: Rc<RefCell<Option<TestBackend>>>,
}

impl SharedTestBackend {
    fn publish(&self) {
        *self.sink.borrow_mut() = Some(self.inner.clone());
    }
}

impl ratatui::backend::Backend for SharedTestBackend {
    type Error = <TestBackend as ratatui::backend::Backend>::Error;

    fn draw<'a, I>(&mut self, content: I) -> Result<(), Self::Error>
    where
        I: Iterator<Item = (u16, u16, &'a ratatui::buffer::Cell)>,
    {
        self.inner.draw(content)?;
        self.publish();
        Ok(())
    }

    fn hide_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> Result<(), Self::Error> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> Result<ratatui::layout::Position, Self::Error> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<ratatui::layout::Position>>(
        &mut self,
        position: P,
    ) -> Result<(), Self::Error> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.inner.clear()
    }

    fn clear_region(
        &mut self,
        region: ratatui::backend::ClearType,
    ) -> Result<(), Self::Error> {
        self.inner.clear_region(region)
    }

    fn size(&self) -> Result<ratatui::layout::Size, Self::Error> {
        self.inner.size()
    }

    fn window_size(&mut self) -> Result<ratatui::backend::WindowSize, Self::Error> {
        self.inner.window_size()
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        self.inner.flush()?;
        self.publish();
        Ok(())
    }
}

/// Build a renderer over an off-screen surface, plus a handle to read it back.
fn off_screen(
    width: u16,
    height: u16,
    theme: ReplTheme,
) -> (ReplRenderer<OffScreen>, Rc<RefCell<Option<TestBackend>>>) {
    let last = Rc::new(RefCell::new(None));
    let host = OffScreen {
        width,
        height,
        last: Rc::clone(&last),
    };
    (ReplRenderer::new(host, theme), last)
}

/// The drawn surface as plain rows of text.
fn rows(sink: &Rc<RefCell<Option<TestBackend>>>) -> Vec<String> {
    let borrowed = sink.borrow();
    let backend = borrowed.as_ref().expect("nothing was ever drawn");
    let buffer = backend.buffer();

    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

fn view(buffer: &str, cursor: usize) -> InputView<'_> {
    InputView {
        prompt: "| ",
        continuation: "|... ",
        buffer,
        cursor,
    }
}

// ── The box itself ──────────────────────────────────────────────────────

#[test]
fn the_input_box_is_drawn_with_a_border_and_the_prompt_inside_it() {
    let (mut renderer, sink) = off_screen(30, 10, ReplTheme::default());
    renderer.render_input(&view("hello", 5));

    let drawn = rows(&sink).join("\n");

    assert!(
        drawn.contains('╭') && drawn.contains('╮'),
        "expected a rounded border, got:\n{drawn}"
    );
    assert!(
        drawn.contains("| hello"),
        "the prompt must still be in front of the text:\n{drawn}"
    );
}

#[test]
fn typing_never_erases_the_prompt() {
    // The original bug: each keystroke repainted from column zero and dropped
    // the prompt. Replay a whole word and check every frame.
    let (mut renderer, sink) = off_screen(30, 10, ReplTheme::default());

    for length in 0..=5 {
        renderer.render_input(&view(&"abcde"[..length], length));
        let drawn = rows(&sink).join("\n");
        assert!(
            drawn.contains('|'),
            "prompt vanished after typing {length} characters:\n{drawn}"
        );
    }
}

#[test]
fn the_text_is_padded_away_from_the_border() {
    let (mut renderer, sink) = off_screen(30, 10, ReplTheme::default());
    renderer.render_input(&view("x", 1));

    let drawn = rows(&sink);
    let text_row = drawn
        .iter()
        .find(|row| row.contains("| x"))
        .expect("the typed line should be on screen");

    let border = text_row.find('│').expect("row should start with a border");
    let prompt = text_row.find('|').expect("row should carry the prompt");
    assert!(
        prompt > border + 1,
        "text is flush against the border in {text_row:?}"
    );
}

#[test]
fn a_borderless_theme_still_pads_and_renders() {
    let theme = ReplTheme {
        border_kind: BorderKind::None,
        ..ReplTheme::default()
    };
    let (mut renderer, sink) = off_screen(30, 10, theme);
    renderer.render_input(&view("hi", 2));

    let drawn = rows(&sink).join("\n");
    assert!(drawn.contains("| hi"), "got:\n{drawn}");
    assert!(!drawn.contains('╭'), "no border was asked for:\n{drawn}");
}

#[test]
fn long_input_wraps_inside_the_box_rather_than_through_it() {
    let (mut renderer, sink) = off_screen(24, 12, ReplTheme::default());
    let text = "the quick brown fox jumps over the lazy dog";
    renderer.render_input(&view(text, text.len()));

    for row in rows(&sink) {
        assert!(
            row.chars().count() <= 24,
            "row {row:?} is wider than the surface"
        );
    }
}

#[test]
fn multiline_input_shows_the_continuation_prompt() {
    let (mut renderer, sink) = off_screen(30, 12, ReplTheme::default());
    renderer.render_input(&view("one\ntwo", 7));

    let drawn = rows(&sink).join("\n");
    assert!(drawn.contains("| one"), "got:\n{drawn}");
    assert!(drawn.contains("|... two"), "got:\n{drawn}");
}

// ── Output ──────────────────────────────────────────────────────────────

#[test]
fn a_response_is_drawn_above_the_box() {
    let (mut renderer, sink) = off_screen(40, 12, ReplTheme::default());
    renderer.print_response("the answer is 42");
    renderer.render_input(&view("", 0));

    let drawn = rows(&sink);
    let answer = drawn
        .iter()
        .position(|row| row.contains("the answer is 42"))
        .expect("the response should be on screen");
    let box_top = drawn
        .iter()
        .position(|row| row.contains('╭'))
        .expect("the input box should be on screen");

    assert!(
        answer < box_top,
        "output must sit above the input box, got:\n{}",
        drawn.join("\n")
    );
}

#[test]
fn a_response_word_wraps_instead_of_splitting_words() {
    let (mut renderer, sink) = off_screen(24, 14, ReplTheme::default());
    renderer.print_response("committing each row to the scrollback");
    renderer.render_input(&view("", 0));

    let drawn = rows(&sink).join("\n");
    assert!(
        !drawn.contains("scrollbac\n"),
        "a word was split across rows:\n{drawn}"
    );
}

// ── The activity indicator ──────────────────────────────────────────────

#[test]
fn the_indicator_shows_its_frame_and_label() {
    let (mut renderer, sink) = off_screen(40, 10, ReplTheme::default());
    renderer.render_activity(&ActivityView {
        frame: "⠙",
        label: "thinking",
        elapsed: None,
        progress: None,
    });

    let drawn = rows(&sink).join("\n");
    assert!(drawn.contains('⠙'), "no animation frame in:\n{drawn}");
    assert!(drawn.contains("thinking"), "no label in:\n{drawn}");
}

#[test]
fn reporting_progress_draws_a_bar_rather_than_a_spinner() {
    let (mut renderer, sink) = off_screen(40, 10, ReplTheme::default());
    renderer.render_activity(&ActivityView {
        frame: "⠙",
        label: "downloading",
        elapsed: None,
        progress: Some(0.5),
    });

    let drawn = rows(&sink).join("\n");
    assert!(drawn.contains("downloading"), "no label in:\n{drawn}");
    assert!(
        !drawn.contains('⠙'),
        "the spinner frame should give way to the bar:\n{drawn}"
    );
}

#[test]
fn streamed_text_appears_while_the_indicator_is_still_running() {
    let (mut renderer, sink) = off_screen(40, 12, ReplTheme::default());
    renderer.render_activity(&ActivityView {
        frame: "⠙",
        label: "thinking",
        elapsed: None,
        progress: None,
    });

    renderer.stream_push("partial answer so far");

    let drawn = rows(&sink).join("\n");
    assert!(
        drawn.contains("partial answer so far"),
        "streamed text should be visible before the activity ends:\n{drawn}"
    );
    assert!(
        drawn.contains("thinking"),
        "the indicator should still be running:\n{drawn}"
    );
}

#[test]
fn ending_an_activity_flushes_the_unfinished_tail() {
    let (mut renderer, sink) = off_screen(40, 12, ReplTheme::default());
    renderer.render_activity(&ActivityView {
        frame: "⠙",
        label: "thinking",
        elapsed: None,
        progress: None,
    });
    renderer.stream_push("a short tail that never filled a row");
    renderer.end_activity();
    renderer.render_input(&view("", 0));

    let drawn = rows(&sink).join("\n");
    assert!(
        drawn.contains("a short tail"),
        "the tail must not be dropped when the activity ends:\n{drawn}"
    );
}

// ── Session behaviour ───────────────────────────────────────────────────

#[test]
fn a_submitted_message_stays_on_screen() {
    let (mut renderer, sink) = off_screen(40, 16, ReplTheme::default());
    renderer.render_input(&view("what is 2+2", 11));
    renderer.finish_input(&view("what is 2+2", 11));
    renderer.print_response("4");
    renderer.render_input(&view("", 0));

    let drawn = rows(&sink).join("\n");
    assert!(
        drawn.contains("what is 2+2"),
        "the submitted message should still be visible:\n{drawn}"
    );
    assert!(drawn.contains('4'), "the reply should be visible:\n{drawn}");
}

#[test]
fn clearing_the_screen_drops_the_transcript() {
    let (mut renderer, sink) = off_screen(40, 12, ReplTheme::default());
    renderer.print_response("old output");
    renderer.clear_screen();
    renderer.render_input(&view("", 0));

    let drawn = rows(&sink).join("\n");
    assert!(!drawn.contains("old output"), "got:\n{drawn}");
}

#[test]
fn a_surface_too_small_for_the_box_does_not_panic() {
    for (width, height) in [(1_u16, 1_u16), (3, 2), (8, 3), (2, 20)] {
        let (mut renderer, _sink) = off_screen(width, height, ReplTheme::default());
        renderer.render_input(&view("some text that will not fit", 27));
        renderer.print_response("a response");
        renderer.render_activity(&ActivityView {
            frame: "⠙",
            label: "working",
            elapsed: None,
            progress: Some(0.25),
        });
        renderer.end_activity();
    }
}

// ── The keybinding hint ─────────────────────────────────────────────────

#[test]
fn the_hint_is_drawn_in_the_boxs_bottom_border() {
    let (mut renderer, sink) = off_screen(60, 10, ReplTheme::default());
    renderer.render_input(&view("hi", 2));

    let drawn = rows(&sink);
    let hint_row = drawn
        .iter()
        .position(|row| row.contains("ctrl+u clear"))
        .expect("the hint should be on screen");
    let text_row = drawn
        .iter()
        .position(|row| row.contains("| hi"))
        .expect("the typed line should be on screen");

    assert!(
        hint_row > text_row,
        "the hint belongs below the text, got:\n{}",
        drawn.join("\n")
    );
    assert!(
        drawn[hint_row].contains('╰') || drawn[hint_row].contains('─'),
        "the hint should ride in the border rather than take a row of its own: \
         {:?}",
        drawn[hint_row]
    );
}

#[test]
fn the_hint_costs_no_extra_rows() {
    let with_hint = {
        let (mut renderer, sink) = off_screen(60, 12, ReplTheme::default());
        renderer.render_input(&view("hi", 2));
        rows(&sink)
            .iter()
            .filter(|row| !row.is_empty())
            .count()
    };
    let without_hint = {
        let theme = ReplTheme {
            hint: None,
            ..ReplTheme::default()
        };
        let (mut renderer, sink) = off_screen(60, 12, theme);
        renderer.render_input(&view("hi", 2));
        rows(&sink)
            .iter()
            .filter(|row| !row.is_empty())
            .count()
    };

    assert_eq!(
        with_hint, without_hint,
        "the hint should ride in the border, not add a row"
    );
}

#[test]
fn the_hint_can_be_turned_off() {
    let theme = ReplTheme {
        hint: None,
        ..ReplTheme::default()
    };
    let (mut renderer, sink) = off_screen(60, 10, theme);
    renderer.render_input(&view("hi", 2));

    let drawn = rows(&sink).join("\n");
    assert!(!drawn.contains("ctrl+u"), "got:\n{drawn}");
}

#[test]
fn the_hint_can_be_replaced() {
    let theme = ReplTheme {
        hint: Some(" press ctrl+u ".into()),
        ..ReplTheme::default()
    };
    let (mut renderer, sink) = off_screen(60, 10, theme);
    renderer.render_input(&view("hi", 2));

    let drawn = rows(&sink).join("\n");
    assert!(drawn.contains("press ctrl+u"), "got:\n{drawn}");
}

#[test]
fn the_indicator_does_not_advertise_keys_that_do_nothing_while_it_runs() {
    let (mut renderer, sink) = off_screen(60, 10, ReplTheme::default());
    renderer.render_activity(&ActivityView {
        frame: "⠙",
        label: "thinking",
        elapsed: None,
        progress: None,
    });

    let drawn = rows(&sink).join("\n");
    assert!(
        !drawn.contains("ctrl+u clear"),
        "the hint belongs on the input box only:\n{drawn}"
    );
}

#[test]
fn a_box_too_narrow_for_the_hint_still_renders() {
    let (mut renderer, sink) = off_screen(14, 8, ReplTheme::default());
    renderer.render_input(&view("hi", 2));

    for row in rows(&sink) {
        assert!(
            row.chars().count() <= 14,
            "the hint pushed row {row:?} past the surface width"
        );
    }
}
