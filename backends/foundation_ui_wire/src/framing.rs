//! Read and write verified frames at a stream or message boundary.
use crate::{read_frame, Frame, WireError, WireLimits};
use futures_util::{Stream, StreamExt};
use std::{
    fmt,
    io::{self, BufRead, BufReader, Read, Write},
    pin::Pin,
};

/// The original bytes of a verified frame.
#[derive(Debug)]
pub struct OwnedFrame(Vec<u8>);

impl OwnedFrame {
    /// Checks the complete frame once and retains the supplied bytes.
    ///
    /// # Errors
    /// Returns the wire verifier error for an invalid frame.
    pub fn new(bytes: Vec<u8>, limits: &WireLimits) -> Result<Self, WireError> {
        read_frame(&bytes, limits)?;
        Ok(Self(bytes))
    }

    /// Borrows the verified table from this frame.
    #[must_use]
    pub fn view(&self) -> Frame<'_> {
        // SAFETY: The constructor verifies the same frame type. The bytes remain immutable.
        unsafe { flatbuffers::root_unchecked::<Frame<'_>>(&self.0) }
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

/// Selects the boundary around a frame.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Framing {
    /// The source contains one frame. End of input marks its end.
    Raw,
    /// Each frame starts with its four-byte little-endian size.
    LengthPrefixed,
}

/// Identifies the failing boundary, source, size check, or frame.
#[derive(Debug)]
pub enum FramingError {
    TruncatedPrefix { read: usize },
    TruncatedPayload { expected: usize, read: usize },
    Limit { limit: usize, actual: usize },
    Wire(WireError),
    Source(io::Error),
}

impl fmt::Display for FramingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TruncatedPrefix { read } => {
                write!(f, "Frame length ended after {read} of 4 bytes")
            }
            Self::TruncatedPayload { expected, read } => {
                write!(f, "Frame ended after {read} of {expected} bytes")
            }
            Self::Limit { limit, actual } => {
                write!(f, "Frame size {actual} exceeds the limit of {limit} bytes")
            }
            Self::Wire(err) => err.fmt(f),
            Self::Source(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for FramingError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Wire(err) => Some(err),
            Self::Source(err) => Some(err),
            _ => None,
        }
    }
}

impl From<WireError> for FramingError {
    fn from(error: WireError) -> Self {
        match error {
            WireError::TooLarge { limit, actual } => Self::Limit { limit, actual },
            other => Self::Wire(other),
        }
    }
}

/// The adapters retain surplus input. The decoder retains only the incomplete frame.
struct Decoder {
    framing: Framing,
    limits: WireLimits,
    prefix: [u8; 4],
    prefix_len: usize,
    payload: Vec<u8>,
}

impl Decoder {
    fn new(framing: Framing, limits: WireLimits) -> Self {
        Self {
            framing,
            limits,
            prefix: [0; 4],
            prefix_len: 0,
            payload: Vec::new(),
        }
    }

    fn check_size(&self, actual: usize) -> Result<(), FramingError> {
        let limit = self.limits.frame_bytes();
        if actual > limit {
            return Err(FramingError::Limit { actual, limit });
        }
        Ok(())
    }

    fn feed(&mut self, input: &mut &[u8]) -> Result<Option<OwnedFrame>, FramingError> {
        if self.framing == Framing::Raw {
            self.check_size(self.payload.len().saturating_add(input.len()))?;
            self.payload.extend_from_slice(input);
            *input = &[];
            return Ok(None);
        }
        if self.prefix_len < self.prefix.len() {
            let count = input.len().min(self.prefix.len() - self.prefix_len);
            self.prefix[self.prefix_len..self.prefix_len + count].copy_from_slice(&input[..count]);
            self.prefix_len += count;
            *input = &input[count..];
            if self.prefix_len < self.prefix.len() {
                return Ok(None);
            }
        }
        let expected = u32::from_le_bytes(self.prefix) as usize;
        self.check_size(expected)?;
        let count = input.len().min(expected - self.payload.len());
        self.payload.extend_from_slice(&input[..count]);
        *input = &input[count..];
        if self.payload.len() == expected {
            self.prefix_len = 0;
            return self.complete().map(Some);
        }
        Ok(None)
    }

    fn complete(&mut self) -> Result<OwnedFrame, FramingError> {
        OwnedFrame::new(std::mem::take(&mut self.payload), &self.limits).map_err(Into::into)
    }

    fn end(&mut self) -> Result<Option<OwnedFrame>, FramingError> {
        match (self.framing, self.prefix_len) {
            (Framing::Raw, _) => self.complete().map(Some),
            (_, 0) => Ok(None),
            (_, 4) => Err(FramingError::TruncatedPayload {
                expected: u32::from_le_bytes(self.prefix) as usize,
                read: self.payload.len(),
            }),
            (_, read) => Err(FramingError::TruncatedPrefix { read }),
        }
    }
}

