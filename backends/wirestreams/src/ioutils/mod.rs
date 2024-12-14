use std::convert::TryFrom;
use std::convert::TryInto;
use std::io::{BufRead, BufReader, BufWriter, IoSlice, IoSliceMut, Read, Result, Write};

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
