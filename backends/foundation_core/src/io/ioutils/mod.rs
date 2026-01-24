// use crate::compati::Mutex;
use std::cell::RefCell;
use std::io::{BufRead, BufReader, BufWriter, Cursor, IoSlice, IoSliceMut, Read, Result, Write};

use std::rc::Rc;
use std::sync::atomic::AtomicPtr;
use std::sync::Arc;
use foundation_nostd::comp::{Mutex, RwLock};

use crate::err;
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
        self.inner.consume(amt);
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
        self.inner.get_mut().consume(amt);
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
            buf[index] = *elem;
        }
        Ok(ending)
    }
}

pub type BufferedStream<T> = BufferedWriter<BufferedReader<T>>;

/// Returns a new `BufferedStream` type for `T` which is wrapping a `BufferedReader` and `BufferedWriter`
/// to support both read and write.
pub fn buffered_stream<T: Write + Read>(inner: T) -> BufferedStream<T> {
    BufferedWriter::new(BufferedReader::new(inner))
}

/// Returns a new `BufferedStream` type for `T` which is wrapping a `BufferedReader` and `BufferedWriter`
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
    LockAcquisitionError,
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
            buf[index] = *elem;
        }

        Ok(ending)
    }
}

pub enum OwnedReader<T: Read> {
    RefCell(Rc<RefCell<T>>),
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

    pub fn ref_cell(reader: Rc<RefCell<T>>) -> Self {
        Self::RefCell(reader)
    }
}

impl<T: Read> OwnedReader<T> {
    pub fn do_ref<F, V>(&self, caller: F) -> V
    where
        F: Fn(&T) -> V,
    {
        match self {
            Self::Atomic(core) => {
                let ptr = core.load(std::sync::atomic::Ordering::Acquire);
                unsafe {
                    let atomic_reader: &mut T = &mut *ptr;
                    (caller)(atomic_reader)
                }
            }
            Self::Sync(core) => {
                let guard = core.lock().expect("can acquire");
                (caller)(&guard)
            }
            Self::RWrite(core) => {
                let guard = core.read().expect("can acquire");
                (caller)(&guard)
            }
            Self::RefCell(core) => {
                let guard = core.borrow();
                (caller)(&guard)
            }
        }
    }

    pub fn do_once<F, V>(&self, caller: F) -> V
    where
        F: FnOnce(&T) -> V,
    {
        match self {
            Self::Atomic(core) => {
                let ptr = core.load(std::sync::atomic::Ordering::Acquire);
                unsafe {
                    let atomic_reader: &mut T = &mut *ptr;
                    (caller)(atomic_reader)
                }
            }
            Self::Sync(core) => {
                let guard = core.lock().expect("can acquire");
                (caller)(&guard)
            }
            Self::RWrite(core) => {
                let guard = core.read().expect("can acquire");
                (caller)(&guard)
            }
            Self::RefCell(core) => {
                let guard = core.borrow();
                (caller)(&guard)
            }
        }
    }

    pub fn do_once_mut<F, V>(&self, caller: F) -> V
    where
        F: FnOnce(&mut T) -> V,
    {
        match self {
            Self::Atomic(core) => {
                let ptr = core.load(std::sync::atomic::Ordering::Acquire);
                unsafe {
                    let atomic_reader: &mut T = &mut *ptr;
                    (caller)(atomic_reader)
                }
            }
            Self::Sync(core) => {
                let mut guard = core.lock().expect("can acquire");
                (caller)(&mut *guard)
            }
            Self::RWrite(core) => {
                let mut guard = core.write().expect("can acquire");
                (caller)(&mut *guard)
            }
            Self::RefCell(core) => {
                let mut guard = core.borrow_mut();
                (caller)(&mut *guard)
            }
        }
    }

