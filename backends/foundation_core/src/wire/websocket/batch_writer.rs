//! Batch frame writer for WebSocket frames.
//!
//! WHY: Sending frames individually incurs syscall overhead for each write/flush.
//! Batching multiple frames reduces syscall count and improves throughput.
//!
//! WHAT: `BatchFrameWriter` accumulates encoded frames and flushes them in batches.
//!
//! HOW: Tracks accumulated bytes, flushes when batch size limit or timeout is reached.
//!
//! # RFC 6455 Compliance
//!
//! - Frames are encoded per Section 5.2 before batching
//! - Flush ordering preserves frame order
//! - Control frames can be prioritized for immediate flush

use super::error::WebSocketError;
use super::frame::WebSocketFrame;
use std::io::Write;
use std::time::{Duration, Instant};

/// Default batch size limit (16 KiB).
///
/// WHY: Balances latency vs. throughput. Large enough to amortize syscall
/// overhead, small enough to avoid excessive buffering.
const DEFAULT_BATCH_SIZE_BYTES: usize = 16 * 1024;

/// Default flush timeout (10ms).
///
/// WHY: Prevents excessive latency for small messages while allowing
/// batch accumulation.
const DEFAULT_FLUSH_TIMEOUT: Duration = Duration::from_millis(10);

/// Batch frame writer for reducing syscall overhead.
///
/// WHY: High-throughput WebSocket applications benefit from batching
/// multiple small frames into fewer, larger writes.
///
/// WHAT: Accumulates encoded frames and flushes them when:
/// - Batch size limit is reached
/// - Flush timeout expires
/// - Explicit flush is requested
///
/// HOW: Maintains an internal buffer, appends encoded frames, flushes
/// to the underlying writer when thresholds are met.
///
/// # Examples
///
/// ```ignore
/// use foundation_core::wire::websocket::{BatchFrameWriter, WebSocketFrame, Opcode};
/// use std::time::Duration;
///
/// let mut writer = BatchFrameWriter::new(
///     &mut my_stream,  // Your TCP stream
///     16 * 1024,  // 16 KiB batch size
///     Duration::from_millis(10),  // 10ms flush timeout
/// );
///
/// // Queue frames for batched writing
/// writer.queue_frame(my_frame)?;
///
/// // Flush when ready (or wait for automatic flush on timeout/size)
/// writer.flush()?;
/// ```
pub struct BatchFrameWriter<W> {
    /// The underlying writer (e.g., TCP stream).
    inner: W,
    /// Accumulated encoded frame data.
    buffer: Vec<u8>,
    /// Maximum batch size in bytes before automatic flush.
    max_batch_size: usize,
    /// Maximum time to wait before flushing accumulated data.
    flush_timeout: Duration,
    /// Time when the current batch started accumulating.
    batch_start: Option<Instant>,
    /// Number of frames queued in the current batch.
    frame_count: usize,
    /// Statistics: total frames written.
    total_frames_written: usize,
    /// Statistics: total flushes performed.
    total_flushes: usize,
    /// Statistics: bytes written (excluding protocol overhead).
    total_bytes_written: usize,
}

