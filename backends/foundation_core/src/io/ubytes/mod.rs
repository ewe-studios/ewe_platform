/// Bytes provides an abstraction of a slice of bytes (u8)
/// letting you advance, track and safely move through a slice
/// of bytes easily.
///
/// This is taking from the [httpparse](https://github.com/seanmonstar/httparse) crate.
pub struct Bytes<'a> {
    start: *const u8,
    end: *const u8,
    /// INVARIANT: start <= cursor && cursor <= end
    cursor: *const u8,
    phantom: core::marker::PhantomData<&'a ()>,
}

#[allow(missing_docs)]
impl<'a> Bytes<'a> {
    #[inline]
    pub fn new(slice: &'a [u8]) -> Bytes<'a> {
        let start = slice.as_ptr();
        // SAFETY: obtain pointer to slice end; start points to slice start.
        let end = unsafe { start.add(slice.len()) };
        let cursor = start;
        Bytes {
            start,
            end,
            cursor,
            phantom: core::marker::PhantomData,
        }
    }

    #[inline]
    pub fn pos(&self) -> usize {
        self.cursor as usize - self.start as usize
    }

    #[inline]
    pub fn peek(&self) -> Option<u8> {
        if self.cursor < self.end {
            // SAFETY:  bounds checked
            Some(unsafe { *self.cursor })
        } else {
            None
        }
    }

    #[inline]
    pub fn peek_ahead(&self, n: usize) -> Option<u8> {
        // SAFETY: obtain a potentially OOB pointer that is later compared against the `self.end`
        // pointer.
        let ptr = self.cursor.wrapping_add(n);
        if ptr < self.end {
            // SAFETY: bounds checked pointer dereference is safe
            Some(unsafe { *ptr })
        } else {
            None
        }
    }

    #[inline]
    pub fn peek_n<'b: 'a, U: TryFrom<&'a [u8]>>(&'b self, n: usize) -> Option<U> {
        // TODO: once we bump MSRC, use const generics to allow only [u8; N] reads
        // TODO: drop `n` arg in favour of const
        // let n = core::mem::size_of::<U>();
        self.as_ref().get(..n)?.try_into().ok()
    }

    /// Advance by 1, equivalent to calling `advance(1)`.
    ///
    /// # Safety
    ///
    /// Caller must ensure that Bytes hasn't been advanced/bumped by more than [`Bytes::len()`].
    #[inline]
    pub unsafe fn bump(&mut self) {
        self.advance(1)
    }

    /// Advance cursor by `n`
    ///
    /// # Safety
    ///
    /// Caller must ensure that Bytes hasn't been advanced/bumped by more than [`Bytes::len()`].
    #[inline]
    pub unsafe fn advance(&mut self, n: usize) {
        self.cursor = self.cursor.add(n);
        debug_assert!(self.cursor <= self.end, "overflow");
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.end as usize - self.cursor as usize
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn slice(&mut self) -> &'a [u8] {
        // SAFETY: not moving position at all, so it's safe
        let slice = unsafe { slice_from_ptr_range(self.start, self.cursor) };
        self.commit();
        slice
    }

    // TODO: this is an anti-pattern, should be removed
    /// Deprecated. Do not use!
    /// # Safety
    ///
    /// Caller must ensure that `skip` is at most the number of advances (i.e., `bytes.advance(3)`
    /// implies a skip of at most 3).
    #[inline]
    pub unsafe fn slice_skip(&mut self, skip: usize) -> &'a [u8] {
        debug_assert!(skip <= self.cursor.offset_from(self.start) as usize);
        let head = slice_from_ptr_range(self.start, self.cursor.sub(skip));
        self.commit();
        head
    }

    #[inline]
    pub fn commit(&mut self) {
        self.start = self.cursor
    }

    /// # Safety
    ///
    /// see [`Bytes::advance`] safety comment.
    #[inline]
    pub unsafe fn advance_and_commit(&mut self, n: usize) {
        self.advance(n);
        self.commit();
    }

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.cursor
    }

    #[inline]
    pub fn start(&self) -> *const u8 {
        self.start
    }

    #[inline]
    pub fn end(&self) -> *const u8 {
        self.end
    }

    /// # Safety
    ///
    /// Must ensure invariant `bytes.start() <= ptr && ptr <= bytes.end()`.
    #[inline]
    pub unsafe fn set_cursor(&mut self, ptr: *const u8) {
        debug_assert!(ptr >= self.start);
        debug_assert!(ptr <= self.end);
        self.cursor = ptr;
    }
}