    pub fn do_mut<F, V>(&self, mut caller: F) -> V
    where
        F: FnMut(&mut T) -> V,
    {
        match self {
            Self::Atomic(core) => {
                let ptr = core.load(std::sync::atomic::Ordering::Acquire);
                unsafe {
                    let atomic_reader: &mut T = &mut *ptr;
                    (caller)(atomic_reader)
                }
            }
            Self::Sync(core) => {
                let mut guard = core.lock().expect("can acquire");
                (caller)(&mut guard)
            }
            Self::RWrite(core) => {
                let mut guard = core.write().expect("can acquire");
                (caller)(&mut guard)
            }
            Self::RefCell(core) => {
                let mut guard = core.borrow_mut();
                (caller)(&mut guard)
            }
        }
    }
}

impl<T: Read> Clone for OwnedReader<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Atomic(core) => Self::Atomic(Arc::clone(core)),
            Self::RWrite(core) => Self::RWrite(Arc::clone(core)),
            Self::RefCell(core) => Self::RefCell(core.clone()),
            Self::Sync(core) => Self::Sync(core.clone()),
        }
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
            Self::RefCell(core) => {
                let mut guard = core.borrow_mut();
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
                    atomic_reader.read_vectored(bufs)
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
            Self::RefCell(core) => {
                let mut guard = core.borrow_mut();
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
            Self::RefCell(core) => {
                let mut guard = core.borrow_mut();
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
            Self::RefCell(core) => {
                let mut guard = core.borrow_mut();
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
            Self::RefCell(core) => {
                let mut guard = core.borrow_mut();
                guard.read_to_string(buf)
            }
        }
    }
}

// Bare metal platforms usually have very small amounts of RAM
// (in the order of hundreds of KB)
pub const DEFAULT_READ_SIZE: usize = if cfg!(target_os = "espidf") {
    512
} else {
    8 * 1024
};

/// `SharedPointerReader` defines a shared buffer reader pointer that allows reading through
/// a underlying buffered stream.
pub struct SharedByteBufferStream<T: Read>(OwnedReader<ByteBufferPointer<T>>);

impl<T: Read> SharedByteBufferStream<T> {
    pub fn do_ref<F, V>(&self, caller: F) -> V
    where
        F: Fn(&ByteBufferPointer<T>) -> V,
    {
        self.0.do_ref(caller)
    }

    pub fn do_once<F, V>(&self, caller: F) -> V
    where
        F: FnOnce(&ByteBufferPointer<T>) -> V,
    {
        self.0.do_once(caller)
    }

    pub fn do_once_mut<F, V>(&self, caller: F) -> V
    where
        F: FnOnce(&mut ByteBufferPointer<T>) -> V,
    {
        self.0.do_once_mut(caller)
    }

    pub fn do_mut<F, V>(&self, caller: F) -> V
    where
        F: FnMut(&mut ByteBufferPointer<T>) -> V,
    {
        self.0.do_mut(caller)
    }
}

// Implement cloning for the [`SharedByteBufferStream`].
impl<T: Read> Clone for SharedByteBufferStream<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

// Constructors

impl<T: Read> SharedByteBufferStream<T> {
    pub fn ref_cell(reader: T) -> Self {
        let wrapped_reader = OwnedReader::ref_cell(Rc::new(RefCell::new(reader)));
        let byte_reader = ByteBufferPointer::new(DEFAULT_READ_SIZE, wrapped_reader);
        Self(OwnedReader::ref_cell(Rc::new(RefCell::new(byte_reader))))
    }

    pub fn ref_cell_with_capacity(capacity: usize, reader: T) -> Self {
        let wrapped_reader = OwnedReader::ref_cell(Rc::new(RefCell::new(reader)));
        let byte_reader = ByteBufferPointer::new(capacity, wrapped_reader);
        Self(OwnedReader::ref_cell(Rc::new(RefCell::new(byte_reader))))
    }

    pub fn rwrite(reader: T) -> Self {
        let wrapped_reader = OwnedReader::rwrite(Arc::new(RwLock::new(reader)));
        let byte_reader = ByteBufferPointer::new(DEFAULT_READ_SIZE, wrapped_reader);
        Self(OwnedReader::rwrite(Arc::new(RwLock::new(byte_reader))))
    }

    pub fn rwrite_with_capacity(capacity: usize, reader: T) -> Self {
        let wrapped_reader = OwnedReader::rwrite(Arc::new(RwLock::new(reader)));
        let byte_reader = ByteBufferPointer::new(capacity, wrapped_reader);
        Self(OwnedReader::rwrite(Arc::new(RwLock::new(byte_reader))))
    }
}

impl<T: Read> SharedByteBufferStream<BufferedReader<T>> {
    pub fn with_owned_reader(reader: T) -> Self {
        let wrapped_reader = BufferedReader::new(reader);
        Self::rwrite(wrapped_reader)
    }
}

impl<T: Read> SharedByteBufferStream<BufferedReader<T>> {
    pub fn with_buffered_owned_reader(reader: T) -> Self {
        Self::rwrite(BufferedReader::new(reader))
    }
}

// Public Methods

impl<T: Read> PeekableReadStream for SharedByteBufferStream<T> {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        self.0
            .do_once_mut(|binding| match binding.nextby(buf.len()) {
                Ok(state) => match state {
                    PeekState::Request(data) => {
                        let ending = if buf.len() > data.len() {
                            data.len()
                        } else {
                            buf.len()
                        };

                        for (index, elem) in data[0..ending].iter().enumerate() {
                            buf[index] = *elem;
                        }
                        Ok(data.len())
                    }
                    PeekState::ZeroLengthInput => Ok(0),
                    _ => unreachable!("We should never hit this state"),
                },
                Err(err) => Err(PeekError::IOError(err)),
            })
    }
}

