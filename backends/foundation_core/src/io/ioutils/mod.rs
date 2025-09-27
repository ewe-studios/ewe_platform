use std::cell::RefCell;
use std::io::{BufRead, BufReader, BufWriter, Cursor, IoSlice, IoSliceMut, Read, Result, Write};
use std::rc::Rc;
use std::sync::atomic::AtomicPtr;
use std::sync::{Arc, Mutex, RwLock};

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
    ZeroLengthNotAllowed,
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

#[derive(Clone)]
pub enum OwnedReader<T: Read> {
    Atomic(Arc<AtomicPtr<T>>),
    Sync(Arc<Mutex<T>>),
    RWrite(Arc<RwLock<T>>),
}

impl<T: Read> OwnedReader<T> {
    pub fn rwrite(reader: Arc<RwLock<T>>) -> Self {
        Self::RWrite(reader)
    }

    pub fn sync(reader: Arc<Mutex<T>>) -> Self {
        Self::Sync(reader)
    }

    pub fn atomic(reader: &mut T) -> Self {
        Self::Atomic(Arc::new(AtomicPtr::new(reader)))
    }
}

impl<T: Read> Read for OwnedReader<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match self {
            Self::Atomic(core) => {
                let ptr = core.load(std::sync::atomic::Ordering::Acquire);
                unsafe {
                    let atomic_reader: &mut T = &mut *ptr;
                    atomic_reader.read(buf)
                }
            }
            Self::Sync(core) => {
                let mut guard = core.lock().expect("can acquire");
                guard.read(buf)
            }
            Self::RWrite(core) => {
                let mut guard = core.write().expect("can acquire");
                guard.read(buf)
            }
        }
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        match self {
            Self::Atomic(core) => {
                let ptr = core.load(std::sync::atomic::Ordering::Acquire);
                unsafe {
                    let atomic_reader: &mut T = &mut *ptr;
                    atomic_reader.read_vectored(buf)
                }
            }
            Self::Sync(core) => {
                let mut guard = core.lock().expect("can acquire");
                guard.read_vectored(bufs)
            }
            Self::RWrite(core) => {
                let mut guard = core.write().expect("can acquire");
                guard.read_vectored(bufs)
            }
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        match self {
            Self::Atomic(core) => {
                let ptr = core.load(std::sync::atomic::Ordering::Acquire);
                unsafe {
                    let atomic_reader: &mut T = &mut *ptr;
                    atomic_reader.read_exact(buf)
                }
            }
            Self::Sync(core) => {
                let mut guard = core.lock().expect("can acquire");
                guard.read_exact(buf)
            }
            Self::RWrite(core) => {
                let mut guard = core.write().expect("can acquire");
                guard.read_exact(buf)
            }
        }
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        match self {
            Self::Atomic(core) => {
                let ptr = core.load(std::sync::atomic::Ordering::Acquire);
                unsafe {
                    let atomic_reader: &mut T = &mut *ptr;
                    atomic_reader.read_to_end(buf)
                }
            }
            Self::Sync(core) => {
                let mut guard = core.lock().expect("can acquire");
                guard.read_to_end(buf)
            }
            Self::RWrite(core) => {
                let mut guard = core.write().expect("can acquire");
                guard.read_to_end(buf)
            }
        }
    }

    fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
        match self {
            Self::Atomic(core) => {
                let ptr = core.load(std::sync::atomic::Ordering::Acquire);
                unsafe {
                    let atomic_reader: &mut T = &mut *ptr;
                    atomic_reader.read_to_string(buf)
                }
            }
            Self::Sync(core) => {
                let mut guard = core.lock().expect("can acquire");
                guard.read_to_string(buf)
            }
            Self::RWrite(core) => {
                let mut guard = core.write().expect("can acquire");
                guard.read_to_string(buf)
            }
        }
    }
}

pub struct ByteBufferPointer<T: Read> {
    reader: OwnedReader<T>,
    pull_amount: usize,
    peek_pos: usize,
    buffer: Vec<u8>,
    pos: usize,
}

// Constructors

const DEFAULT_READ_SIZE: usize = 1024;

impl<T: Read> ByteBufferPointer<T> {
    pub fn new(pull_amount: usize, reader: OwnedReader<T>) -> Self {
        Self {
            buffer: Vec::with_capacity(pull_amount),
            pull_amount,
            peek_pos: 0,
            reader,
            pos: 0,
        }
    }

