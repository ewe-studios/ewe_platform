//! Tests for the input-box layout engine.
//!
//! This is the logic the whole rendered box depends on: which row each piece of
//! the buffer lands on, and where the caret goes once the text has wrapped.
//! It is deliberately terminal-free so it can be asserted on directly.

use foundation_repl::{layout_input, visible_window, InputView};

/// Build a view with the default-ish prompts used across these tests.
fn view(buffer: &str, cursor: usize) -> InputView<'_> {
    InputView {
        prompt: "| ",
        continuation: "|... ",
        buffer,
        cursor,
    }
}

/// Rendered rows as `gutter + text`, for comparing whole layouts at a glance.
fn rendered(view: &InputView<'_>, width: usize) -> Vec<String> {
    layout_input(view, width)
        .rows
        .iter()
        .map(|row| format!("{}{}", row.gutter, row.text))
        .collect()
}

// ── The prompt survives, which is the bug this replaced ─────────────────

#[test]
fn empty_buffer_still_draws_the_prompt() {
    let view = view("", 0);
    let layout = layout_input(&view, 40);

    assert_eq!(layout.rows.len(), 1, "an empty buffer still needs a row");
    assert_eq!(layout.rows[0].gutter, "| ");
    assert_eq!(layout.rows[0].text, "");
    assert!(layout.rows[0].is_prompt);
    assert_eq!(
        (layout.cursor_row, layout.cursor_col),
        (0, 2),
        "the caret sits just after the prompt, not at column zero"
    );
}

#[test]
fn typing_keeps_the_prompt_in_front_of_the_text() {
    let view = view("hello", 5);
    let layout = layout_input(&view, 40);

    assert_eq!(rendered(&view, 40), vec!["| hello"]);
    assert_eq!(layout.cursor_col, 7, "prompt width plus five characters");
}

// ── Wrapping ────────────────────────────────────────────────────────────

#[test]
fn text_wraps_at_the_inner_width_and_aligns_under_the_prompt() {
    // Width 10, prompt "| " is 2 wide, so 8 text columns per row.
    let view = view("abcdefghijkl", 12);
    let layout = layout_input(&view, 10);

    assert_eq!(rendered(&view, 10), vec!["| abcdefgh", "  ijkl"]);
    assert!(layout.rows[0].is_prompt);
    assert!(
        !layout.rows[1].is_prompt,
        "a wrapped row continues a line, it does not start one"
    );
    assert_eq!((layout.cursor_row, layout.cursor_col), (1, 6));
}

#[test]
fn a_caret_at_the_wrap_boundary_moves_to_the_next_row() {
    // Exactly 8 characters fills row one; the caret belongs at the start of
    // row two, where the next character will actually appear.
    let view = view("abcdefgh", 8);
    let layout = layout_input(&view, 10);

    assert_eq!(
        (layout.cursor_row, layout.cursor_col),
        (1, 2),
        "caret wraps with the text rather than sitting outside the box"
    );
    assert_eq!(layout.rows.len(), 2);
}

#[test]
fn wrapping_never_produces_a_row_wider_than_the_box() {
    let long = "x".repeat(500);
    let view = view(&long, long.len());

    for width in [3_usize, 8, 20, 81] {
        let layout = layout_input(&view, width);
        for row in &layout.rows {
            let rendered = format!("{}{}", row.gutter, row.text);
            assert!(
                rendered.chars().count() <= width,
                "row {rendered:?} exceeds width {width}"
            );
        }
        assert!(layout.cursor_col <= width);
    }
}

// ── Multiline input ─────────────────────────────────────────────────────

#[test]
fn newlines_start_rows_with_the_continuation_prompt() {
    let view = view("one\ntwo", 7);
    let layout = layout_input(&view, 40);

    assert_eq!(rendered(&view, 40), vec!["| one", "|... two"]);
    assert!(layout.rows[1].is_prompt);
    assert_eq!((layout.cursor_row, layout.cursor_col), (1, 8));
}