impl AsRef<[u8]> for Bytes<'_> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        // SAFETY: not moving position at all, so it's safe
        unsafe { slice_from_ptr_range(self.cursor, self.end) }
    }
}

/// # Safety
///
/// Must ensure start and end point to the same memory object to uphold memory safety.
#[inline]
unsafe fn slice_from_ptr_range<'a>(start: *const u8, end: *const u8) -> &'a [u8] {
    debug_assert!(start <= end);
    core::slice::from_raw_parts(start, end as usize - start as usize)
}

impl Iterator for Bytes<'_> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        if self.cursor < self.end {
            // SAFETY: bounds checked dereference
            unsafe {
                let b = *self.cursor;
                self.bump();
                Some(b)
            }
        } else {
            None
        }
    }
}

#[derive(Clone, Debug)]
pub struct BytesPointer<'a> {
    content: &'a [u8],
    pos: usize,
    peek_pos: usize,
}

impl<'a> BytesPointer<'a> {
    pub fn new(content: &'a [u8]) -> Self {
        Self {
            content,
            pos: 0,
            peek_pos: 0,
        }
    }

    #[inline]
    pub fn content(&self) -> &'a [u8] {
        self.content
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    /// Returns the total length of the string being accumulated on.
    #[inline]
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// peek_rem_len returns the remaining count of strings
    /// left from the current peeks's cursor.
    #[inline]
    pub fn peek_rem_len(&self) -> usize {
        (self.content[self.peek_pos..]).len()
    }

    /// rem_len returns the remaining count of strings
    /// left from the current position's cursor
    /// regardless of where the peek cursor is at.
    #[inline]
    pub fn rem_len(&self) -> usize {
        (self.content[self.pos..]).len()
    }

    /// resets resets the location of the cursors for both read and peek to 0.
    /// Basically moving them to the start position.
    #[inline]
    pub fn reset(&mut self) {
        self.reset_to(0)
    }

