//! Turns a raw input buffer into the rows of text that go inside the input box.
//!
//! WHY: the renderer has to know two things that cannot be read back out of a
//! terminal — which visual row each piece of the buffer landed on, and where
//! the caret sits once the text has wrapped. Computing both here keeps that
//! logic away from any terminal, so it can be unit tested directly.
//!
//! WHAT: [`layout_input`] wraps the buffer to a given inner width, attaches the
//! prompt (or continuation prompt) to the front of each logical line, and
//! reports the caret's row and column. [`visible_window`] then picks which of
//! those rows to show when there are more than the box is allowed to grow to.
//!
//! HOW: wrapping is a hard wrap at display-width boundaries rather than a word
//! wrap, so every byte of the buffer maps to exactly one cell and the caret can
//! be placed by measuring the text in front of it. Widths come from
//! `unicode-width`, so full-width characters occupy the two cells they actually
//! take up on screen.

use core::fmt;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// The state of the input line at the moment it is drawn.
///
/// WHY: rendering needs the text, the prompts and the caret together — passing
/// them as one value keeps the display trait from growing a parameter per part.
///
/// WHAT: the buffer being edited plus the caret's byte offset into it, and the
/// prompt strings to put in front of the first and subsequent lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputView<'a> {
    /// Prompt drawn in front of the first line.
    pub prompt: &'a str,
    /// Prompt drawn in front of every line after a newline.
    pub continuation: &'a str,
    /// The text currently being edited.
    pub buffer: &'a str,
    /// Caret position as a byte offset into `buffer`.
    pub cursor: usize,
}

impl fmt::Display for InputView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InputView({} bytes, cursor {})",
            self.buffer.len(),
            self.cursor
        )
    }
}

/// One visual row inside the input box.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputRow {
    /// Prompt, continuation prompt, or the blanks that keep a wrapped
    /// continuation aligned under the prompt above it.
    pub gutter: String,
    /// The slice of the buffer that landed on this row.
    pub text: String,
    /// True when `gutter` is a real prompt rather than alignment blanks.
    pub is_prompt: bool,
}

impl fmt::Display for InputRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.gutter, self.text)
    }
}

/// Every row of the input box, plus where the caret sits among them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputLayout {
    /// The rows, top to bottom. Always at least one, even for empty input.
    pub rows: Vec<InputRow>,
    /// Index into `rows` of the row holding the caret.
    pub cursor_row: usize,
    /// Caret's display column, measured from the start of the gutter.
    pub cursor_col: usize,
}

impl fmt::Display for InputLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "InputLayout({} rows, caret at {}:{})",
            self.rows.len(),
            self.cursor_row,
            self.cursor_col
        )
    }
}