#[test]
fn a_trailing_newline_leaves_an_empty_row_for_the_caret() {
    let view = view("one\n", 4);
    let layout = layout_input(&view, 40);

    assert_eq!(layout.rows.len(), 2);
    assert_eq!(layout.rows[1].text, "");
    assert_eq!(
        (layout.cursor_row, layout.cursor_col),
        (1, 5),
        "caret waits at the start of the new line, after its prompt"
    );
}

#[test]
fn caret_can_sit_in_the_middle_of_an_earlier_line() {
    let view = view("one\ntwo", 1);
    let layout = layout_input(&view, 40);

    assert_eq!((layout.cursor_row, layout.cursor_col), (0, 3));
}

// ── Unicode ─────────────────────────────────────────────────────────────

#[test]
fn full_width_characters_take_two_columns() {
    // Width 10 minus a 2-column prompt leaves 8 columns, which is four CJK
    // characters rather than the eight a byte- or char-count would allow.
    let view = view("日本語です", 15);
    let layout = layout_input(&view, 10);

    assert_eq!(rendered(&view, 10), vec!["| 日本語で", "  す"]);
    assert_eq!(
        layout.cursor_col,
        2 + 2,
        "one full-width character into the second row"
    );
}

#[test]
fn a_caret_inside_a_multibyte_character_is_clamped_not_panicked() {
    // Byte 1 is in the middle of 'é'; it must clamp to the boundary below.
    let view = view("é", 1);
    let layout = layout_input(&view, 40);

    assert_eq!(layout.cursor_col, 2, "clamped back to before the character");
}

#[test]
fn a_caret_past_the_end_is_clamped_to_the_end() {
    let view = view("abc", 999);
    let layout = layout_input(&view, 40);

    assert_eq!((layout.cursor_row, layout.cursor_col), (0, 5));
}

// ── Degenerate widths ───────────────────────────────────────────────────

#[test]
fn a_width_of_zero_still_produces_a_renderable_row() {
    let view = view("abc", 3);
    let layout = layout_input(&view, 0);

    assert!(!layout.rows.is_empty());
    assert!(layout.cursor_col <= 1);
}

#[test]
fn a_prompt_wider_than_the_box_is_truncated_rather_than_overflowing_it() {
    let view = InputView {
        prompt: "a-very-long-prompt> ",
        continuation: "> ",
        buffer: "hi",
        cursor: 2,
    };
    let layout = layout_input(&view, 6);

    assert!(
        !layout.rows.is_empty(),
        "must not give up and render nothing"
    );
    for row in &layout.rows {
        let width = format!("{}{}", row.gutter, row.text).chars().count();
        assert!(
            width <= 6,
            "row {row} is {width} columns wide and would run through the border"
        );
    }
    assert!(
        !layout.rows[0].text.is_empty(),
        "at least one column must be left for what the user is typing"
    );
}

// ── The scrolling window ────────────────────────────────────────────────

#[test]
fn everything_is_shown_when_it_fits() {
    assert_eq!(visible_window(3, 0, 10), (0, 3));
    assert_eq!(visible_window(10, 9, 10), (0, 10));
}

#[test]
fn the_window_follows_the_caret_once_the_rows_overflow() {
    // 20 rows, room for 5. A caret at the top keeps the window at the top.
    assert_eq!(visible_window(20, 0, 5), (0, 5));
    // A caret at row 6 pulls the window down so the caret is its last row.
    assert_eq!(visible_window(20, 6, 5), (2, 5));
    // At the very bottom the window stops rather than running off the end.
    assert_eq!(visible_window(20, 19, 5), (15, 5));
}

#[test]
fn the_window_always_contains_the_caret() {
    for total in 1_usize..40 {
        for cursor in 0..total {
            for max in 1_usize..8 {
                let (start, count) = visible_window(total, cursor, max);
                assert!(
                    cursor >= start && cursor < start + count,
                    "caret {cursor} outside window {start}..{} (total {total}, max {max})",
                    start + count
                );
                assert!(start + count <= total, "window runs past the last row");
                assert!(count <= max.max(1));
            }
        }
    }
}

#[test]
fn a_zero_height_window_is_treated_as_one_row() {
    let (start, count) = visible_window(10, 4, 0);
    assert_eq!(count, 1);
    assert_eq!(start, 4);
}