    pub fn from_reader(pull_amount: usize, reader: T) -> Self {
        let wrapped_reader = OwnedReader::rwrite(Arc::new(RwLock::new(reader)));
        Self::new(pull_amount, wrapped_reader)
    }

    pub fn reader(reader: T) -> Self {
        let wrapped_reader = OwnedReader::rwrite(Arc::new(RwLock::new(reader)));
        Self::new(DEFAULT_READ_SIZE, wrapped_reader)
    }
}

// Methods

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeekState<'a> {
    Request(&'a [u8]), // data you resulted
    LessThanRequested, // when data is way less than requested position
    EndOfBuffered,     // end of buffered data, so consume and read more
    EndOfFile,         // end of file, the real underlying stream is finished
    NoNext, // indicates the peek cursor is still at the same position as the data cursor.
    Continue,
    ZeroLengthInput, // indicates you can continue to peek into the buffer
}

#[allow(unused)]
impl<T: Read> ByteBufferPointer<T> {
    /// Returns the distance between the peek position and the actual cursor
    /// position.
    #[inline]
    pub fn distance(&self) -> usize {
        self.peek_pos - self.pos
    }

    pub fn peek_cursor(&self) -> usize {
        self.peek_pos
    }

    pub fn data_cursor(&self) -> usize {
        self.pos
    }