/// A blocking iterator over a file or another byte source.
pub struct FrameReader<R> {
    input: BufReader<R>,
    decoder: Decoder,
    done: bool,
}

impl<R: Read> FrameReader<R> {
    #[must_use]
    pub fn new(reader: R, framing: Framing, limits: WireLimits) -> Self {
        Self {
            input: BufReader::new(reader),
            decoder: Decoder::new(framing, limits),
            done: false,
        }
    }
    #[must_use]
    pub fn raw(reader: R, limits: WireLimits) -> Self {
        Self::new(reader, Framing::Raw, limits)
    }
    #[must_use]
    pub fn length_prefixed(reader: R, limits: WireLimits) -> Self {
        Self::new(reader, Framing::LengthPrefixed, limits)
    }
}

impl<R: Read> Iterator for FrameReader<R> {
    type Item = Result<OwnedFrame, FramingError>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        loop {
            let result = match self.input.fill_buf() {
                Ok([]) => {
                    self.done = true;
                    self.decoder.end()
                }
                Ok(buffer) => {
                    let total = buffer.len();
                    let mut rest = buffer;
                    let parsed = self.decoder.feed(&mut rest);
                    let consumed = total - rest.len();
                    self.input.consume(consumed);
                    parsed
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => Err(FramingError::Source(e)),
            };
            match result {
                Ok(None) if !self.done => {}
                Err(error) => {
                    self.done = true;
                    return Some(Err(error));
                }
                frame => return frame.transpose(),
            }
        }
    }
}

/// An asynchronous reader over a fallible stream of byte chunks.
pub struct AsyncFrameReader<S> {
    input: Pin<Box<S>>,
    decoder: Decoder,
    chunk: Vec<u8>,
    offset: usize,
    done: bool,
}

impl<S> AsyncFrameReader<S> {
    #[must_use]
    pub fn new(input: S, framing: Framing, limits: WireLimits) -> Self {
        Self {
            input: Box::pin(input),
            decoder: Decoder::new(framing, limits),
            chunk: Vec::new(),
            offset: 0,
            done: false,
        }
    }
    #[must_use]
    pub fn raw(input: S, limits: WireLimits) -> Self {
        Self::new(input, Framing::Raw, limits)
    }
    #[must_use]
    pub fn length_prefixed(input: S, limits: WireLimits) -> Self {
        Self::new(input, Framing::LengthPrefixed, limits)
    }
}

impl<S, E> AsyncFrameReader<S>
where
    S: Stream<Item = Result<Vec<u8>, E>>,
    E: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    /// Waits for one frame, a source failure, or the end of the source.
    ///
    /// # Errors
    /// Reports a truncated boundary, a limit, a verifier error, or a source error.
    pub async fn next_frame(&mut self) -> Result<Option<OwnedFrame>, FramingError> {
        while !self.done {
            if self.offset < self.chunk.len() {
                let mut rest = &self.chunk[self.offset..];
                let result = self.decoder.feed(&mut rest);
                self.offset = self.chunk.len() - rest.len();
                if !matches!(result, Ok(None)) {
                    self.done = result.is_err();
                    return result;
                }
            }
            match self.input.next().await {
                Some(Ok(chunk)) => {
                    self.chunk = chunk;
                    self.offset = 0;
                }
                Some(Err(err)) => {
                    self.done = true;
                    return Err(FramingError::Source(io::Error::other(err)));
                }
                None => {
                    self.done = true;
                    return self.decoder.end();
                }
            }
        }
        Ok(None)
    }
}

/// Writes verified frames to a sink. Raw output contains one complete frame.
pub struct StreamFrameWriter<W> {
    out: W,
    framing: Framing,
    finished: bool,
}

impl<W: Write> StreamFrameWriter<W> {
    #[must_use]
    pub fn new(out: W, framing: Framing) -> Self {
        Self {
            out,
            framing,
            finished: false,
        }
    }
    #[must_use]
    pub fn raw(out: W) -> Self {
        Self::new(out, Framing::Raw)
    }
    #[must_use]
    pub fn length_prefixed(out: W) -> Self {
        Self::new(out, Framing::LengthPrefixed)
    }

    /// Writes one complete frame. A failed write closes this writer.
    ///
    /// # Errors
    /// Reports a closed writer, an unsupported size, or the sink error.
    pub fn write_frame(&mut self, frame: &OwnedFrame) -> io::Result<()> {
        if self.finished {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Frame writer is closed",
            ));
        }
        self.finished = true;
        if self.framing == Framing::LengthPrefixed {
            let size = u32::try_from(frame.as_bytes().len()).map_err(io::Error::other)?;
            self.out.write_all(&size.to_le_bytes())?;
        }
        self.out.write_all(frame.as_bytes())?;
        self.finished = self.framing == Framing::Raw;
        Ok(())
    }
}