    /// reset_to lets you reset the position of the cursor for both
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
        self.pos = self.peek_pos
    }

    /// peek pulls the next token at the current peek position
    /// cursor which will
    #[inline]
    pub fn peek(&mut self, by: usize) -> Option<&'a [u8]> {
        self.peek_slice(by)
    }

    /// peek_next allows you to increment the peek cursor, moving
    /// the peek cursor forward by a step and returns the next
    /// token string.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn peek_next(&mut self) -> Option<&'a [u8]> {
        if let Some(res) = self.peek_slice(1) {
            self.peek_pos += 1;
            return Some(res);
        }
        None
    }

    /// peek_next allows you to increment the peek cursor, moving
    /// the peek cursor forward by a mount of step and returns the next
    /// token string.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn peek_next_by(&mut self, by: usize) -> Option<&'a [u8]> {
        if let Some(res) = self.peek_slice(by) {
            self.peek_pos += by;
            return Some(res);
        }
        None
    }

    /// unpeek_next reverses the last forward move of the peek
    /// cursor by -1.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn unpeek_next(&mut self) -> Option<&'a [u8]> {
        if let Some(res) = self.unpeek_slice(1) {
            return Some(res);
        }
        None
    }

    /// unpeek_slice lets you reverse the peek cursor position
    /// by a certain amount to reverse the forward movement.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    fn unpeek_slice(&mut self, by: usize) -> Option<&'a [u8]> {
        if self.peek_pos == 0 {
            return None;
        }

        // unpeek only works when we are higher then current pos cursor.
        // it should have no effect when have not moved forward
        if self.peek_pos > self.pos {
            self.peek_pos -= 1;
        }

        let new_peek_pos = self.peek_pos + by;
        Some(&self.content[self.peek_pos..new_peek_pos])
    }

    /// scan returns the whole string slice currently at the points of where
    /// the main pos (position) cursor till the end.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn scan_remaining(&mut self) -> Option<&'a [u8]> {
        Some(&self.content[self.pos..])
    }

    /// scan returns the whole string slice currently at the points of where
    /// the main pos (position) cursor and the peek cursor so you can
    /// pull the string right at the current range.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn scan(&mut self) -> Option<&'a [u8]> {
        Some(&self.content[self.pos..self.peek_pos])
    }

    /// ppeek_at allows you to do a non-permant position cursor adjustment
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
    pub fn ppeek_at(&mut self, from: usize, to: usize) -> Option<&'a [u8]> {
        let new_peek_pos = self.pos + from;
        let until_pos = if new_peek_pos + to > self.content.len() {
            self.content.len()
        } else {
            new_peek_pos + to
        };

        if new_peek_pos > self.content.len() {
            return None;
        }

        Some(&self.content[new_peek_pos..until_pos])
    }

    /// vpeek_at allows you to do a non-permant peek cursor adjustment
    /// by taking the current peek cursor position with an adjustment
    /// where we add the `from` (peek_cursor + from) to get the new
    /// position to start from and `to` is added (peek_cursor + from + to)
    /// the position to end at, if the total is more than the length of the string
    /// then its adjusted to be the string last index for the slice.
    ///
    /// It's a nice way to get to see whats at a given position without changing
    /// the current location of the peek cursor.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn vpeek_at(&mut self, from: usize, to: usize) -> Option<&'a [u8]> {
        let new_peek_pos = self.peek_pos + from;
        let until_pos = if new_peek_pos + to > self.content.len() {
            self.content.len()
        } else {
            new_peek_pos + to
        };

        if new_peek_pos > self.content.len() {
            return None;
        }

        Some(&self.content[new_peek_pos..until_pos])
    }

    /// peek_slice allows you to peek forward by an amount
    /// from the current peek cursor position.
    ///
    /// If we've exhausted the total string slice left or are trying to
    /// take more than available text length then we return None
    /// which can indicate no more text for processing.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    fn peek_slice(&mut self, by: usize) -> Option<&'a [u8]> {
        if self.peek_pos + by > self.content.len() {
            return None;
        }
        let from = self.peek_pos;
        let until_pos = self.peek_pos + by;
        Some(&self.content[from..until_pos])
    }

    /// take returns the total string slice from the
    /// actual accumulators position cursor til the current
    /// peek cursor position with adjustment on `by` amount i.e
    /// [u8][position_cursor..(peek_cursor + by_value)].
    ///
    /// Allow you to collect the whole slice of strings that have been
    /// checked and peeked through.
    ///
    /// If we've exhausted the total string slice left or are trying to
    /// take more than available text length then we return None
    /// which can indicate no more text for processing.
    #[cfg_attr(feature = "debug_trace", tracing::instrument(level = "trace"))]
    #[inline]
    pub fn take_with_amount(&mut self, by: usize) -> Option<(&'a [u8], (usize, usize))> {
        if self.peek_pos + by > self.content.len() {
            return None;
        }

        let until_pos = if self.peek_pos + by > self.content.len() {
            self.content.len()
        } else {
            self.peek_pos + by
        };

        if self.pos >= self.content.len() {
            return None;
        }

        let from = self.pos;
        tracing::debug!(
            "take_with_amount: possibly shift in positions: org:({}, {}) then end in final:({},{})",
            self.content.len(),
            self.pos,
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
            "take_with_amount: sliced worked from: {}, by: {}, till loc: {} with text: '{:?}'",
            self.pos,
            by,
            until_pos,
            content_slice,
        );

        let res = Some((content_slice, position));
        self.pos = self.peek_pos;
        res
    }

    /// take_positional returns the total string slice from the
    /// actual accumulators position cursor til the current
    /// peek cursor position i.e [u8][position_cursor...peek_cursor].
    /// Allow you to collect the whole slice of strings that have been
    /// checked and peeked through.
    #[inline]
    pub fn take_positional(&mut self) -> Option<(&'a [u8], (usize, usize))> {
        self.take_with_amount(0)
    }

    /// take returns the total string slice from the
    /// actual accumulators position cursor til the current
    /// peek cursor position i.e [u8][position_cursor...peek_cursor].
    /// Allow you to collect the whole slice of strings that have been
    /// checked and peeked through.
    #[inline]
    pub fn take(&mut self) -> Option<&'a [u8]> {
        match self.take_with_amount(0) {
            Some((text, _)) => Some(text),
            None => None,
        }
    }
}