    /// Returns the total length of the string being accumulated on.
    #[inline]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    pub fn is_empty(&mut self) -> bool {
        self.buffer.len() == 0
    }

    #[inline]
    pub fn uncapture(&mut self, by: usize) {
        self.uncapture_by(1);
    }

    #[inline]
    pub fn uncapture_by(&mut self, by: usize) {
        if (self.pos - by) > 0 {
            self.pos -= by;
        } else {
            self.pos = 0;
        }
    }

    /// full_scan returns the whole buffer as is, so you see the entire
    /// content regardless of cursors position.
    #[inline]
    pub fn full_scan<'a>(&'a self) -> &'a [u8] {
        &self.buffer[..]
    }

    /// scan returns the whole string slice currently at the points of where
    /// the main pos (position) cursor till the end.
    #[inline]
    pub fn scan<'a>(&'a self) -> &'a [u8] {
        &self.buffer[self.pos..self.peek_pos]
    }

    #[inline]
    pub fn greater_than_40_percent(&self) -> bool {
        // if we have not moved at all the ignore
        if self.pos == 0 {
            return false;
        }

        let buffer_length = self.buffer.len();
        let precentage = (buffer_length as f64 / self.pos as f64);
        return precentage > 0.4;
    }

    #[inline]
    pub fn truncate(&mut self, force: bool) {
        // if we have not moved at all the ignore
        if self.pos == 0 {
            return;
        }

        let distance = self.peek_pos - self.pos;
        let buffer_length = self.buffer.len();
        let percentage = (buffer_length as f64 / self.pos as f64);
        let should_truncate = percentage > 0.7 || force;

        if !should_truncate {
            return;
        }

        let slice = &self.buffer[self.pos..buffer_length];
        let mut slice_copy = vec![0; slice.len()];
        slice_copy.truncate(0);
        slice_copy.extend_from_slice(slice);
        let slice_length = slice_copy.len();

        // move the slice to the start of the buffer.
        for (index, byte) in slice_copy.iter().enumerate() {
            self.buffer[index] = *byte;
        }

        self.buffer.truncate(slice_length);
        self.pos = 0;
        self.peek_pos = self.pos + distance;
    }

    /// [`fill_up`] fills the internal buffer with the pull amount
    /// which allows you to continue to collect the relevant
    /// set of data which we match is right in the correct set of
    /// bytes until we indicate to the pointer to consume the data until
    /// the position cursor.
    #[inline]
    pub fn fill_up(&mut self) -> std::io::Result<usize> {
        // truncate buffer if we have read most of it.
        let force = self.greater_than_40_percent();
        self.truncate(force);

        // extract and add more to buffer from reader.
        // self.reader.fill_buf()?;
        let mut copied = vec![0; self.pull_amount];
        let read = match self.reader.read(&mut copied) {
            Ok(read) => read,
            Err(err) => return Err(err),
        };

        // copy into the buffer the data just extracted from the buffer.
        // let location_before_extend = self.buffer.len();
        self.buffer.extend_from_slice(&copied[0..read]);

        Ok(read)
    }

    #[inline]
    pub fn peek<'a>(&'a self) -> std::io::Result<PeekState<'a>> {
        self.peek_by(1)
    }

    #[inline]
    pub fn peek_by<'a>(&'a self, by: usize) -> std::io::Result<PeekState<'a>> {
        let until_pos = self.peek_pos + by;

        // if we are further than the current buffer and the actual content of
        // the reading buffer, then indicate we are beyond available data which
        // requires user to fill up the buffer.
        let buffer_length = self.buffer.len();

        // data is less than requested
        if until_pos > buffer_length {
            return Ok(PeekState::LessThanRequested);
        }

        let from = self.peek_pos;
        Ok(PeekState::Request(&self.buffer[from..until_pos]))
    }

    pub fn read_size<'a, 'b>(&'a mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self.peek_size(buf.len()) {
            Ok(state) => match state {
                PeekState::Request(data) => {
                    let ending = if buf.len() > data.len() {
                        data.len()
                    } else {
                        buf.len()
                    };

                    for (index, elem) in data[0..ending].iter().enumerate() {
                        buf[index] = *elem
                    }
                    Ok(data.len())
                }
                PeekState::LessThanRequested => {
                    let ending = if buf.len() > self.buffer.len() {
                        self.buffer.len()
                    } else {
                        buf.len()
                    };
                    for (index, elem) in self.buffer[0..ending].iter().enumerate() {
                        buf[index] = *elem
                    }
                    Ok(ending)
                }
                _ => return Ok(0),
            },
            Err(err) => Err(err),
        }
    }

    /// [`peek_size`] returns a portion of the underlying buffer for the specified
    /// size using a tight loop until the requested size is of data has being pulled
    /// into the internal peek buffer for peeking .
    ///
    /// This moves forward the cursor forward until the requested size is achieved
    /// and if the loop stops and the current buffer size does not match then
    /// we return what's already acquired, so you need to be aware that this can happen
    /// since generally it means we have reached EOF from the internal readers perspective.
    #[inline]
    pub fn peek_size<'a, 'b>(&'a mut self, size: usize) -> std::io::Result<PeekState<'a>> {
        if size == 0 {
            return Ok(PeekState::ZeroLengthInput);
        }

        loop {
            let buffer_len = self.buffer.len();
            let rem = size - buffer_len;

            if rem < 0 {
                break;
            }

            let state = self.peek_by(rem)?;
            let read = match state {
                PeekState::Request(po) => po.len(),
                PeekState::Continue | PeekState::EndOfBuffered | PeekState::LessThanRequested => {
                    continue;
                }
                PeekState::EndOfFile => {
                    break;
                }
                _ => {
                    unreachable!("Should never hit this types")
                }
            };

            self.peek_pos += read;
        }

        let slice = &self.buffer[self.pos..self.peek_pos];
        Ok(PeekState::Request(slice))
    }

    /// [`peek_until`] returns the peek state for the requested data until the delimiter is seen
    /// at which point the underlying reference to the data is either shared in a
    /// [`PeekState::Request`] if fully read else return [`PeekState::EndOfFile`] if the
    /// source returns EOF or if the data available is less than requested.
    ///
    /// It runs in a tight loop and ensures to acquire as much data as needed.
    #[inline]
    pub fn peek_until<'a, 'b>(&'a mut self, signal: &'b [u8]) -> std::io::Result<PeekState<'a>> {
        if signal.len() == 0 {
            return Ok(PeekState::ZeroLengthInput);
        }
        loop {
            let state = self.peek_by(signal.len())?;
            match state {
                PeekState::Request(data) => {
                    if data == signal {
                        break;
                    }
                }
                PeekState::EndOfBuffered | PeekState::LessThanRequested => {
                    // if at the end then return with EndOfFile
                    if self.fill_up()? == 0 {
                        return Ok(PeekState::EndOfFile);
                    }
                    continue;
                }
                PeekState::Continue => continue,
                PeekState::EndOfFile => {
                    return Ok(PeekState::EndOfFile);
                }
                _ => {
                    unreachable!("Should never hit this types")
                }
            };

            self.peek_pos += signal.len();
        }

        // account for the found block where we break and ensure we can capture
        // the last peek correctly.
        self.peek_pos += signal.len();
        Ok(PeekState::Request(&self.buffer[self.pos..self.peek_pos]))
    }

    /// [`read_bytes_until`] reads the provided bytes into a Vec consuming read cursor, until it
    /// finds the relevant else stopping.
    ///
    /// If the new line byte is not found then everything read till that point is appended to the
    /// string and the cursor is consumed.
    pub fn read_bytes_until<'a>(
        &'a mut self,
        target: &[u8],
        buf: &mut Vec<u8>,
    ) -> std::io::Result<usize> {
        let read = match self.peek_until(target) {
            Ok(inner) => match inner {
                PeekState::Request(c) => {
                    unsafe {
                        buf.extend_from_slice(&c);
                    };
                    Ok(c.len())
                }
                PeekState::EndOfFile => Ok(0),
                PeekState::ZeroLengthInput => Ok(0),
                _ => unreachable!("Should never trigger"),
            },
            Err(err) => return Err(err),
        };

        match read {
            Ok(inner) => {
                if inner == 0 {
                    let slice = &self.buffer[self.pos..self.peek_pos];
                    let slice_length = slice.len();
                    unsafe {
                        buf.extend_from_slice(&slice);
                    };

                    self.skip();
                    return Ok(slice_length);
                }

                self.skip();
                return Ok(inner);
            }
            Err(err) => Err(err),
        }
    }

    /// [`read_line`] reads the provided line into a string and consumes the read data moving
    /// the cusrsor forward.
    ///
    /// If the new line byte is not found then everything read till that point is appended to the
    /// string and the cursor is consumed.
    pub fn read_line<'a>(&'a mut self, buf: &mut String) -> std::io::Result<usize> {
        const NEWLINE_SLICE: &[u8] = b"\n";
        let read = match self.peek_until(NEWLINE_SLICE) {
            Ok(inner) => match inner {
                PeekState::Request(c) => {
                    unsafe {
                        let mut buf_vec = buf.as_mut_vec();
                        buf_vec.extend_from_slice(&c);
                    };
                    Ok(c.len())
                }
                PeekState::EndOfFile => Ok(0),
                PeekState::ZeroLengthInput => Ok(0),
                _ => unreachable!("Should never trigger"),
            },
            Err(err) => return Err(err),
        };

        match read {
            Ok(inner) => {
                if inner == 0 {
                    let slice = &self.buffer[self.pos..self.peek_pos];
                    let slice_length = slice.len();

                    unsafe {
                        let mut buf_vec = buf.as_mut_vec();
                        buf_vec.extend_from_slice(&slice);
                    };

                    self.skip();
                    return Ok(slice_length);
                }

                self.skip();
                return Ok(inner);
            }
            Err(err) => Err(err),
        }
    }

    /// [`peek_line`] reads the provided line into a string without consuming the read content.
    /// Allowing you to perform further operation on the data.
    ///
    /// If the newline byte is not found then it reads all the bytes into the provided buffer ontil
    /// it hits EOF or EndOfFile but this is key, it won't move the cursor forward, just copies the
    /// underlying data over.
    pub fn peek_line<'a>(&'a mut self, buf: &mut String) -> std::io::Result<usize> {
        const NEWLINE_SLICE: &[u8] = b"\n";
        let read = match self.peek_until(NEWLINE_SLICE) {
            Ok(inner) => match inner {
                PeekState::Request(c) => {
                    unsafe {
                        let mut buf_vec = buf.as_mut_vec();
                        buf_vec.extend_from_slice(&c);
                    };
                    Ok(c.len())
                }
                PeekState::EndOfFile => Ok(0),
                PeekState::ZeroLengthInput => Ok(0),
                _ => unreachable!("Should never trigger"),
            },
            Err(err) => return Err(err),
        };

        match read {
            Ok(inner) => {
                if inner == 0 {
                    let slice = &self.buffer[self.pos..self.peek_pos];
                    let slice_length = slice.len();

                    unsafe {
                        let mut buf_vec = buf.as_mut_vec();
                        buf_vec.extend_from_slice(&slice);
                    };

                    return Ok(slice_length);
                }

                return Ok(inner);
            }
            Err(err) => Err(err),
        }
    }

    /// [`peek_bytes_until`] reads the provided bytes into a Vec without consuming the read cursor.
    /// Allowing you to perform further operation on the data.
    ///
    /// If the target byte is not found then it reads all the bytes into the provided buffer ontil
    /// it hits EOF or EndOfFile but this is key, it won't move the cursor forward, just copies the
    /// underlying data over.
    pub fn peek_bytes_until<'a>(
        &'a mut self,
        target: &[u8],
        buf: &mut Vec<u8>,
    ) -> std::io::Result<usize> {
        let read = match self.peek_until(target) {
            Ok(inner) => match inner {
                PeekState::Request(c) => {
                    unsafe {
                        buf.extend_from_slice(&c);
                    };
                    Ok(c.len())
                }
                PeekState::EndOfFile => Ok(0),
                PeekState::ZeroLengthInput => Ok(0),
                _ => unreachable!("Should never trigger"),
            },
            Err(err) => return Err(err),
        };

        match read {
            Ok(inner) => {
                if inner == 0 {
                    let slice = &self.buffer[self.pos..self.peek_pos];
                    let slice_length = slice.len();

                    unsafe {
                        buf.extend_from_slice(&slice);
                    };

                    return Ok(slice_length);
                }

                return Ok(inner);
            }
            Err(err) => Err(err),
        }
    }

    #[inline]
    fn forward(&mut self) -> std::io::Result<PeekState<'_>> {
        self.forward_by(1)
    }

    #[inline]
    fn forward_by(&mut self, by: usize) -> std::io::Result<PeekState<'_>> {
        let buffer_length = self.buffer.len();
        let new_peek = self.peek_pos + by;
        if new_peek > buffer_length {
            self.peek_pos = buffer_length;
            Ok(PeekState::EndOfBuffered)
        } else {
            self.peek_pos = new_peek;
            Ok(PeekState::Continue)
        }
    }

    /// unnext_by moves your peek position backwards at which if it moves
    /// all the way back, will forever stay at the last know position of
    /// the actual data cursor.
    #[inline]
    fn unforward_by(&mut self, by: usize) -> std::io::Result<PeekState<'_>> {
        let new_peek = self.peek_pos - by;
        if new_peek <= self.pos {
            self.peek_pos = self.pos;
            Ok(PeekState::Continue)
        } else {
            self.peek_pos = new_peek;
            Ok(PeekState::Continue)
        }
    }

    /// consumes the amount of data that has been peeked over-so far, returning
    /// that to the caller, this also moves the position of the data cursor
    /// forward to the location of the skip cursor.
    fn consume(&mut self) -> std::io::Result<Vec<u8>> {
        let from = self.pos;
        let until_pos = self.peek_pos;
        if from == until_pos {
            return Ok(vec![]);
        }
        let slice = &self.buffer[from..until_pos];
        self.pos = self.peek_pos;

        let mut slice_copy = vec![0; slice.len()];
        slice_copy.truncate(0);
        slice_copy.extend_from_slice(slice);

        Ok(slice_copy)
    }

    /// skip the amount of data that has been peeked over-so far, returning
    /// that to the caller, this also moves the position of the data cursor
    /// forward to the location of the skip cursor.
    fn skip(&mut self) {
        let from = self.pos;
        let until_pos = self.peek_pos;
        if from == until_pos {
            return;
        }
        self.pos = self.peek_pos;
    }
}

