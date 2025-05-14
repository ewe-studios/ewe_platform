use std::io::{BufRead, BufReader, BufWriter, Cursor, IoSlice, IoSliceMut, Read, Result, Write};

use derive_more::derive::From;

// BufferCapacity Trait

pub trait BufferCapacity {
    fn read_buffer(&self) -> &[u8];
    fn read_capacity(&self) -> usize;
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

    pub fn buffer(&mut self) -> &[u8] {
        self.inner.buffer()
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

    pub fn buffer(&mut self) -> &[u8] {
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
        buf.copy_from_slice(&buffer[0..buf.len()]);
        Ok(buf.len())
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
        buf.copy_from_slice(&buffer[0..buf.len()]);
        Ok(buf.len())
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
