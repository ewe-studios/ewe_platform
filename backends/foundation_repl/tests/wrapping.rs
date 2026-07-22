//! Tests for output word-wrapping and the streaming commit rule.
//!
//! Streaming correctness rests on one property: a row handed to the terminal's
//! scrollback can never be changed again. These tests assert that property
//! directly, by replaying a stream one fragment at a time and comparing the
//! result against wrapping the whole text at once.

use foundation_repl::{split_committed, wrap_text};

// ── Word wrapping ───────────────────────────────────────────────────────

#[test]
fn short_text_is_one_row() {
    assert_eq!(wrap_text("hello world", 40), vec!["hello world"]);
}

#[test]
fn wrapping_breaks_at_spaces_not_mid_word() {
    let rows = wrap_text("the quick brown fox jumps", 10);

    assert_eq!(rows, vec!["the quick", "brown fox", "jumps"]);
    for row in &rows {
        assert!(row.chars().count() <= 10, "row {row:?} is too wide");
    }
}

#[test]
fn a_word_longer_than_the_row_is_split_because_there_is_nowhere_else_to_break() {
    let rows = wrap_text("supercalifragilistic", 8);

    assert_eq!(rows, vec!["supercal", "ifragili", "stic"]);
}

#[test]
fn a_long_word_after_short_ones_starts_its_own_row() {
    let rows = wrap_text("hi supercalifragilistic", 8);

    assert_eq!(rows.first().map(String::as_str), Some("hi"));
    for row in &rows {
        assert!(row.chars().count() <= 8, "row {row:?} is too wide");
    }
}

#[test]
fn explicit_newlines_always_start_a_row() {
    assert_eq!(wrap_text("one\ntwo", 40), vec!["one", "two"]);
    assert_eq!(wrap_text("one\n\ntwo", 40), vec!["one", "", "two"]);
}

#[test]
fn empty_text_is_a_single_empty_row() {
    assert_eq!(wrap_text("", 40), vec![""]);
}

#[test]
fn full_width_characters_are_measured_in_columns_not_characters() {
    // Four CJK characters is eight columns, so only four fit in a row of eight.
    let rows = wrap_text("日本語です日本語です", 8);

    for row in &rows {
        let columns: usize = row
            .chars()
            .map(unicode_width_of)
            .sum();
        assert!(columns <= 8, "row {row:?} is {columns} columns");
    }
    assert!(rows.len() >= 2);
}

/// Column width of a character, matching what the wrapper uses.
fn unicode_width_of(character: char) -> usize {
    match character {
        '\u{1100}'..='\u{115F}' | '\u{2E80}'..='\u{A4CF}' | '\u{FF00}'..='\u{FF60}' => 2,
        _ => 1,
    }
}

#[test]
fn trailing_spaces_do_not_push_a_row_over_the_limit() {
    // "12345678" plus a space is nine columns, but the space is trimmed away.
    let rows = wrap_text("12345678 x", 8);
    assert_eq!(rows, vec!["12345678", "x"]);
}

// ── The streaming commit rule ───────────────────────────────────────────

#[test]
fn nothing_is_committed_until_a_row_is_full() {
    let (committed, tail) = split_committed("hello", 40);

    assert!(committed.is_empty(), "one short row is still being written to");
    assert_eq!(tail, "hello");
}

#[test]
fn a_finished_row_is_committed_and_the_rest_stays_live() {
    let (committed, tail) = split_committed("the quick brown fox", 10);

    assert_eq!(committed, vec!["the quick"]);
    assert_eq!(tail, "brown fox");
}

#[test]
fn the_tail_keeps_the_spacing_that_joins_it_to_the_next_fragment() {
    // The tail is raw text, not a trimmed row: dropping the trailing space here
    // would glue the next word onto the previous one.
    let (_, tail) = split_committed("aaaa bbbb cc ", 10);

    assert!(
        tail.ends_with(' '),
        "tail {tail:?} lost the space before the next fragment"
    );
}

#[test]
fn a_newline_finishes_its_row_immediately() {
    let (committed, tail) = split_committed("done\n", 40);

    assert_eq!(committed, vec!["done"]);
    assert_eq!(tail, "", "the next fragment starts a fresh row");
}

#[test]
fn committed_rows_never_change_as_more_text_arrives() {
    // Replay a stream fragment by fragment, checking after every push that
    // everything already committed is still a prefix of the final wrapping.
    let full = "Streaming works by committing each row as soon as it can no \
                longer change, so a long reply scrolls like ordinary output \
                instead of being trapped in a redrawn region.";
    let width = 24;

    for fragment_size in [1_usize, 3, 7, 20] {
        let mut tail = String::new();
        let mut emitted: Vec<String> = Vec::new();

        let bytes: Vec<char> = full.chars().collect();
        for chunk in bytes.chunks(fragment_size) {
            tail.push_str(&chunk.iter().collect::<String>());
            let (committed, remainder) = split_committed(&tail, width);
            emitted.extend(committed);
            tail = remainder;
        }
        // End of stream: whatever is left becomes the final row.
        if !tail.trim_end().is_empty() {
            emitted.push(tail.trim_end().to_string());
        }

        let at_once = wrap_text(full, width);
        assert_eq!(
            emitted, at_once,
            "streaming in {fragment_size}-char fragments produced different \
             rows than wrapping the whole text at once"
        );

        for row in &emitted {
            assert!(
                row.chars().count() <= width,
                "streamed row {row:?} is wider than the output area"
            );
        }
    }
}

#[test]
fn streaming_survives_newlines_mid_stream() {
    let full = "first line\nsecond line is a bit longer\n\nfourth";
    let width = 16;

    let mut tail = String::new();
    let mut emitted: Vec<String> = Vec::new();
    for character in full.chars() {
        tail.push(character);
        let (committed, remainder) = split_committed(&tail, width);
        emitted.extend(committed);
        tail = remainder;
    }
    emitted.push(tail.trim_end().to_string());

    assert_eq!(emitted, wrap_text(full, width));
}

#[test]
fn a_zero_width_output_area_does_not_hang_or_panic() {
    let (committed, tail) = split_committed("some text here", 0);

    assert!(!committed.is_empty());
    assert!(tail.chars().count() <= 1);
}