impl<T: Read> Read for ByteBufferPointer<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.read_size(buf)
    }
}

impl<T: Read> PeekableReadStream for Arc<Mutex<ByteBufferPointee<T>>> {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        let binding = self.lock().unwrap();
        binding.peek(buf)
    }
}

impl<T: Read> PeekableReadStream for ByteBufferPointer<T> {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        match self.peek_size(buf.len()) {
            Ok(state) => match state {
                PeekState::Request(data) => {
                    let ending = if buf.len() > data.len() {
                        data.len()
                    } else {
                        buf.len()
                    };

                    for (index, elem) in data[0..ending].iter().enumerate() {
                        buf[index] = *elem
                    }
                    Ok(data.len())
                }
                PeekState::ZeroLengthInput => Ok(0),
                _ => unreachable!("We should never hit this state"),
            },
            Err(err) => Err(PeekError::IOError(err)),
        }
    }
}

#[cfg(test)]
mod byte_buffered_buffer_pointer {
    use std::io::Cursor;
    use std::sync::Arc;
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn can_read_bytes_until_found() {
        const NO_BAR: &[u8] = b"_";
        let content = b"alexander_wonderbat\n";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

        let mut binding = buffer.lock().unwrap();

        let mut new_line = Vec::new();
        let read = binding.read_bytes_until(NO_BAR, &mut new_line);

        assert_eq!(10, read.unwrap());
        let data4 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(new_line, data4);
    }