impl<W> BatchFrameWriter<W>
where
    W: Write,
{
    /// Create a new batch frame writer with default limits.
    ///
    /// # Arguments
    ///
    /// * `inner` - The underlying writer (e.g., TCP stream)
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_core::wire::websocket::BatchFrameWriter;
    ///
    /// let mut writer = BatchFrameWriter::with_defaults(&mut my_stream);
    /// ```
    pub fn with_defaults(inner: W) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(DEFAULT_BATCH_SIZE_BYTES),
            max_batch_size: DEFAULT_BATCH_SIZE_BYTES,
            flush_timeout: DEFAULT_FLUSH_TIMEOUT,
            batch_start: None,
            frame_count: 0,
            total_frames_written: 0,
            total_flushes: 0,
            total_bytes_written: 0,
        }
    }

    /// Create a new batch frame writer with custom limits.
    ///
    /// # Arguments
    ///
    /// * `inner` - The underlying writer
    /// * `max_batch_size` - Maximum bytes to accumulate before flush
    /// * `flush_timeout` - Maximum time to wait before flushing
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_core::wire::websocket::BatchFrameWriter;
    /// use std::time::Duration;
    ///
    /// let writer = BatchFrameWriter::new(
    ///     &mut my_stream,  // Your TCP stream
    ///     32 * 1024,  // 32 KiB
    ///     Duration::from_millis(5),  // 5ms
    /// );
    /// ```
    pub fn new(inner: W, max_batch_size: usize, flush_timeout: Duration) -> Self {
        Self {
            inner,
            buffer: Vec::with_capacity(max_batch_size),
            max_batch_size,
            flush_timeout,
            batch_start: None,
            frame_count: 0,
            total_frames_written: 0,
            total_flushes: 0,
            total_bytes_written: 0,
        }
    }

    /// Check if the batch buffer needs flushing.
    ///
    /// WHY: Determines whether accumulated frames should be written now.
    ///
    /// WHAT: Returns true if size limit or timeout threshold is exceeded.
    ///
    /// # Panics
    /// Never panics.
    fn needs_flush(&self) -> bool {
        // Flush if buffer is at/over capacity
        if self.buffer.len() >= self.max_batch_size {
            return true;
        }

        // Flush if timeout has elapsed since batch started
        if let Some(start) = self.batch_start {
            if start.elapsed() >= self.flush_timeout {
                return true;
            }
        }

        false
    }

    /// Queue a frame for batched writing.
    ///
    /// WHY: Applications queue multiple frames before flushing.
    ///
    /// WHAT: Encodes the frame and adds it to the buffer.
    /// Automatically flushes if batch size limit is reached.
    ///
    /// # Arguments
    ///
    /// * `frame` - The WebSocket frame to queue
    ///
    /// # Errors
    ///
    /// Returns `WebSocketError::IoError` if the underlying write fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn queue_frame(&mut self, frame: WebSocketFrame) -> Result<(), WebSocketError> {
        let encoded = frame.encode();
        let encoded_len = encoded.len();

        // Start timing the batch if this is the first frame
        if self.batch_start.is_none() {
            self.batch_start = Some(Instant::now());
        }

        // Check if adding this frame would exceed the batch limit
        // If so, flush first then add the new frame
        if self.buffer.len() + encoded_len > self.max_batch_size && !self.buffer.is_empty() {
            self.flush()?;
            // Restart timing for the new batch
            self.batch_start = Some(Instant::now());
        }

        // Add frame to buffer
        self.buffer.extend_from_slice(&encoded);
        self.frame_count += 1;
        self.total_frames_written += 1;
        self.total_bytes_written += encoded_len;

        // Check if we need to flush after adding this frame
        if self.needs_flush() {
            self.flush()?;
        }

        Ok(())
    }

    /// Queue multiple frames for batched writing.
    ///
    /// WHY: Efficiently batch multiple frames at once.
    ///
    /// WHAT: Queues each frame, flushing only when batch limits are reached.
    ///
    /// # Arguments
    ///
    /// * `frames` - Iterator of WebSocket frames to queue
    ///
    /// # Errors
    ///
    /// Returns `WebSocketError::IoError` if the underlying write fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn queue_frames<I>(&mut self, frames: I) -> Result<(), WebSocketError>
    where
        I: IntoIterator<Item = WebSocketFrame>,
    {
        for frame in frames {
            self.queue_frame(frame)?;
        }
        Ok(())
    }

    /// Flush accumulated frames to the underlying writer.
    ///
    /// WHY: Ensures queued frames are actually transmitted.
    ///
    /// WHAT: Writes the accumulated buffer and resets state.
    ///
    /// # Errors
    ///
    /// Returns `WebSocketError::IoError` if the underlying write/flush fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn flush(&mut self) -> Result<(), WebSocketError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        // Write accumulated data
        self.inner
            .write_all(&self.buffer)
            .map_err(WebSocketError::IoError)?;
        self.inner.flush().map_err(WebSocketError::IoError)?;

        // Update statistics
        self.total_flushes += 1;

        // Reset batch state
        self.buffer.clear();
        self.frame_count = 0;
        self.batch_start = None;

        Ok(())
    }

    /// Write a single frame immediately (bypassing batching).
    ///
    /// WHY: Control frames or urgent messages may need immediate transmission.
    ///
    /// WHAT: Encodes and writes the frame directly, flushing any existing batch first.
    ///
    /// # Arguments
    ///
    /// * `frame` - The frame to write immediately
    ///
    /// # Errors
    ///
    /// Returns `WebSocketError::IoError` if the underlying write/flush fails.
    ///
    /// # Panics
    /// Never panics.
    pub fn write_immediate(&mut self, frame: WebSocketFrame) -> Result<(), WebSocketError> {
        // Flush any existing batch first to maintain ordering
        self.flush()?;

        // Write the frame immediately
        let encoded = frame.encode();
        self.inner
            .write_all(&encoded)
            .map_err(WebSocketError::IoError)?;
        self.inner.flush().map_err(WebSocketError::IoError)?;

        // Update statistics
        self.total_frames_written += 1;
        self.total_bytes_written += encoded.len();
        self.total_flushes += 1;

        Ok(())
    }

    /// Get the number of frames currently queued in the buffer.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_core::wire::websocket::BatchFrameWriter;
    ///
    /// let writer = BatchFrameWriter::with_defaults(&mut my_stream);
    /// assert_eq!(writer.queued_frame_count(), 0);
    /// ```
    #[must_use]
    pub fn queued_frame_count(&self) -> usize {
        self.frame_count
    }

    /// Get the current buffer size in bytes.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_core::wire::websocket::BatchFrameWriter;
    ///
    /// let writer = BatchFrameWriter::with_defaults(&mut my_stream);
    /// assert_eq!(writer.buffered_bytes(), 0);
    /// ```
    #[must_use]
    pub fn buffered_bytes(&self) -> usize {
        self.buffer.len()
    }

    /// Get writer statistics.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use foundation_core::wire::websocket::BatchFrameWriter;
    ///
    /// let writer = BatchFrameWriter::with_defaults(&mut my_stream);
    /// let stats = writer.stats();
    /// println!("Frames: {}, Flushes: {}, Bytes: {}", stats.frames, stats.flushes, stats.bytes);
    /// ```
    #[must_use]
    pub fn stats(&self) -> BatchWriterStats {
        BatchWriterStats {
            frames: self.total_frames_written,
            flushes: self.total_flushes,
            bytes: self.total_bytes_written,
            buffered_bytes: self.buffer.len(),
            queued_frames: self.frame_count,
        }
    }

    /// Consume the writer and return the inner writer.
    ///
    /// # Panics
    /// Never panics.
    pub fn into_inner(mut self) -> Result<W, WebSocketError> {
        self.flush()?;
        Ok(self.inner)
    }

    /// Get a mutable reference to the inner writer.
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }
}

/// Statistics for a batch frame writer.
///
/// WHY: Applications may want to monitor batching efficiency.
///
/// WHAT: Provides counts of frames, flushes, and bytes written.
#[derive(Debug, Clone, Copy, Default)]
pub struct BatchWriterStats {
    /// Total frames written since creation.
    pub frames: usize,
    /// Total flush operations performed.
    pub flushes: usize,
    /// Total bytes written (encoded frame data).
    pub bytes: usize,
    /// Currently buffered bytes awaiting flush.
    pub buffered_bytes: usize,
    /// Currently queued frames awaiting flush.
    pub queued_frames: usize,
}

impl BatchWriterStats {
    /// Calculate the average batch size (bytes per flush).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn avg_batch_size(&self) -> f64 {
        if self.flushes == 0 {
            0.0
        } else {
            self.bytes as f64 / self.flushes as f64
        }
    }

    /// Calculate the flush efficiency (frames per flush).
    ///
    /// # Panics
    /// Never panics.
    #[must_use]
    pub fn flush_efficiency(&self) -> f64 {
        if self.flushes == 0 {
            0.0
        } else {
            self.frames as f64 / self.flushes as f64
        }
    }
}