/// Lay the buffer out into rows no wider than `width` display columns.
///
/// WHY: the caller renders into a fixed-width box and needs both the wrapped
/// text and a caret position that agrees with it.
///
/// WHAT: returns one [`InputRow`] per visual row, with the caret's row and
/// column. The caret column is measured from column zero of the row, so it
/// already accounts for the gutter.
///
/// HOW: each logical line (split on `\n`) is hard-wrapped at `width` minus its
/// gutter width; the caret is located by measuring the display width of the
/// text in front of it on its row. A caret sitting exactly at the wrap boundary
/// moves to the start of the following row, and a trailing row is added when
/// that boundary is also the end of the text.
///
/// `width` is clamped to at least one column, and at least one text column is
/// always left after the gutter, so a prompt wider than the box still renders.
///
/// # Panics
/// Never panics. A `cursor` past the end of `buffer`, or one landing inside a
/// multi-byte character, is clamped to the nearest valid boundary at or below it.
#[must_use]
pub fn layout_input(view: &InputView<'_>, width: usize) -> InputLayout {
    let width = width.max(1);
    let cursor = clamp_to_char_boundary(view.buffer, view.cursor);

    let mut rows: Vec<InputRow> = Vec::new();
    let mut cursor_row = 0usize;
    let mut cursor_col = 0usize;
    let mut cursor_placed = false;
    let mut line_start = 0usize;

    for (line_index, line) in view.buffer.split('\n').enumerate() {
        let gutter = if line_index == 0 {
            view.prompt
        } else {
            view.continuation
        };
        let gutter_width = gutter.width().min(width.saturating_sub(1));
        let text_width = width.saturating_sub(gutter_width).max(1);
        let chunks = wrap_line(line, text_width);
        let last_chunk = chunks.len() - 1;

        for (chunk_index, (chunk_start, chunk)) in chunks.into_iter().enumerate() {
            let absolute_start = line_start + chunk_start;
            let absolute_end = absolute_start + chunk.len();

            rows.push(InputRow {
                gutter: if chunk_index == 0 {
                    // Truncated, not just measured: a prompt longer than the
                    // box would otherwise be drawn straight through the border.
                    truncate_to_width(gutter, gutter_width)
                } else {
                    " ".repeat(gutter_width)
                },
                text: chunk.to_string(),
                is_prompt: chunk_index == 0,
            });

            // A caret exactly at a chunk's end belongs to the next chunk's
            // start, unless this is the final chunk of the logical line.
            let owns_cursor = cursor >= absolute_start
                && (cursor < absolute_end || (cursor == absolute_end && chunk_index == last_chunk));

            if !cursor_placed && owns_cursor {
                cursor_row = rows.len() - 1;
                cursor_col = gutter_width + view.buffer[absolute_start..cursor].width();
                cursor_placed = true;
            }
        }

        // `+ 1` steps over the '\n' that `split` consumed.
        line_start += line.len() + 1;
    }

    // A caret that filled the final column has nowhere to sit on this row, so
    // it wraps onto a fresh one — the same thing the text would have done.
    if cursor_col >= width {
        let gutter_width = rows
            .get(cursor_row)
            .map_or(0, |row| row.gutter.width())
            .min(width.saturating_sub(1));
        rows.insert(
            cursor_row + 1,
            InputRow {
                gutter: " ".repeat(gutter_width),
                text: String::new(),
                is_prompt: false,
            },
        );
        cursor_row += 1;
        cursor_col = gutter_width;
    }

    InputLayout {
        rows,
        cursor_row,
        cursor_col,
    }
}

/// Choose which rows to show when the box cannot hold them all.
///
/// WHY: the box has a height ceiling, but the caret must stay on screen no
/// matter how far down the text it has moved.
///
/// WHAT: returns `(first_row, row_count)` — a window of at most `max_rows` rows
/// that is guaranteed to contain `cursor_row`.
///
/// HOW: the window is anchored to the top until the caret would fall off the
/// bottom, then follows the caret, and is always pulled back so it ends at the
/// last row rather than leaving blank space below.
///
/// # Panics
/// Never panics. A `max_rows` of zero is treated as one.
#[must_use]
pub fn visible_window(total_rows: usize, cursor_row: usize, max_rows: usize) -> (usize, usize) {
    let max_rows = max_rows.max(1);
    if total_rows <= max_rows {
        return (0, total_rows);
    }

    let last_start = total_rows - max_rows;
    let start = cursor_row.saturating_sub(max_rows - 1).min(last_start);
    (start, max_rows)
}

/// Word-wrap `text` into rows at most `width` display columns wide.
///
/// WHY: input is hard-wrapped so every byte maps to a cell and the caret can be
/// placed — but output is prose, and breaking prose mid-word ("scrollbac /
/// k") is just hard to read.
///
/// WHAT: rows broken at spaces where possible, at character boundaries when a
/// single word is wider than the row. Explicit newlines always start a new row.
///
/// # Panics
/// Never panics. A `width` of zero is treated as one column.
#[must_use]
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
    wrap_ranges(text, width)
        .into_iter()
        .map(|(start, end)| text[start..end].trim_end().to_string())
        .collect()
}