    #[test]
    fn can_read_bytes_until_not_found() {
        const NO_BAR: &[u8] = b"-";
        let content = b"alexander_wonderbat\n";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

        let mut binding = buffer.lock().unwrap();

        let mut new_line = Vec::new();
        let read = binding.read_bytes_until(NO_BAR, &mut new_line);

        assert_eq!(content.len(), read.unwrap());
        let data4 = vec![
            97, 108, 101, 120, 97, 110, 100, 101, 114, 95, 119, 111, 110, 100, 101, 114, 98, 97,
            116, 10,
        ];
        assert_eq!(new_line, data4);
    }

    #[test]
    fn can_peek_line() {
        let content = b"alexander_wonderbat\n";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

        let mut binding = buffer.lock().unwrap();

        let mut new_line = String::new();
        let read = binding.peek_line(&mut new_line);

        assert_eq!(content.len(), read.unwrap());
        assert_eq!(new_line, "alexander_wonderbat\n");

        let data4 = vec![
            97, 108, 101, 120, 97, 110, 100, 101, 114, 95, 119, 111, 110, 100, 101, 114, 98, 97,
            116, 10,
        ];
        assert_eq!(binding.scan(), &data4);
    }

    #[test]
    fn can_read_line() {
        let content = b"alexander_wonderbat\n";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

        let mut binding = buffer.lock().unwrap();

        let mut new_line = String::new();
        let read = binding.read_line(&mut new_line);

        assert_eq!(content.len(), read.unwrap());
        assert_eq!(new_line, "alexander_wonderbat\n");
    }

