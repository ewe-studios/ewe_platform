//! `ZstdCompressor` — Zstandard via the `zstd` crate (Decision 06, `zstd` feature).

use super::{read_to_limit, CompressionError, Compressor};

const NAME: &str = "zstd";
const DEFAULT_LEVEL: i32 = 3;

/// The Zstandard compressor. Default level 3.
#[derive(Debug, Clone, Copy)]
pub struct ZstdCompressor {
    level: i32,
}

impl ZstdCompressor {
    /// Create a zstd compressor at the given level.
    #[must_use]
    pub fn new(level: i32) -> Self {
        Self { level }
    }
}

impl Default for ZstdCompressor {
    fn default() -> Self {
        Self { level: DEFAULT_LEVEL }
    }
}

impl Compressor for ZstdCompressor {
    fn name(&self) -> &str {
        NAME
    }

    fn compress(&self, input: &[u8]) -> Result<Vec<u8>, CompressionError> {
        zstd::stream::encode_all(input, self.level).map_err(|e| CompressionError::Compress {
            algorithm: NAME,
            message: e.to_string(),
        })
    }

    fn decompress(&self, input: &[u8], max_bytes: usize) -> Result<Vec<u8>, CompressionError> {
        let decoder = zstd::stream::read::Decoder::new(input).map_err(|e| {
            CompressionError::Decompress {
                algorithm: NAME,
                message: e.to_string(),
            }
        })?;
        read_to_limit(decoder, max_bytes, NAME)
    }
}