/// Split streamed text into rows that can no longer change, plus the live tail.
///
/// WHY: streamed output has to reach the terminal's scrollback to survive
/// scrolling and selection, but a row still being written to would be committed
/// wrong.
///
/// WHAT: returns `(finished rows, remaining text)`. The remainder is returned
/// as raw text rather than a rendered row, so the spacing at the join is not
/// lost and the next fragment appends to it correctly.
///
/// HOW: wrapping is greedy and left to right, so appending text can only ever
/// affect the final row — every row before it is final the moment it exists.
///
/// # Panics
/// Never panics. A `width` of zero is treated as one column.
#[must_use]
pub fn split_committed(text: &str, width: usize) -> (Vec<String>, String) {
    let mut ranges = wrap_ranges(text, width);

    let Some((tail_start, _)) = ranges.pop() else {
        return (Vec::new(), String::new());
    };

    let committed = ranges
        .into_iter()
        .map(|(start, end)| text[start..end].trim_end().to_string())
        .collect();

    (committed, text[tail_start..].to_string())
}

/// Byte ranges of each word-wrapped row within `text`.
///
/// Ranges never include the newline that ended a row, so re-joining them is
/// unambiguous.
fn wrap_ranges(text: &str, width: usize) -> Vec<(usize, usize)> {
    let width = width.max(1);
    let mut ranges = Vec::new();
    let mut line_base = 0usize;

    for line in text.split('\n') {
        let mut position = 0usize;
        let mut row_start = 0usize;
        let mut used = 0usize;

        // `split_inclusive` keeps the space with the word before it, so the
        // tokens tile the line exactly and `position` stays in step.
        for token in line.split_inclusive(' ') {
            let word_width = token.trim_end_matches(' ').width();

            // The word does not fit beside what is already on this row.
            if used > 0 && used + word_width > width {
                ranges.push((line_base + row_start, line_base + position));
                row_start = position;
                used = 0;
            }

            // A word wider than a whole row has to be split mid-word; there is
            // nowhere else to break it.
            if used == 0 && word_width > width {
                let mut chunk_start = position;
                let mut chunk_width = 0usize;

                for (offset, character) in token.char_indices() {
                    let character_width = character.width().unwrap_or(0);
                    if chunk_width + character_width > width
                        && position + offset > chunk_start
                    {
                        ranges.push((line_base + chunk_start, line_base + position + offset));
                        chunk_start = position + offset;
                        chunk_width = 0;
                    }
                    chunk_width += character_width;
                }

                row_start = chunk_start;
                used = chunk_width;
                position += token.len();
                continue;
            }

            used += token.width();
            position += token.len();
        }

        ranges.push((line_base + row_start, line_base + line.len()));
        // `+ 1` steps over the '\n' that `split` consumed.
        line_base += line.len() + 1;
    }

    ranges
}

/// Hard-wrap one logical line into chunks at most `width` columns wide.
///
/// Returns `(byte offset within `line`, chunk)` pairs, always at least one pair
/// so that an empty line still produces a row to draw the caret on.
fn wrap_line(line: &str, width: usize) -> Vec<(usize, &str)> {
    if line.is_empty() {
        return vec![(0, "")];
    }

    let mut chunks = Vec::new();
    let mut start = 0usize;
    let mut used = 0usize;

    for (index, character) in line.char_indices() {
        let character_width = character.width().unwrap_or(0);
        // `index > start` keeps a character wider than the whole row from
        // producing an empty chunk and looping forever.
        if used + character_width > width && index > start {
            chunks.push((start, &line[start..index]));
            start = index;
            used = 0;
        }
        used += character_width;
    }
    chunks.push((start, &line[start..]));

    chunks
}

/// Cut `text` down to at most `width` display columns.
///
/// A character that would straddle the limit is dropped rather than half-drawn,
/// so the result is never wider than asked for.
fn truncate_to_width(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }

    let mut used = 0usize;
    let mut end = 0usize;
    for (index, character) in text.char_indices() {
        let character_width = character.width().unwrap_or(0);
        if used + character_width > width {
            break;
        }
        used += character_width;
        end = index + character.len_utf8();
    }

    text[..end].to_string()
}

/// Clamp a byte offset to the nearest character boundary at or below it,
/// and to the end of `text` if it points past the end.
fn clamp_to_char_boundary(text: &str, offset: usize) -> usize {
    if offset >= text.len() {
        return text.len();
    }
    let mut offset = offset;
    while offset > 0 && !text.is_char_boundary(offset) {
        offset -= 1;
    }
    offset
}