    #[test]
    fn can_peek_until_a_signal_and_consume() {
        const SIGNAL: &[u8] = b"_";

        let content = b"alexander_wonderbat";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

        let mut binding = buffer.lock().unwrap();
        assert!(matches!(
            binding.peek_until(SIGNAL).expect("should peek"),
            PeekState::Request(_)
        ));

        let result = binding.consume();
        let content = result.unwrap();

        let data4 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(content, data4);

        let data_4_str = str::from_utf8(&data4).expect("convert into string");
        assert_eq!(data_4_str, "alexander_");
    }

    #[test]
    fn can_peek_until_a_signal() {
        const signal: &[u8] = b"_";

        let content = b"alexander_wonderbat";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

        let mut binding = buffer.lock().unwrap();
        let result = binding.peek_until(signal);
        assert!(matches!(result, Ok(PeekState::Request(_))));

        let PeekState::Request(content) = result.unwrap() else {
            panic!("Failed expectation")
        };

        let data4 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(content, &data4);

        let data_4_str = str::from_utf8(&data4).expect("convert into string");
        assert_eq!(data_4_str, "alexander_");
    }

    #[test]
    fn can_peek_until_a_signal_but_not_found() {
        const signal: &[u8] = b"-";

        let content = b"alexander_wonderbat";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

        let mut binding = buffer.lock().unwrap();
        let result = binding.peek_until(signal);
        assert!(matches!(result, Ok(PeekState::EndOfFile)));
    }

    #[test]
    fn can_take_peeked_data_with_multi_mutex() {
        let content = b"alexander_wonderbat";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

        assert_eq!(
            buffer.lock().unwrap().peek().expect("less than requested"),
            PeekState::LessThanRequested
        );

        buffer.lock().unwrap().fill_up().expect("should fill up");

        let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(
            buffer.lock().unwrap().peek_by(10).expect("capture 3"),
            PeekState::Request(data3.as_ref())
        );

        assert_eq!(
            buffer.lock().unwrap().forward_by(9).expect("capture 3"),
            PeekState::Continue
        );

        assert_eq!(buffer.lock().unwrap().scan(), &data3[0..9]);
        assert_eq!(buffer.lock().unwrap().full_scan(), &data3[0..10]);

        assert_eq!(
            buffer.lock().unwrap().forward_by(10).expect("capture 4"),
            PeekState::EndOfBuffered
        );

        let amt = buffer.lock().unwrap().fill_up().expect("should fill up");

        assert_eq!(
            buffer.lock().unwrap().forward_by(amt).expect("capture 3"),
            PeekState::Continue
        );

        let data4 = vec![
            97, 108, 101, 120, 97, 110, 100, 101, 114, 95, 119, 111, 110, 100, 101, 114, 98, 97,
            116,
        ];
        assert_eq!(buffer.lock().unwrap().scan(), &data4);
    }

