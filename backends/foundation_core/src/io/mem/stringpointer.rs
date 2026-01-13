#[derive(Clone, Debug)]
pub struct StringPointer<'a> {
    content: &'a str,
    pos: usize,
    peek_pos: usize,
}

impl<'a> StringPointer<'a> {
    #[must_use] 
    pub fn new(content: &'a str) -> Self {
        Self {
            content,
            pos: 0,
            peek_pos: 0,
        }
    }

    #[inline]
    #[must_use] 
    pub fn content(&self) -> &'a str {
        self.content
    }

    #[inline]
    #[must_use] 
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns the total length of the string being accumulated on.
    #[inline]
    #[must_use] 
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// `peek_rem_len` returns the remaining count of strings
    /// left from the current peeks's cursor.
    #[inline]
    #[must_use] 
    pub fn peek_rem_len(&self) -> usize {
        (self.content[self.peek_pos..]).len()
    }

    /// [`rem_len`] returns the remaining count of strings
    /// left from the current position's cursor
    /// regardless of where the peek cursor is at.
    #[inline]
    #[must_use] 
    pub fn rem_len(&self) -> usize {
        (self.content[self.pos..]).len()
    }

    /// resets resets the location of the cursors for both read and peek to 0.
    /// Basically moving them to the start position.
    #[inline]
    pub fn reset(&mut self) {
        self.reset_to(0);
    }

    /// `reset_to` lets you reset the position of the cursor for both
    /// position and peek to the to value.
    #[inline]
    pub fn reset_to(&mut self, to: usize) {
        self.pos = to;
        self.peek_pos = to;
    }

    /// skip will skip all the contents of the accumulator up to
    /// the current position of the peek cursor.
    #[inline]
    pub fn skip(&mut self) {
        self.pos = self.peek_pos;
    }

    /// peek pulls the next token at the current peek position
    /// cursor which will
    #[inline]
    pub fn peek(&mut self, by: usize) -> Option<&'a str> {
        self.peek_slice(by)
    }

    /// `peek_next` allows you to increment the peek cursor, moving
    /// the peek cursor forward by a mount of step and returns the next
    /// token string.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn peek_next_by(&mut self, by: usize) -> Option<&'a str> {
        if let Some(res) = self.peek_slice(by) {
            self.peek_pos = self.ensure_character_boundary_index(self.peek_pos + by);
            return Some(res);
        }
        None
    }

    /// scan returns the whole string slice currently at the points of where
    /// the main pos (position) cursor till the end.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn scan_remaining(&mut self) -> Option<&'a str> {
        Some(&self.content[self.pos..])
    }

    /// scan returns the whole string slice currently at the points of where
    /// the main pos (position) cursor and the peek cursor so you can
    /// pull the string right at the current range.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn scan(&mut self) -> Option<&'a str> {
        Some(&self.content[self.pos..self.peek_pos])
    }

    /// `peek_next` allows you to increment the peek cursor, moving
    /// the peek cursor forward by a step and returns the next
    /// token string.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn peek_next(&mut self) -> Option<&'a str> {
        if let Some(res) = self.peek_slice(1) {
            self.peek_pos = self.ensure_character_boundary_index(self.peek_pos + 1);
            return Some(res);
        }
        None
    }

    /// `unpeek_next` reverses the last forward move of the peek
    /// cursor by -1.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn unpeek_next(&mut self) -> Option<&'a str> {
        if let Some(res) = self.unpeek_slice(1) {
            return Some(res);
        }
        None
    }

    /// `unpeek_slice` lets you reverse the peek cursor position
    /// by a certain amount to reverse the forward movement.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    fn unpeek_slice(&mut self, by: usize) -> Option<&'a str> {
        if self.peek_pos == 0 {
            return None;
        }

        // unpeek only works when we are higher then current pos cursor.
        // it should have no effect when have not moved forward
        if self.peek_pos > self.pos {
            self.peek_pos = self.inverse_ensure_character_boundary_index(self.peek_pos - 1);
        }

        let new_peek_pos = self.ensure_character_boundary_index(self.peek_pos + by);
        Some(&self.content[self.peek_pos..new_peek_pos])
    }

    /// `ppeek_at` allows you to do a non-permant position cursor adjustment
    /// by taking the current position cursor index with an adjustment
    /// where we add the `from` (pos + from) to get the new
    /// position to start from and `to` is added (pos + from + to)
    /// the position to end at, if the total is more than the length of the string
    /// then its adjusted to be the string last index for the slice.
    ///
    /// It's a nice way to get to see whats at a given position without changing
    /// the current location of the peek cursor.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn ppeek_at(&mut self, from: usize, to: usize) -> Option<&'a str> {
        let new_peek_pos = self.ensure_character_boundary_index(self.pos + from);
        let mut until_pos = if new_peek_pos + to > self.content.len() {
            self.content.len()
        } else {
            new_peek_pos + to
        };

        if new_peek_pos > self.content.len() {
            return None;
        }

        until_pos = self.ensure_character_boundary_index(until_pos);

        Some(&self.content[new_peek_pos..until_pos])
    }

    /// `vpeek_at` allows you to do a non-permant peek cursor adjustment
    /// by taking the current peek cursor position with an adjustment
    /// where we add the `from` (`peek_cursor` + from) to get the new
    /// position to start from and `to` is added (`peek_cursor` + from + to)
    /// the position to end at, if the total is more than the length of the string
    /// then its adjusted to be the string last index for the slice.
    ///
    /// It's a nice way to get to see whats at a given position without changing
    /// the current location of the peek cursor.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn vpeek_at(&mut self, from: usize, to: usize) -> Option<&'a str> {
        let mut new_peek_pos = self.peek_pos + from;
        let mut until_pos = if new_peek_pos + to > self.content.len() {
            self.content.len()
        } else {
            new_peek_pos + to
        };

        if new_peek_pos > self.content.len() {
            return None;
        }

        // ensure we are always at the char boundary
        new_peek_pos = self.ensure_character_boundary_index(new_peek_pos);
        until_pos = self.ensure_character_boundary_index(until_pos);

        tracing::debug!(
            "Check if we are out of char boundary: start: {}:{}, end: {}:{}",
            new_peek_pos,
            self.content.is_char_boundary(new_peek_pos),
            until_pos,
            self.content.is_char_boundary(until_pos)
        );
        Some(&self.content[new_peek_pos..until_pos])
    }

    #[inline]
    fn inverse_ensure_character_boundary_index(&self, current_index: usize) -> usize {
        let mut next_index = current_index;
        // ensure we are always at the char boundary
        loop {
            if !self.content.is_char_boundary(next_index) {
                next_index -= 1;
                continue;
            }
            break;
        }
        next_index
    }

    #[inline]
    fn ensure_character_boundary_index(&self, current_index: usize) -> usize {
        let mut next_index = current_index;
        // ensure we are always at the char boundary
        loop {
            if !self.content.is_char_boundary(next_index) {
                next_index += 1;
                continue;
            }
            break;
        }
        next_index
    }

    /// `peek_slice` allows you to peek forward by an amount
    /// from the current peek cursor position.
    ///
    /// If we've exhausted the total string slice left or are trying to
    /// take more than available text length then we return None
    /// which can indicate no more text for processing.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    fn peek_slice(&mut self, by: usize) -> Option<&'a str> {
        if self.peek_pos + by > self.content.len() {
            return None;
        }
        let from = self.ensure_character_boundary_index(self.peek_pos);
        let until_pos = self.ensure_character_boundary_index(self.peek_pos + by);
        Some(&self.content[from..until_pos])
    }

    /// take returns the total string slice from the
    /// actual accumulators position cursor til the current
    /// peek cursor position with adjustment on `by` amount i.e
    /// str[`position_cursor..(peek_cursor` + `by_value`)].
    ///
    /// Allow you to collect the whole slice of strings that have been
    /// checked and peeked through.
    ///
    /// If we've exhausted the total string slice left or are trying to
    /// take more than available text length then we return None
    /// which can indicate no more text for processing.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn take_with_amount(&mut self, by: usize) -> Option<(&'a str, (usize, usize))> {
        if self.peek_pos + by > self.content.len() {
            return None;
        }

        let mut until_pos = if self.peek_pos + by > self.content.len() {
            self.content.len()
        } else {
            self.peek_pos + by
        };

        if self.pos >= self.content.len() {
            return None;
        }

        let org_from = self.pos;
        let org_until = until_pos;
        let from = self.ensure_character_boundary_index(self.pos);
        until_pos = self.ensure_character_boundary_index(until_pos);

        tracing::debug!(
            "take_with_amount: possibly shift in positions: org:({}, {}) then end in final:({},{})",
            org_from,
            org_until,
            from,
            until_pos,
        );

        let position = (from, until_pos);

        tracing::debug!(
            "take_with_amount: content len: {} with pos: {}, peek_pos: {}, by: {}, until: {}",
            self.content.len(),
            self.pos,
            self.peek_pos,
            by,
            until_pos,
        );

        let content_slice = &self.content[from..until_pos];

        tracing::debug!(
            "take_with_amount: sliced worked from: {}, by: {}, till loc: {} with text: '{}'",
            self.pos,
            by,
            until_pos,
            content_slice,
        );

        let res = Some((content_slice, position));
        self.pos = self.peek_pos;
        res
    }

    /// `take_positional` returns the total string slice from the
    /// actual accumulators position cursor til the current
    /// peek cursor position i.e str[`position_cursor...peek_cursor`].
    /// Allow you to collect the whole slice of strings that have been
    /// checked and peeked through.
    #[inline]
    pub fn take_positional(&mut self) -> Option<(&'a str, (usize, usize))> {
        self.take_with_amount(0)
    }

    /// take returns the total string slice from the
    /// actual accumulators position cursor til the current
    /// peek cursor position i.e str[`position_cursor...peek_cursor`].
    /// Allow you to collect the whole slice of strings that have been
    /// checked and peeked through.
    #[inline]
    pub fn take(&mut self) -> Option<&'a str> {
        match self.take_with_amount(0) {
            Some((text, _)) => Some(text),
            None => None,
        }
    }
}

