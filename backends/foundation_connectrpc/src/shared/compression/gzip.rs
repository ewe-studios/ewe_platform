//! `GzipCompressor` — the default per-message compressor via flate2 (Decision 06).
//!
//! WHY: gzip is the ConnectRPC/gRPC default encoding and flate2 is already a
//! platform dependency.
//!
//! WHAT: [`GzipCompressor`] (batch [`Compressor`]) plus the RS10 reset-able
//! streaming [`GzipStreamCompressor`] for large/streamed payloads, which reuses
//! the encoder state across messages.
//!
//! HOW: `flate2::write::GzEncoder` / `read::GzDecoder`; decompression is bounded
//! by [`read_to_limit`](super::read_to_limit) to stop compression bombs.

use std::io::Write;

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

use super::{read_to_limit, CompressionError, Compressor};

const NAME: &str = "gzip";
const DEFAULT_LEVEL: u32 = 6;

/// The gzip compressor (flate2). Default level 6.
#[derive(Debug, Clone, Copy)]
pub struct GzipCompressor {
    level: u32,
}

impl GzipCompressor {
    /// Create a gzip compressor at the given flate2 level (0–9).
    #[must_use]
    pub fn new(level: u32) -> Self {
        Self { level }
    }
}

impl Default for GzipCompressor {
    fn default() -> Self {
        Self { level: DEFAULT_LEVEL }
    }
}

impl Compressor for GzipCompressor {
    fn name(&self) -> &str {
        NAME
    }

    fn compress(&self, input: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::new(self.level));
        encoder
            .write_all(input)
            .and_then(|()| encoder.finish())
            .map_err(|e| CompressionError::Compress {
                algorithm: NAME,
                message: e.to_string(),
            })
    }

    fn decompress(&self, input: &[u8], max_bytes: usize) -> Result<Vec<u8>, CompressionError> {
        read_to_limit(GzDecoder::new(input), max_bytes, NAME)
    }
}

/// RS10: a reset-able streaming gzip compressor. Feeding a message with
/// [`write`](Self::write) then [`finish_message`](Self::finish_message) yields
/// its compressed bytes and resets the encoder — reusing its compression state
/// and buffers for the next message instead of reallocating.
pub struct GzipStreamCompressor {
    level: Compression,
    encoder: GzEncoder<Vec<u8>>,
}

impl GzipStreamCompressor {
    /// Create a streaming compressor at the given level (0–9).
    #[must_use]
    pub fn new(level: u32) -> Self {
        let level = Compression::new(level);
        Self {
            level,
            encoder: GzEncoder::new(Vec::new(), level),
        }
    }

    /// Append data to the current message.
    ///
    /// # Errors
    /// Returns [`CompressionError::Compress`] on an underlying I/O error.
    pub fn write(&mut self, data: &[u8]) -> Result<(), CompressionError> {
        self.encoder.write_all(data).map_err(|e| CompressionError::Compress {
            algorithm: NAME,
            message: e.to_string(),
        })
    }

    /// Finish the current message and reset for the next, returning the
    /// compressed bytes. The streaming compressor is reusable across messages —
    /// feed the next message with [`write`](Self::write) after this returns.
    ///
    /// # Errors
    /// Returns [`CompressionError::Compress`] on an underlying I/O error.
    pub fn finish_message(&mut self) -> Result<Vec<u8>, CompressionError> {
        let finished = std::mem::replace(
            &mut self.encoder,
            GzEncoder::new(Vec::new(), self.level),
        );
        finished.finish().map_err(|e| CompressionError::Compress {
            algorithm: NAME,
            message: e.to_string(),
        })
    }
}

impl Default for GzipStreamCompressor {
    fn default() -> Self {
        Self::new(DEFAULT_LEVEL)
    }
}