    #[test]
    fn can_read_data_via_byte_buffer_pointer() {
        let content = b"alexander_wonderbat";
        let mut source = Cursor::new(content.to_vec());
        let reader = OwnedReader::atomic(&mut source);
        let mut buffer = ByteBufferPointer::new(128, reader);

        assert_eq!(
            buffer.peek().expect("less than requested"),
            PeekState::LessThanRequested
        );

        buffer.fill_up().expect("should fill up");

        let data = vec![97];

        assert_eq!(
            buffer.peek().expect("capture 1"),
            PeekState::Request(data.as_ref())
        );

        let data2 = vec![97, 108];
        assert_eq!(
            buffer.peek_by(2).expect("capture 2"),
            PeekState::Request(data2.as_ref())
        );

        let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(
            buffer.peek_by(10).expect("capture 3"),
            PeekState::Request(data3.as_ref())
        );
    }

    #[test]
    fn can_take_peeked_data() {
        let content = b"alexander_wonderbat";
        let mut source = Cursor::new(content.to_vec());
        let reader = OwnedReader::atomic(&mut source);
        let mut buffer = ByteBufferPointer::new(128, reader);

        assert_eq!(
            buffer.peek().expect("less than requested"),
            PeekState::LessThanRequested
        );

        buffer.fill_up().expect("should fill up");

        let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(
            buffer.peek_by(10).expect("capture 3"),
            PeekState::Request(data3.as_ref())
        );
    }

    #[test]
    fn can_take_unmove_data() {
        let content = b"alexander_wonderbat";
        let mut source = Cursor::new(content.to_vec());
        let reader = OwnedReader::atomic(&mut source);
        let mut buffer = ByteBufferPointer::new(128, reader);

        assert_eq!(
            buffer.peek().expect("less than requested"),
            PeekState::LessThanRequested
        );

        buffer.fill_up().expect("should fill up");

        let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(
            buffer.peek_by(10).expect("capture 3"),
            PeekState::Request(data3.as_ref())
        );

        assert_eq!(
            buffer.forward_by(10).expect("capture 3"),
            PeekState::Continue
        );

        assert_eq!(buffer.scan(), &data3[0..10]);

        assert_eq!(
            buffer.unforward_by(5).expect("capture 3"),
            PeekState::Continue
        );

        assert_eq!(buffer.scan(), &data3[0..5]);
    }

    #[test]
    fn can_skip_data() {
        let content = b"alexander_wonderbat";
        let mut source = Cursor::new(content.to_vec());
        let reader = OwnedReader::atomic(&mut source);
        let mut buffer = ByteBufferPointer::new(128, reader);

        assert_eq!(
            buffer.peek().expect("less than requested"),
            PeekState::LessThanRequested
        );

        buffer.fill_up().expect("should fill up");

        let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(
            buffer.peek_by(10).expect("capture 3"),
            PeekState::Request(data3.as_ref())
        );

        assert_eq!(
            buffer.forward_by(10).expect("capture 3"),
            PeekState::Continue
        );

        assert_eq!(buffer.scan(), &data3[0..10]);

        assert_eq!(
            buffer.unforward_by(5).expect("capture 3"),
            PeekState::Continue
        );

        buffer.skip();

        let data4: Vec<u8> = vec![];
        assert_eq!(buffer.scan(), &data4);

        let data5: Vec<u8> = vec![110, 100, 101, 114, 95];
        assert_eq!(
            buffer.peek_by(5).expect("capture 3"),
            PeekState::Request(data5.as_ref())
        );
    }

