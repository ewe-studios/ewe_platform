use std::io::{BufRead, BufReader, BufWriter, Cursor, IoSlice, IoSliceMut, Read, Result, Write};

use derive_more::derive::From;

// BufferCapacity Trait

pub trait BufferCapacity {
    fn read_buffer(&self) -> &[u8];
    fn read_capacity(&self) -> usize;

    fn buffer_length(&self) -> usize {
        self.read_buffer().len()
    }
}

// -- Reader

pub struct BufferedReader<T: ?Sized> {
    inner: BufReader<T>,
}

// -- Constructor

impl<T: Read> BufferedReader<T> {
    pub fn with_capacity(capacity: usize, inner: T) -> Self {
        Self {
            inner: BufReader::with_capacity(capacity, inner),
        }
    }

    pub fn new(inner: T) -> Self {
        Self {
            inner: BufReader::new(inner),
        }
    }
}

impl<T: Read + Write> BufferedReader<BufferedWriter<T>> {
    pub fn get_core_ref(&self) -> &T {
        self.inner.get_ref().get_inner_ref()
    }

    pub fn get_core_mut(&mut self) -> &mut T {
        self.inner.get_mut().get_inner_mut()
    }
}

impl<T: Read> BufferedReader<T> {
    pub fn get_ref(&self) -> &BufReader<T> {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut BufReader<T> {
        &mut self.inner
    }

    pub fn get_inner_ref(&self) -> &T {
        self.inner.get_ref()
    }

    pub fn get_inner_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    pub fn capacity(&mut self) -> usize {
        self.inner.capacity()
    }

    pub fn buffer(&self) -> &[u8] {
        self.inner.buffer()
    }

    pub fn buffer_len(&self) -> usize {
        self.inner.buffer().len()
    }

    pub fn consume(&mut self, amount: usize) {
        self.inner.consume(amount);
    }
}

impl<T: BufferCapacity> BufferCapacity for BufferedReader<T> {
    fn read_buffer(&self) -> &[u8] {
        self.inner.buffer()
    }

    fn read_capacity(&self) -> usize {
        self.inner.capacity()
    }
}

// -- Implement Read for Size? for both BufRead & Read

impl<T: Read + ?Sized> BufRead for BufferedReader<T> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt)
    }
}

impl<T: Read + ?Sized> Read for BufferedReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        self.inner.read_vectored(bufs)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        self.inner.read_exact(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        self.inner.read_to_end(buf)
    }

    fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
        self.inner.read_to_string(buf)
    }
}

impl<T: Write + ?Sized> Write for BufferedReader<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.inner.get_mut().write(buf)
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.inner.get_mut().write_all(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        self.inner.get_mut().write_vectored(bufs)
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.get_mut().flush()
    }
}

#[cfg(test)]
mod buffered_reader_tests {
    use super::*;

    #[test]
    fn can_buffered_reader_peek() {
        let content = b"alexander_wonderbat";
        let mut reader = BufferedReader::new(&content[..]);

        let mut content_to_read = vec![0; 5];
        reader
            .peek(&mut content_to_read)
            .expect("should read data correctly");

        assert_eq!(b"alexa", &content_to_read[..]);

        assert_eq!(content, reader.buffer());
    }
}

// -- Writer

pub struct BufferedWriter<T: ?Sized + Write> {
    inner: BufWriter<T>,
}

// -- Constructor

impl<T: Write> BufferedWriter<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner: BufWriter::new(inner),
        }
    }

    pub fn with_capacity(capacity: usize, inner: T) -> Self {
        Self {
            inner: BufWriter::with_capacity(capacity, inner),
        }
    }
}

impl<T: Read + Write + BufferCapacity> BufferedWriter<T> {
    pub fn read_capacity(&mut self) -> usize {
        self.inner.get_ref().read_capacity()
    }

    pub fn read_buffer(&self) -> &[u8] {
        self.inner.get_ref().read_buffer()
    }
}

impl<T: Read + Write> BufferedWriter<BufferedReader<T>> {
    pub fn get_core_ref(&self) -> &T {
        self.inner.get_ref().get_inner_ref()
    }

    pub fn get_core_mut(&mut self) -> &mut T {
        self.inner.get_mut().get_inner_mut()
    }
}