#[cfg(test)]
mod bytes_pointer_tests {
    use super::*;

    #[test]
    fn test_can_use_accumulator_to_peek_next_character() {
        let mut accumulator = BytesPointer::new(b"hello");
        assert_eq!(b"h", accumulator.peek(1).unwrap());
        assert_eq!(b"h", accumulator.peek(1).unwrap());
    }

    #[test]
    fn test_can_use_accumulator_to_peek_two_characters_away() {
        let mut accumulator = BytesPointer::new(b"hello");
        assert_eq!(b"he", accumulator.peek(2).unwrap());
    }

    #[test]
    fn test_can_virtual_peek_ahead_without_changing_peek_cursor() {
        let mut accumulator = BytesPointer::new(b"hello");

        assert_eq!(b"h", accumulator.peek_next().unwrap());
        assert_eq!(b"e", accumulator.peek_next().unwrap());

        assert_eq!(b"llo", accumulator.vpeek_at(0, 3).unwrap()); // from peek cursor till 3 ahead
        assert_eq!(b"lo", accumulator.vpeek_at(1, 3).unwrap()); // from 1 character ahead of peek cursor

        assert_eq!(b"l", accumulator.peek_next().unwrap());
        assert_eq!(b"l", accumulator.peek_next().unwrap());
        assert_eq!(b"o", accumulator.peek_next().unwrap());
        assert_eq!(None, accumulator.peek_next());
    }

    #[test]
    fn test_can_peek_next_to_accumulate_more_seen_text() {
        let mut accumulator = BytesPointer::new(b"hello");

        assert_eq!(b"h", accumulator.peek_next().unwrap());
        assert_eq!(b"e", accumulator.peek_next().unwrap());
        assert_eq!(b"l", accumulator.peek_next().unwrap());
        assert_eq!(b"l", accumulator.peek_next().unwrap());
        assert_eq!(b"o", accumulator.peek_next().unwrap());

        assert_eq!(None, accumulator.peek_next());
    }

    #[test]
    fn test_can_peek_next_and_take_text_then_continue_peeking() {
        let mut accumulator = BytesPointer::new(b"hello");

        assert_eq!(5, accumulator.len());

        assert_eq!(b"h", accumulator.peek_next().unwrap());
        assert_eq!(b"e", accumulator.peek_next().unwrap());
        assert_eq!(b"l", accumulator.peek_next().unwrap());

        assert_eq!(5, accumulator.len());
        assert_eq!(5, accumulator.rem_len());
        assert_eq!(2, accumulator.peek_rem_len());

        assert_eq!(b"hel", accumulator.take().unwrap());

        assert_eq!(2, accumulator.rem_len());
        assert_eq!(2, accumulator.peek_rem_len());

        assert_eq!(b"l", accumulator.peek_next().unwrap());
        assert_eq!(b"o", accumulator.peek_next().unwrap());

        assert_eq!(2, accumulator.rem_len());
        assert_eq!(0, accumulator.peek_rem_len());

        assert_eq!(None, accumulator.peek_next());

        assert_eq!(2, accumulator.rem_len());
        assert_eq!(0, accumulator.peek_rem_len());

        assert_eq!(b"lo", accumulator.take().unwrap());

        assert_eq!(0, accumulator.rem_len());

        assert_eq!(None, accumulator.peek_next());
    }
}