impl<T: Read> std::io::Read for SharedByteBufferStream<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.0.do_once_mut(|binding| binding.read(buf))
    }

    fn read_vectored(&mut self, buf: &mut [IoSliceMut<'_>]) -> Result<usize> {
        self.0.do_once_mut(|binding| binding.read_vectored(buf))
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        self.0.do_once_mut(|binding| binding.read_exact(buf))
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        self.0.do_once_mut(|binding| binding.read_to_end(buf))
    }

    fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
        self.0.do_once_mut(|binding| binding.read_to_string(buf))
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

impl<T: Read> ByteBufferPointer<T> {
    #[must_use]
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
pub enum Data<'a> {
    Request(&'a [u8]), // data you resulted
    Consumed(Vec<u8>),
}

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
    #[must_use]
    pub fn distance(&self) -> usize {
        self.peek_pos - self.pos
    }

    #[must_use]
    pub fn peek_cursor(&self) -> usize {
        self.peek_pos
    }

    #[must_use]
    pub fn data_cursor(&self) -> usize {
        self.pos
    }

    /// Returns the total length of the string being accumulated on.
    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
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

    /// `full_scan` returns the whole buffer as is, so you see the entire
    /// content regardless of cursors position.
    #[inline]
    #[must_use]
    pub fn full_scan(&self) -> &[u8] {
        &self.buffer[..]
    }

    /// scan returns the whole string slice currently at the points of where
    /// the main pos (position) cursor till the end.
    #[inline]
    #[must_use]
    pub fn scan(&self) -> &[u8] {
        &self.buffer[self.pos..self.peek_pos]
    }

    #[inline]
    #[must_use]
    pub fn greater_than_40_percent(&self) -> bool {
        // if we have not moved at all the ignore
        if self.pos == 0 {
            return false;
        }

        let buffer_length = self.buffer.len();
        let percentage = (buffer_length as f64 / self.pos as f64);
        percentage > 0.4
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
        let read = self.reader.read(&mut copied)?;

        // copy into the buffer the data just extracted from the buffer.
        // let location_before_extend = self.buffer.len();
        self.buffer.extend_from_slice(&copied[0..read]);

        Ok(read)
    }

    /// [`fill_all`] reads the whole underlying reader into the underlying
    /// buffer, allowing you to extract the remaining data within the stream
    /// as fully in-memory.
    #[inline]
    pub fn fill_all(&mut self, read_limit: Option<usize>) -> std::io::Result<usize> {
        let mut total_read = 0;

        // pull the amount of data until we reach EOF
        let mut copied = vec![0; self.pull_amount];
        loop {
            let read = self.reader.read(&mut copied)?;

            if read == 0 {
                break;
            }

            total_read += read;

            if let Some(limited_val) = read_limit {
                if limited_val < total_read {
                    return Err(crate::err!(
                        Interrupted,
                        "LimitReadTriggered",
                        "Crossed read limit {}",
                        limited_val,
                    ));
                }
            }

            self.buffer.extend_from_slice(&copied[0..read]);
        }

        Ok(total_read)
    }

    /// Forward by 1 by calling the [`Self::forward_by`] method underneath.
    #[inline]
    pub fn forward(&mut self) -> std::io::Result<PeekState<'_>> {
        self.forward_by(1)
    }

    /// [`forward_by`] provides method to move the peek cursor by a certain amount.
    /// Generally this is used external as the logic is generally backed into the
    /// `next*` and `read*` methods but in cases where you intend to progress the
    /// cursor your seek using the `peek*` methods this provides that surface.
    #[inline]
    pub fn forward_by(&mut self, by: usize) -> std::io::Result<PeekState<'_>> {
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

    /// Unforward by 1 by calling the [`Self::unforward_by`] method underneath.
    #[inline]
    pub fn unforward(&mut self) -> std::io::Result<PeekState<'_>> {
        self.unforward_by(1)
    }

    /// [`unforward_by`] moves your peek position backwards at which if it moves
    /// all the way back, will forever stay at the last know position of
    /// the actual data cursor.
    #[inline]
    pub fn unforward_by(&mut self, by: usize) -> std::io::Result<PeekState<'_>> {
        let new_peek = self.peek_pos - by;
        if new_peek <= self.pos {
            self.peek_pos = self.pos;
            Ok(PeekState::Continue)
        } else {
            self.peek_pos = new_peek;
            Ok(PeekState::Continue)
        }
    }

    /// [`consume_some`] consumes the amount of data that has been peeked over-so far, returning
    /// that to the caller, this also moves the position of the data cursor
    /// forward to the location of the skip cursor.
    pub fn consume_some(&mut self) -> Option<Vec<u8>> {
        let from = self.pos;
        let until_pos = self.peek_pos;
        if from == until_pos {
            return None;
        }
        let slice = &self.buffer[from..until_pos];
        let slice_string = String::from_utf8(slice.to_vec());
        self.pos = self.peek_pos;

        let mut slice_copy = vec![0; slice.len()];
        slice_copy.truncate(0);
        slice_copy.extend_from_slice(slice);

        Some(slice_copy)
    }

    /// [`consume`] consumes the amount of data that has been peeked over-so far, returning
    /// that to the caller, this also moves the position of the data cursor
    /// forward to the location of the skip cursor.
    pub fn consume(&mut self) -> std::io::Result<Vec<u8>> {
        match self.consume_some() {
            Some(item) => Ok(item),
            None => Ok(vec![]),
        }
    }

    /// [`skip`] skip the amount of data that has been peeked over-so far, returning
    /// that to the caller, this also moves the position of the data cursor
    /// forward to the location of the skip cursor.
    pub fn skip(&mut self) {
        let from = self.pos;
        let until_pos = self.peek_pos;
        if from == until_pos {
            return;
        }
        self.pos = self.peek_pos;
    }

    /// [`peek`] peeks into the future by 1 position without actually permanently
    /// change the peek cursor's position.
    #[inline]
    pub fn peek(&self) -> std::io::Result<PeekState<'_>> {
        self.peek_by(1)
    }

    /// [`peek_by`] returns the available data if there is available within the forward movement
    /// being requested, the peek cursor is never adjusted but only used to look forward.
    ///
    /// WARNING: Do not use this method and then call [`Self::consume`] has it has no effect
    // or you may consume far less than intended. This is intended to let you peek forward
    /// without actual changes to peek cursor which is used in consume to extract the data
    /// already seen by calling all `next_*` methods.
    #[inline]
    pub fn peek_by(&self, by: usize) -> std::io::Result<PeekState<'_>> {
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

    // [`peek_until`] returns the peek state for the requested data until the delimiter is seen
    /// at which point the underlying reference to the data is either shared in a
    /// [`PeekState::Request`] if fully read else return [`PeekState::EndOfFile`] if the
    /// source returns EOF or if the data available is less than requested.
    ///
    /// It runs in a tight loop and ensures to acquire as much data as needed.
    ///
    /// It catches the initial position of the peek cursor then returns back to that
    /// position after stopping. So the peek cursor will not adjust though the internal
    /// buffer may have been filled up using memory attempting to find the giving signal
    /// and this might read the underlying reader until its EOF or OOM occurs if the signal
    /// is never found.
    ///
    /// WARNING: Do not use this method and then call [`Self::consume`] has it has no effect
    /// or you may consume far less than intended. This is intended to let you peek forward
    /// without actual changes to peek cursor which is used in consume to extract the data
    /// already seen by calling all `next_*` methods.
    #[inline]
    pub fn peek_until<'a>(&'a mut self, signal: &[u8]) -> std::io::Result<PeekState<'a>> {
        if signal.is_empty() {
            return Ok(PeekState::ZeroLengthInput);
        }
        let current_peek_pos = self.peek_pos;
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
            }

            self.peek_pos += signal.len();
        }

        // account for the found block where we break and ensure we can capture
        // the last peek correctly.
        self.peek_pos += signal.len();

        let slice = &self.buffer[self.pos..self.peek_pos];

        // restores peek position
        self.peek_pos = current_peek_pos;

        Ok(PeekState::Request(slice))
    }

    /// [`peekby2`] provides a friendly method which calls [`peek_size`] underneath
    /// to peek into the distance without modifying cursor position.
    ///
    /// This means the cursor is at that position it was after peek into the need size from
    /// the current position.
    ///
    /// WARNING: Do not use this method and then call [`Self::consume`] has it has no effect
    /// or you may consume far less than intended. This is intended to let you peek forward
    /// without actual changes to peek cursor which is used in consume to extract the data
    /// already seen by calling all `next_*` methods.
    pub fn peekby2<'b>(&mut self, size: usize) -> std::io::Result<&[u8]> {
        self.peekby(size).map(|item| match item {
            PeekState::Request(inner) => Ok(inner),
            PeekState::NoNext => Err(crate::err!(UnexpectedEof, "No more data to pull through")),
            PeekState::ZeroLengthInput => Err(crate::err!(WriteZero, "Provided zero size request")),
            _ => unreachable!("Should not trigger this stage"),
        })?
    }

    /// [`peekby`] returns a portion of the underlying buffer for the specified
    /// size using a tight loop until the requested size is of data has being pulled
    /// into the internal peek buffer for peeking .
    ///
    /// This moves forward the peek cursor forward temporarily until the requested size is achieved
    /// and if the loop stops and the current buffer size does not match then
    /// we return what's already acquired, so you need to be aware that this can happen
    /// since generally it means we have reached EOF from the internal readers perspective.
    ///
    /// WARNING: Do not use this method and then call [`Self::consume`] has it has no effect
    /// or you may consume far less than intended. This is intended to let you peek forward
    /// without actual changes to peek cursor which is used in consume to extract the data
    /// already seen by calling all `next_*` methods.
    #[inline]
    pub fn peekby(&mut self, size: usize) -> std::io::Result<PeekState<'_>> {
        if size == 0 {
            return Ok(PeekState::ZeroLengthInput);
        }

        loop {
            let buffer_len = self.buffer.len() as isize;
            let rem = (size as isize) - buffer_len;

            if rem < 0 {
                break;
            }

            // request more data so we get to enough to actually resolve the
            // requested size.
            if self.fill_up()? == 0 {
                break;
            }
        }

        let buffer_len = self.buffer.len();

        let mut until_pos = self.peek_pos + size;
        if until_pos > buffer_len {
            until_pos = buffer_len;
        }

        let slice = &self.buffer[self.peek_pos..until_pos];
        if slice.is_empty() {
            return Ok(PeekState::NoNext);
        }

        Ok(PeekState::Request(slice))
    }

    /// [`next_size`] returns a portion of the underlying buffer for the specified
    /// size using a tight loop until the requested size is of data has being pulled
    /// into the internal peek buffer for peeking .
    ///
    /// This moves forward the cursor forward until the requested size is achieved
    /// and if the loop stops and the current buffer size does not match then
    /// we return what's already acquired, so you need to be aware that this can happen
    /// since generally it means we have reached EOF from the internal readers perspective.
    ///
    /// This moves the peek cursor forward.
    #[inline]
    pub fn nextby(&mut self, size: usize) -> std::io::Result<PeekState<'_>> {
        if size == 0 {
            return Ok(PeekState::ZeroLengthInput);
        }

        loop {
            let buffer_len = self.buffer.len() as isize;
            let rem = (size as isize) - buffer_len;

            if rem < 0 {
                break;
            }

            // request more data so we get to enough to actually resolve the
            // requested size.
            if self.fill_up()? == 0 {
                break;
            }
        }

        let buffer_len = self.buffer.len();

        let original_peek = self.peek_pos;

        // never overflows, how much exactly did we have before we
        // requested for more data
        let total_data_left = buffer_len - original_peek;

        // we were less than even what is requested or exactly what was requested
        // move peek to the end and move on
        if total_data_left <= size {
            self.peek_pos = buffer_len;
        } else {
            // if we basically indicative that the length to wanted is basically
            self.peek_pos = original_peek + size;
        }

        let slice = &self.buffer[original_peek..self.peek_pos];

        if self.peek_pos == original_peek {
            return Ok(PeekState::NoNext);
        }
        Ok(PeekState::Request(slice))
    }

    /// [`read_size`] reads the underlying size of the buffer provided data, consuming
    /// all that is requested which moves both the peek cursor and the actual data cursor
    /// which means that part of the internal buffer read from the underlying reader will
    /// at some point be discarded as it should be.
    pub fn read_size(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = match self.nextby(buf.len()) {
            Ok(state) => match state {
                PeekState::Request(data) => {
                    let ending = if buf.len() > data.len() {
                        data.len()
                    } else {
                        buf.len()
                    };

                    for (index, elem) in data[0..ending].iter().enumerate() {
                        buf[index] = *elem;
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
                        buf[index] = *elem;
                    }
                    Ok(ending)
                }
                _ => return Ok(0),
            },
            Err(err) => Err(err),
        };

        match read {
            Ok(val) => {
                self.skip();
                Ok(val)
            }
            Err(err) => Err(err),
        }
    }

    /// [`nextby`] provides a more friendly API ontop [`Self::next_size`] returning
    /// the slice `&[u8]` without the [`PeekState`] wrapper.
    pub fn nextby2(&mut self, size: usize) -> std::io::Result<&[u8]> {
        self.nextby(size).map(|item| match item {
            PeekState::Request(inner) => Ok(inner),
            PeekState::NoNext => Err(crate::err!(UnexpectedEof, "No more data to pull through")),
            PeekState::ZeroLengthInput => Err(crate::err!(WriteZero, "Provided zero size request")),
            _ => unreachable!("Should not trigger this stage"),
        })?
    }

    /// [`next_until`] returns the peek state for the requested data until the delimiter is seen
    /// at which point the underlying reference to the data is either shared in a
    /// [`PeekState::Request`] if fully read else return [`PeekState::EndOfFile`] if the
    /// source returns EOF or if the data available is less than requested.
    ///
    /// It runs in a tight loop and ensures to acquire as much data as needed.
    ///
    /// This moves the peek cursor forward.
    #[inline]
    pub fn next_until<'a>(&'a mut self, signal: &[u8]) -> std::io::Result<PeekState<'a>> {
        if signal.is_empty() {
            return Ok(PeekState::ZeroLengthInput);
        }
        loop {
            let state = self.peek_by(signal.len())?;
            // tracing::debug!(
            //     "Next_Until: using signal: {:?} against {:?}",
            //     &signal,
            //     &state,
            // );
            match state {
                PeekState::Request(data) => {
                    // tracing::debug!(
                    //     "Next_Until: using signal: {:?}(len={}) against {:?} as {:?}",
                    //     &signal,
                    //     signal.len(),
                    //     &data,
                    //     String::from_utf8(data.to_vec()),
                    // );
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
            }

            self.peek_pos += signal.len();
        }

        // account for the found block where we break and ensure we can capture
        // the last peek correctly.
        self.peek_pos += signal.len();
        Ok(PeekState::Request(&self.buffer[self.pos..self.peek_pos]))
    }

    /// [`next_line`] reads the provided line into a string without consuming the read content.
    /// Allowing you to perform further operation on the data.
    ///
    /// If the newline byte is not found then it reads all the bytes into the provided buffer ontil
    /// it hits EOF or `EndOfFile` but this is key, it won't move the cursor forward, just copies the
    /// underlying data over.
    ///
    /// This moves the peek cursor forward.
    pub fn next_line(&mut self, buf: &mut String) -> std::io::Result<usize> {
        const NEWLINE_SLICE: &[u8] = b"\n";
        let read = match self.next_until(NEWLINE_SLICE) {
            Ok(inner) => match inner {
                PeekState::Request(c) => {
                    unsafe {
                        let mut buf_vec = buf.as_mut_vec();
                        buf_vec.extend_from_slice(c);
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
                        buf_vec.extend_from_slice(slice);
                    };

                    return Ok(slice_length);
                }

                Ok(inner)
            }
            Err(err) => Err(err),
        }
    }

    /// [`next_bytes_until`] reads the provided bytes into a Vec without consuming the read cursor.
    /// Allowing you to perform further operation on the data.
    ///
    /// If the target byte is not found then it reads all the bytes into the provided buffer ontil
    /// it hits EOF or `EndOfFile` but this is key, it won't move the cursor forward, just copies the
    /// underlying data over.
    ///
    /// This moves the peek cursor forward.
    pub fn next_bytes_until(&mut self, target: &[u8], buf: &mut Vec<u8>) -> std::io::Result<usize> {
        let read = match self.next_until(target) {
            Ok(inner) => match inner {
                PeekState::Request(c) => {
                    unsafe {
                        buf.extend_from_slice(c);
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
                        buf.extend_from_slice(slice);
                    };

                    return Ok(slice_length);
                }

                Ok(inner)
            }
            Err(err) => Err(err),
        }
    }

    /// [`read_bytes_until`] reads the provided bytes into a Vec consuming read cursor, until it
    /// finds the relevant else stopping.
    ///
    /// If the new line byte is not found then everything read till that point is appended to the
    /// string and the cursor is consumed.
    ///
    /// This moves the peek cursor forward.
    pub fn read_bytes_until(&mut self, target: &[u8], buf: &mut Vec<u8>) -> std::io::Result<usize> {
        let read = match self.next_until(target) {
            Ok(inner) => match inner {
                PeekState::Request(c) => {
                    unsafe {
                        buf.extend_from_slice(c);
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
                        buf.extend_from_slice(slice);
                    };

                    self.skip();
                    return Ok(slice_length);
                }

                self.skip();
                Ok(inner)
            }
            Err(err) => Err(err),
        }
    }

    /// [`read_line`] reads the provided line into a string and consumes the read data moving
    /// the cusrsor forward.
    ///
    /// If the new line byte is not found then everything read till that point is appended to the
    /// string and the cursor is consumed.
    ///
    /// This moves the peek cursor forward.
    pub fn read_line(&mut self, buf: &mut String) -> std::io::Result<usize> {
        const NEWLINE_SLICE: &[u8] = b"\n";
        let read = match self.next_until(NEWLINE_SLICE) {
            Ok(inner) => match inner {
                PeekState::Request(c) => {
                    match String::from_utf8(c.to_vec()) {
                        Ok(inner) => {
                            buf.push_str(&inner);
                            Ok(inner.len())
                        }
                        Err(err) => {
                            tracing::error!(
                                "Received invalid utf8 data {:?} with error: {:?}",
                                &c,
                                err,
                            );

                            // fallback into character by character handling
                            buf.extend(c.iter().map(|b| char::from(*b)));

                            Ok(c.len())
                        }
                    }
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
                        buf_vec.extend_from_slice(slice);
                    };

                    self.skip();
                    return Ok(slice_length);
                }

                self.skip();
                Ok(inner)
            }
            Err(err) => Err(err),
        }
    }

    /// [`read_all`] pulls the whole data within the underlying stream into provided buffer
    /// returning the total length of bytes read out after pulling all the data within the
    /// stream until EOF.
    pub fn read_all(
        &mut self,
        buf: &mut Vec<u8>,
        read_limit: Option<usize>,
    ) -> std::io::Result<usize> {
        self.fill_all(read_limit)?;

        // get len of buffer
        let buffer_len = self.buffer.len();

        // get current peek position
        let original_peek = self.peek_pos;

        let slice = &self.buffer[original_peek..buffer_len];
        let slice_length = slice.len();

        buf.extend_from_slice(slice);

        self.peek_pos = buffer_len;
        self.skip();

        Ok(slice_length)
    }
}

