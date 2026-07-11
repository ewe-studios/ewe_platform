//! `BrotliCompressor` — Brotli via the `brotli` crate (Decision 06, `brotli` feature).

use std::io::Read;

use super::{read_to_limit, CompressionError, Compressor};

const NAME: &str = "br";
const DEFAULT_QUALITY: u32 = 4;
const LG_WINDOW: u32 = 22;
const BUF_SIZE: usize = 4096;

/// The Brotli compressor. Default quality 4.
#[derive(Debug, Clone, Copy)]
pub struct BrotliCompressor {
    quality: u32,
}

impl BrotliCompressor {
    /// Create a brotli compressor at the given quality (0–11).
    #[must_use]
    pub fn new(quality: u32) -> Self {
        Self { quality }
    }
}

impl Default for BrotliCompressor {
    fn default() -> Self {
        Self {
            quality: DEFAULT_QUALITY,
        }
    }
}

impl Compressor for BrotliCompressor {
    fn name(&self) -> &str {
        NAME
    }

    fn compress(&self, input: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let mut out = Vec::new();
        let mut reader = brotli::CompressorReader::new(input, BUF_SIZE, self.quality, LG_WINDOW);
        reader
            .read_to_end(&mut out)
            .map_err(|e| CompressionError::Compress {
                algorithm: NAME,
                message: e.to_string(),
            })?;
        Ok(out)
    }

    fn decompress(&self, input: &[u8], max_bytes: usize) -> Result<Vec<u8>, CompressionError> {
        read_to_limit(brotli::Decompressor::new(input, BUF_SIZE), max_bytes, NAME)
    }
}
