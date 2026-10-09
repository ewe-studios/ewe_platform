//! Cryptographic key generation and encryption primitives.
//!
//! Provides a generic `EncryptionKey<N>` backed by vendored entropy
//! (wasm-safe, no getrandom dependency) plus algorithm-specific modules.
//! Sensitive buffers use [`zeroize`] to clear memory on drop.

mod key;

#[cfg(feature = "crypto-chacha")]
pub mod chacha;

pub use key::{EncryptionKey, SecureBytes, SecureString, fill_random, generate_nonce};