    #[test]
    fn can_use_buffered_reader_move_next_data() {
        let content = b"alexander_wonderbat";
        let mut reader = BufferedReader::new(BufferedWriter::new(Cursor::new(content.to_vec())));
        let mut buffer = ByteBufferPointer::new(128, OwnedReader::atomic(&mut reader));

        assert_eq!(
            buffer.peek().expect("less than requested"),
            PeekState::LessThanRequested
        );

        buffer.fill_up().expect("should fill up");

        let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(0, buffer.data_cursor());
        assert_eq!(0, buffer.peek_cursor());
        assert_eq!(
            buffer.peek_by(10).expect("capture 3"),
            PeekState::Request(data3.as_ref())
        );
        assert_eq!(
            buffer.forward_by(10).expect("capture 3"),
            PeekState::Continue
        );
        assert_eq!(buffer.scan(), &data3[0..10]);
        assert_eq!(buffer.greater_than_40_percent(), false);
        assert_eq!(buffer.consume().expect("capture 3"), data3.clone());
        assert_eq!(buffer.greater_than_40_percent(), true);
    }

    #[test]
    fn can_move_next_data() {
        let content = b"alexander_wonderbat";
        let mut source = Cursor::new(content.to_vec());
        let reader = OwnedReader::atomic(&mut source);
        let mut buffer = ByteBufferPointer::new(128, reader);

        assert_eq!(
            buffer.peek().expect("less than requested"),
            PeekState::LessThanRequested
        );

        buffer.fill_up().expect("should fill up");

        let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(0, buffer.data_cursor());
        assert_eq!(0, buffer.peek_cursor());
        assert_eq!(
            buffer.peek_by(10).expect("capture 3"),
            PeekState::Request(data3.as_ref())
        );
        assert_eq!(
            buffer.forward_by(10).expect("capture 3"),
            PeekState::Continue
        );
        assert_eq!(buffer.scan(), &data3[0..10]);
        assert_eq!(buffer.greater_than_40_percent(), false);
        assert_eq!(buffer.consume().expect("capture 3"), data3.clone());
        assert_eq!(buffer.greater_than_40_percent(), true);
    }

    #[test]
    fn can_pull_more_data() {
        let content = b"alexander_wonderbat";
        println!("ContentLength: {}", content.len());

        let mut source = Cursor::new(content.to_vec());
        let reader = OwnedReader::atomic(&mut source);
        let mut buffer = ByteBufferPointer::new(10, reader);

        assert_eq!(
            buffer.peek().expect("less than requested"),
            PeekState::LessThanRequested
        );

        buffer.fill_up().expect("should fill up");

        let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(
            buffer.peek_by(10).expect("capture 3"),
            PeekState::Request(data3.as_ref())
        );

        assert_eq!(
            buffer.forward_by(9).expect("capture 3"),
            PeekState::Continue
        );

        assert_eq!(buffer.scan(), &data3[0..9]);
        assert_eq!(buffer.full_scan(), &data3[0..10]);

        assert_eq!(
            buffer.forward_by(10).expect("capture 4"),
            PeekState::EndOfBuffered
        );

        let amt = buffer.fill_up().expect("should fill up");

        assert_eq!(
            buffer.forward_by(amt).expect("capture 3"),
            PeekState::Continue
        );

        let data4 = vec![
            97, 108, 101, 120, 97, 110, 100, 101, 114, 95, 119, 111, 110, 100, 101, 114, 98, 97,
            116,
        ];
        assert_eq!(buffer.scan(), &data4);
    }

    #[test]
    fn can_pull_more_data_with_mutex() {
        let content = b"alexander_wonderbat";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let mut buffer = ByteBufferPointer::new(10, reader);

        assert_eq!(
            buffer.peek().expect("less than requested"),
            PeekState::LessThanRequested
        );

        buffer.fill_up().expect("should fill up");

        let data3 = vec![97, 108, 101, 120, 97, 110, 100, 101, 114, 95];
        assert_eq!(
            buffer.peek_by(10).expect("capture 3"),
            PeekState::Request(data3.as_ref())
        );

        assert_eq!(
            buffer.forward_by(9).expect("capture 3"),
            PeekState::Continue
        );

        assert_eq!(buffer.scan(), &data3[0..9]);
        assert_eq!(buffer.full_scan(), &data3[0..10]);

        assert_eq!(
            buffer.forward_by(10).expect("capture 4"),
            PeekState::EndOfBuffered
        );

        let amt = buffer.fill_up().expect("should fill up");

        assert_eq!(
            buffer.forward_by(amt).expect("capture 3"),
            PeekState::Continue
        );

        let data4 = vec![
            97, 108, 101, 120, 97, 110, 100, 101, 114, 95, 119, 111, 110, 100, 101, 114, 98, 97,
            116,
        ];
        assert_eq!(buffer.scan(), &data4);
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