impl<T: Write> BufferedWriter<T> {
    pub fn get_ref(&self) -> &BufWriter<T> {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut BufWriter<T> {
        &mut self.inner
    }

    pub fn get_inner_ref(&self) -> &T {
        self.inner.get_ref()
    }

    pub fn get_inner_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    pub fn capacity(&mut self) -> usize {
        self.inner.capacity()
    }

    pub fn buffer(&self) -> &[u8] {
        self.inner.buffer()
    }
}

// -- Implement Write for Size? for both BufRead & Read

impl<T: Write + ?Sized> Write for BufferedWriter<T> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.inner.write(buf)
    }

    fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        self.inner.write_all(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        self.inner.write_vectored(bufs)
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()
    }
}

impl<T: Write + Read + ?Sized> Read for BufferedWriter<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.inner.get_mut().read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        self.inner.get_mut().read_vectored(bufs)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        self.inner.get_mut().read_exact(buf)
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        self.inner.get_mut().read_to_end(buf)
    }

    fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
        self.inner.get_mut().read_to_string(buf)
    }
}

impl<T: Write + BufRead + ?Sized> BufRead for BufferedWriter<T> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        self.inner.get_mut().fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.get_mut().consume(amt)
    }
}

pub type BufferedStream<T> = BufferedWriter<BufferedReader<T>>;

/// Returns a new BufferedStream type for `T` which is wrapping a `BufferedReader` and `BufferedWriter`
/// to support both read and write.
pub fn buffered_stream<T: Write + Read>(inner: T) -> BufferedStream<T> {
    BufferedWriter::new(BufferedReader::new(inner))
}

/// Returns a new BufferedStream type for `T` which is wrapping a `BufferedReader` and `BufferedWriter`
/// to support both read and write with a customized capacity.
pub fn buffered_stream_with_capacity<T: Write + Read>(
    capacity: usize,
    inner: T,
) -> BufferedStream<T> {
    BufferedWriter::with_capacity(capacity, BufferedReader::with_capacity(capacity, inner))
}

#[derive(From, Debug)]
pub enum PeekError {
    NotSupported,
    BiggerThanCapacity {
        requested: usize,
        buffer_capacity: usize,
    },
    IOError(std::io::Error),
}

impl std::error::Error for PeekError {}

impl core::fmt::Display for PeekError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub trait PeekableReadStream: Read {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError>;
}

impl<T: Read> PeekableReadStream for BufferedReader<T> {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        if buf.len() > self.inner.capacity() {
            return Err(PeekError::BiggerThanCapacity {
                requested: buf.len(),
                buffer_capacity: self.inner.capacity(),
            });
        }

        let mut last_len = 0;
        while self.inner.buffer().len() < buf.len() {
            self.inner.fill_buf()?;
            let current_len = self.inner.buffer().len();
            if last_len == current_len {
                break;
            }
            last_len = current_len;
        }

        let buffer = self.inner.buffer();

        let ending = if buffer.len() < buf.len() {
            buffer.len()
        } else {
            buf.len()
        };

        for (index, elem) in buffer[0..ending].iter().enumerate() {
            buf[index] = *elem
        }

        Ok(ending)
    }
}

pub struct ByteBufferPointer<'a, T: Read> {
    reader: &'a mut BufferedReader<T>,
    buffer: Vec<u8>,
    pos: usize,
    peek_pos: usize,
}

// Constructors

impl<'a, T: Read> ByteBufferPointer<'a, T> {
    pub fn new(buf_capacity: usize, reader: &'a mut BufferedReader<T>) -> Self {
        Self {
            buffer: Vec::with_capacity(buf_capacity),
            reader: reader,
            pos: 0,
            peek_pos: 0,
        }
    }
}

// Methods

/// [PeekState] is the request peek value you wanted to see.
///
pub enum PeekState<'a> {
    Request(&'a [u8]), // data you resulted
    EndOfBuffered,     // end of buffered data, so consume and read more
    EndOfFile,         // end of file, the real underlying stream is finished
}

#[allow(unused)]
impl<'a, T: Read> ByteBufferPointer<'a, T> {
    #[inline]
    pub fn is_empty(&mut self) -> bool {
        self.reader.buffer_len() == 0
    }

    /// Returns the total length of the string being accumulated on.
    #[inline]
    pub fn len(&mut self) -> usize {
        self.reader.buffer_len()
    }