impl<T: Read> Read for ByteBufferPointer<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.read_size(buf)
    }
}

impl<T: Read> PeekableReadStream for ByteBufferPointer<T> {
    fn peek(&mut self, buf: &mut [u8]) -> std::result::Result<usize, PeekError> {
        match self.nextby(buf.len()) {
            Ok(state) => match state {
                PeekState::Request(data) => {
                    let ending = if buf.len() > data.len() {
                        data.len()
                    } else {
                        buf.len()
                    };

                    for (index, elem) in data[0..ending].iter().enumerate() {
                        buf[index] = *elem;
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
    fn fails_read_all_the_data_when_limit_is_reached() {
        let content = b"alexander_wonderbat";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(5, reader));

        let mut binding = buffer.lock().unwrap();

        let mut read_data = Vec::new();

        let read_length = binding.read_all(&mut read_data, Some(2));
        assert!(matches!(read_length, Err(_)))
    }

    #[test]
    fn can_read_all_the_data() {
        let content = b"alexander_wonderbat";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(5, reader));

        let mut binding = buffer.lock().unwrap();

        let mut read_data = Vec::new();

        let read_length = binding
            .read_all(&mut read_data, None)
            .expect("read data successfully");
        assert_eq!(read_length, content.len());

        let data_4_str = str::from_utf8(&read_data[0..read_length]).expect("convert into string");
        assert_eq!(data_4_str, "alexander_wonderbat");
    }

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
        let read = binding.next_line(&mut new_line);

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
            binding.next_until(SIGNAL).expect("should peek"),
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
        const SIGNAL: &[u8] = b"_";

        let content = b"alexander_wonderbat";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

        let mut binding = buffer.lock().unwrap();
        let result = binding.next_until(SIGNAL);
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
        const SIGNAL: &[u8] = b"-";

        let content = b"alexander_wonderbat";

        let reader = OwnedReader::Sync(Arc::new(Mutex::new(Cursor::new(content.to_vec()))));
        let buffer = Mutex::new(ByteBufferPointer::new(10, reader));

        let mut binding = buffer.lock().unwrap();
        let result = binding.next_until(SIGNAL);
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

    /// `get_ref` returns the reference to the wrapped `Cursor<T>`.
    pub fn get_ref(&self) -> &Cursor<T> {
        &self.0
    }

    /// `get_mut` returns the mutable reference to the wrapped `Cursor<T>`.
    pub fn get_mut(&mut self) -> &mut Cursor<T> {
        &mut self.0
    }

    /// `get_inner_mut` returns a immutable reference to the  inner content
    /// of the wrapped `Cursor<T>`.
    pub fn get_inner_ref(&self) -> &T {
        self.0.get_ref()
    }

    /// `get_inner_mut` returns a mutable reference to the  inner content
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