#[cfg(test)]
mod accumulator_tests {
    use super::*;

    #[test]
    fn test_can_use_accumulator_to_peek_next_character() {
        let mut accumulator = StringPointer::new("hello");
        assert_eq!("h", accumulator.peek(1).unwrap());
        assert_eq!("h", accumulator.peek(1).unwrap());
    }

    #[test]
    fn test_can_use_accumulator_to_peek_two_characters_away() {
        let mut accumulator = StringPointer::new("hello");
        assert_eq!("he", accumulator.peek(2).unwrap());
    }

    #[test]
    fn test_can_virtual_peek_ahead_without_changing_peek_cursor() {
        let mut accumulator = StringPointer::new("hello");

        assert_eq!("h", accumulator.peek_next().unwrap());
        assert_eq!("e", accumulator.peek_next().unwrap());

        assert_eq!("llo", accumulator.vpeek_at(0, 3).unwrap()); // from peek cursor till 3 ahead
        assert_eq!("lo", accumulator.vpeek_at(1, 3).unwrap()); // from 1 character ahead of peek cursor

        assert_eq!("l", accumulator.peek_next().unwrap());
        assert_eq!("l", accumulator.peek_next().unwrap());
        assert_eq!("o", accumulator.peek_next().unwrap());
        assert_eq!(None, accumulator.peek_next());
    }