    pub fn peek(&'a self, by: usize) -> Option<&'a [u8]> {
        let buffer = self.reader.buffer();

        None
    }

    // /// Returns the distance between the peek position and the actual cursor
    // /// position.
    // #[inline]
    // pub fn distance(&mut self) -> usize {
    //     self.peek_pos - self.pos
    // }
    //
    // /// [`fill`] asks the internal reader to fill the buffer with more
    // /// data if there is still space for it else its a no-op.
    // pub fn fill(&mut self) -> std::io::Result<()> {
    //     self.reader.fill_buf().map(|_| ())
    // }
    //
    // /// peek_slice allows you to peek forward by an amount
    // /// from the current peek cursor position.
    // ///
    // /// If we've exhausted the total string slice left or are trying to
    // /// take more than available text length then we return None
    // /// which can indicate no more text for processing.
    // ///
    // /// IMPORTANT: because we are using a BufferedReader internally, all this operations
    // /// instead work on the currently filled buffer (array slice), hence progress is only
    // /// ever possible once you've consumed the data via calling read or consume.
    // #[inline]
    // fn peek_slice(&'a mut self, by: usize) -> Option<&'a [u8]> {
    //     let mut until_pos = self.peek_pos + by;
    //     if self.peek_pos + by > self.len() {
    //         until_pos = self.len()
    //     }
    //
    //     let from = self.peek_pos;
    //
    //     let buf = self.reader.buffer();
    //
    //     Some(&buf[from..until_pos])
    // }

    /// peek_slice_range allows you to peek forward by an amount
    /// from the current peek cursor position to a specific position
    /// in the current reader's buffer (if the position is more than the buffer length)
    /// then it uses the length of the underlying buffer.
    ///
    /// If we've exhausted the total string slice left or are trying to
    /// take more than available text length then we return None
    /// which can indicate no more text for processing.
    ///
    /// IMPORTANT: because we are using a BufferedReader internally, all this operations
    /// instead work on the currently filled buffer (array slice), hence progress is only
    /// ever possible once you've consumed the data via calling read or consume.
    pub fn peek_slice_range(&'a mut self, from: usize, to: usize) -> Option<&'a [u8]> {
        let new_peek_pos = self.peek_pos + from;
        let until_pos = if new_peek_pos + to > self.len() {
            self.len()
        } else {
            new_peek_pos + to
        };

        let buf = self.reader.buffer();
        Some(&buf[new_peek_pos..until_pos])
    }

    // /// scan returns the whole string slice currently at the points of where
    // /// the main pos (position) cursor till the end.
    // #[inline]
    // pub fn scan_remaining(&'a mut self) -> Option<&'a [u8]> {
    //     let buf = self.reader.buffer();
    //     let portion = &buf[self.pos..];
    //
    //     Some(portion)
    // }
    //
    // /// scan returns the whole string slice currently at the points of where
    // /// the main pos (position) cursor and the peek cursor so you can
    // /// pull the string right at the current range.
    // #[inline]
    // pub fn scan(&'a mut self) -> Option<&'a [u8]> {
    //     let buf = self.reader.buffer();
    //     let portion = &buf[self.pos..self.peek_pos];
    //
    //     Some(portion)
    // }
    //
    // /// unpeek_slice lets you reverse the peek cursor position
    // /// by a certain amount to reverse the forward movement.
    // #[inline]
    // fn unpeek_slice(&'a mut self, by: usize) -> Option<&'a [u8]> {
    //     if self.peek_pos == 0 {
    //         return None;
    //     }
    //
    //     // unpeek only works when we are higher then current pos cursor.
    //     // it should have no effect when have not moved forward
    //     if self.peek_pos > self.pos {
    //         self.peek_pos -= 1;
    //     }
    //
    //     let new_peek_pos = self.peek_pos + by;
    //     let buf = self.reader.buffer();
    //     let portion = &buf[self.peek_pos..new_peek_pos];
    //
    //     Some(portion)
    // }
    //
    // /// peek pulls the next token at the current peek position
    // /// cursor which will
    // #[inline]
    // pub fn peek(&'a mut self, by: usize) -> Option<&'a [u8]> {
    //     self.peek_slice(by)
    // }

    /// peek_next allows you to increment the peek cursor, moving
    /// the peek cursor forward by a step and returns the next
    /// token string.
    #[inline]
    pub fn peek_next(&'a mut self) -> Option<&'a [u8]> {
        let by = 1;
        if self.peek_pos + by > self.len() {
            return None;
        }

        let from = self.peek_pos;
        let until_pos = self.peek_pos + by;

        let buf = self.reader.buffer();
        let portion = &buf[from..until_pos];

        self.peek_pos += by;
        Some(portion)
    }

    /// peek_next allows you to increment the peek cursor, moving
    /// the peek cursor forward by a mount of step and returns the next
    /// token string.
    #[inline]
    pub fn peek_next_by(&'a mut self, by: usize) -> Option<&'a [u8]> {
        if self.peek_pos + by > self.len() {
            return None;
        }

        let from = self.peek_pos;
        let until_pos = self.peek_pos + by;

        let buf = self.reader.buffer();
        let portion = &buf[from..until_pos];

        self.peek_pos += by;
        Some(portion)
    }

    // /// unpeek_next reverses the last forward move of the peek
    // /// cursor by -1.
    // #[inline]
    // pub fn unpeek_next(&'a mut self) -> Option<&'a [u8]> {
    //     if let Some(res) = self.unpeek_slice(1) {
    //         return Some(res);
    //     }
    //     None
    // }

    /// look_with_amount returns the total bytes slice from the
    /// actual accumulators position cursor til the current
    /// peek cursor position with adjustment on `by` amount i.e
    /// [u8][position_cursor..(peek_cursor + by_value)].
    ///
    /// It provides super-useful way to borrow the contents within
    /// that region temporarily without actually consuming the reader
    /// which will empower in certain scenarios.
    ///
    /// But note, it never actual moves the position of the cursors (position and peek cursor)
    /// to do that instead use [`Self::consume_with_amount`] or [`Self::consume`]
    ///
    #[inline]
    pub fn look_with_amount(&'a mut self, by: usize) -> Option<(&'a [u8], (usize, usize))> {
        if self.peek_pos + by > self.len() {
            return None;
        }

        let until_pos = if self.peek_pos + by > self.len() {
            self.len()
        } else {
            self.peek_pos + by
        };

        if self.pos >= self.len() {
            return None;
        }

        let from = self.pos;
        // tracing::debug!(
        //     "take_with_amount: possibly shift in positions: org:({}, {}) then end in final:({},{})",
        //     self.len(),
        //     self.pos,
        //     from,
        //     until_pos,
        // );

        let position = (from, until_pos);

        // tracing::debug!(
        //     "take_with_amount: len: {} with pos: {}, peek_pos: {}, by: {}, until: {}",
        //     self.len(),
        //     self.pos,
        //     self.peek_pos,
        //     by,
        //     until_pos,
        // );

        let buf = self.reader.buffer();
        let content_slice = &buf[from..until_pos];

        // tracing::debug!(
        //     "take_with_amount: sliced worked from: {}, by: {}, till loc: {} with text: '{:?}'",
        //     self.pos,
        //     by,
        //     until_pos,
        //     content_slice,
        // );

        let res = Some((content_slice, position));
        res
    }

    /// consume_silently is unique and destructive and must be carefully used, generally it
    /// calculates the amount of bytes between the peek cursor and the current positional cursor
    /// and then consumes (throws away) that amount of bytes in the internal reader.
    #[inline]
    pub fn consume_silently(&mut self) -> std::io::Result<()> {
        let buf_remaining = self.len() - self.pos;
        let amount = self.peek_pos - self.pos;
        self.reader.consume(amount);

        if buf_remaining <= amount {
            self.pos = self.peek_pos;
        } else {
            self.reader.fill_buf()?;
            self.pos = 0;
        }

        Ok(())
    }

    #[inline]
    pub(crate) fn consume_amount_silently(
        &mut self,
        amount: usize,
        new_pos: usize,
    ) -> std::io::Result<()> {
        let buf_remaining = self.len() - self.pos;
        self.reader.consume(amount);
        if buf_remaining <= amount {
            self.pos = new_pos;
        } else {
            self.reader.fill_buf()?;
            self.pos = 0;
        }

        Ok(())
    }

    #[inline]
    pub fn consme(&mut self) -> std::io::Result<(Vec<u8>, (usize, usize))> {
        self.consume_with_amount(0)
    }

    /// [`consume_with_amount`] will perform the same steps as [`look_with_amount`] but
    /// will also consume the data returning a owned Slice of the bytes in that position.
    #[inline]
    pub fn consume_with_amount(&mut self, by: usize) -> std::io::Result<(Vec<u8>, (usize, usize))> {
        let until_pos = if self.peek_pos + by > self.len() {
            self.len()
        } else {
            self.peek_pos + by
        };

        let from = self.pos;
        // tracing::debug!(
        //     "take_with_amount: possibly shift in positions: org:({}, {}) then end in final:({},{})",
        //     self.len(),
        //     self.pos,
        //     from,
        //     until_pos,
        // );

        let position = (from, until_pos);

        // tracing::debug!(
        //     "take_with_amount: len: {} with pos: {}, peek_pos: {}, by: {}, until: {}",
        //     self.len(),
        //     self.pos,
        //     self.peek_pos,
        //     by,
        //     until_pos,
        // );

        let buf = self.reader.buffer();
        let content_slice = &buf[from..until_pos];

        let copied_slice: Vec<u8> = content_slice.into();

        // consume the reader till the position
        let amount = until_pos - from;
        self.consume_amount_silently(until_pos - from, until_pos)?;

        // tracing::debug!(
        //     "take_with_amount: sliced worked from: {}, by: {}, till loc: {} with text: '{:?}'",
        //     self.pos,
        //     by,
        //     until_pos,
        //     content_slice,
        // );

        Ok((copied_slice, position))
    }
}

impl<T: Write + BufRead + BufferCapacity> PeekableReadStream for BufferedWriter<T> {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        if buf.len() > self.get_inner_ref().read_capacity() {
            return Err(PeekError::BiggerThanCapacity {
                requested: buf.len(),
                buffer_capacity: self.get_inner_ref().read_capacity(),
            });
        }

        let mut last_len = 0;
        while self.read_buffer().len() < buf.len() {
            self.inner.get_mut().fill_buf()?;
            let current_len = self.get_inner_ref().read_buffer().len();
            if last_len == current_len {
                break;
            }
            last_len = current_len;
        }

        let buffer = self.get_inner_ref().read_buffer();
        let ending = if buffer.len() < buf.len() {
            buffer.len()
        } else {
            buf.len()
        };

        for (index, elem) in buffer[0..ending].iter().enumerate() {
            buf[index] = *elem
        }
        Ok(ending)
    }
}

// -- Cursor

pub struct BufferedCapacityCursor<T>(std::io::Cursor<T>);

impl BufferCapacity for BufferedCapacityCursor<&str> {
    fn read_buffer(&self) -> &[u8] {
        self.0.get_ref().as_bytes()
    }

    fn read_capacity(&self) -> usize {
        self.0.get_ref().len()
    }
}

impl BufferCapacity for BufferedCapacityCursor<String> {
    fn read_buffer(&self) -> &[u8] {
        self.0.get_ref().as_bytes()
    }

    fn read_capacity(&self) -> usize {
        self.0.get_ref().len()
    }
}

impl BufferCapacity for BufferedCapacityCursor<&[u8]> {
    fn read_buffer(&self) -> &[u8] {
        self.0.get_ref()
    }

    fn read_capacity(&self) -> usize {
        self.0.get_ref().len()
    }
}

impl BufferCapacity for BufferedCapacityCursor<Vec<u8>> {
    fn read_buffer(&self) -> &[u8] {
        self.0.get_ref()
    }

    fn read_capacity(&self) -> usize {
        self.0.get_ref().len()
    }
}

impl<T> BufferedCapacityCursor<T> {
    pub fn new(cursor: std::io::Cursor<T>) -> Self {
        Self(cursor)
    }

    /// get_ref returns the reference to the wrapped `Cursor<T>`.
    pub fn get_ref(&self) -> &Cursor<T> {
        &self.0
    }

    /// get_mut returns the mutable reference to the wrapped `Cursor<T>`.
    pub fn get_mut(&mut self) -> &mut Cursor<T> {
        &mut self.0
    }

    /// get_inner_mut returns a immutable reference to the  inner content
    /// of the wrapped `Cursor<T>`.
    pub fn get_inner_ref(&self) -> &T {
        self.0.get_ref()
    }

    /// get_inner_mut returns a mutable reference to the  inner content
    /// of the wrapped `Cursor<T>`.
    pub fn get_inner_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }
}

#[cfg(test)]
mod buffered_writer_tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn can_buffered_writer_peek() {
        let content = b"alexander_wonderbat";
        let mut reader = BufferedReader::new(BufferedWriter::new(Cursor::new(content.to_vec())));

        let mut content_to_read = vec![0; 5];
        reader
            .peek(&mut content_to_read)
            .expect("should read data correctly");

        assert_eq!(b"alexa", &content_to_read[..]);

        assert_eq!(content, reader.buffer());
    }
}