    #[test]
    fn test_can_peek_next_to_accumulate_more_seen_text() {
        let mut accumulator = StringPointer::new("hello");

        assert_eq!("h", accumulator.peek_next().unwrap());
        assert_eq!("e", accumulator.peek_next().unwrap());
        assert_eq!("l", accumulator.peek_next().unwrap());
        assert_eq!("l", accumulator.peek_next().unwrap());
        assert_eq!("o", accumulator.peek_next().unwrap());

        assert_eq!(None, accumulator.peek_next());
    }

    #[test]
    fn test_can_peek_next_and_take_text_then_continue_peeking() {
        let mut accumulator = StringPointer::new("hello");

        assert_eq!(5, accumulator.len());

        assert_eq!("h", accumulator.peek_next().unwrap());
        assert_eq!("e", accumulator.peek_next().unwrap());
        assert_eq!("l", accumulator.peek_next().unwrap());

        assert_eq!(5, accumulator.len());
        assert_eq!(5, accumulator.rem_len());
        assert_eq!(2, accumulator.peek_rem_len());

        assert_eq!("hel", accumulator.take().unwrap());

        assert_eq!(2, accumulator.rem_len());
        assert_eq!(2, accumulator.peek_rem_len());

        assert_eq!("l", accumulator.peek_next().unwrap());
        assert_eq!("o", accumulator.peek_next().unwrap());

        assert_eq!(2, accumulator.rem_len());
        assert_eq!(0, accumulator.peek_rem_len());

        assert_eq!(None, accumulator.peek_next());

        assert_eq!(2, accumulator.rem_len());
        assert_eq!(0, accumulator.peek_rem_len());

        assert_eq!("lo", accumulator.take().unwrap());

        assert_eq!(0, accumulator.rem_len());

        assert_eq!(None, accumulator.peek_next());
    }
}
