//! Generic fixed-size encryption key with secure generation and zeroize-on-drop.

use crate::entropy;
use zeroize::Zeroizing;

/// A securely generated encryption key of `N` bytes.
///
/// Backed by vendored OS entropy — works on all targets including wasm32.
/// Memory is zeroed on drop via [`zeroize`].
///
/// Common sizes:
/// - `EncryptionKey<16>` — 128-bit (AES-128, etc.)
/// - `EncryptionKey<32>` — 256-bit (ChaCha20-Poly1305, AES-256, etc.)
/// - `EncryptionKey<64>` — 512-bit (HMAC-SHA512, etc.)
pub struct EncryptionKey<const N: usize>(Zeroizing<[u8; N]>);

impl<const N: usize> EncryptionKey<N> {
    /// Generate a new random key from the platform entropy source.
    ///
    /// # Panics
    ///
    /// Panics if the platform entropy source is unavailable.
    #[must_use]
    pub fn generate() -> Self {
        let mut bytes = [0u8; N];
        entropy::fill(&mut bytes).expect("platform entropy source unavailable");
        Self(Zeroizing::new(bytes))
    }

    /// Create a key from raw bytes. The input array is moved into a
    /// zeroize-on-drop wrapper.
    #[must_use]
    pub fn from_bytes(bytes: [u8; N]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// Borrow the raw key bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; N] {
        &self.0
    }

    /// Key length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        N
    }

    /// Always false — keys have a fixed nonzero size.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        N == 0
    }
}

impl<const N: usize> Clone for EncryptionKey<N> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<const N: usize> core::fmt::Debug for EncryptionKey<N> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "EncryptionKey<{N}>([REDACTED])")
    }
}

/// A `Vec<u8>` that is zeroed on drop — use for plaintext, decrypted
/// payloads, or any sensitive byte buffer.
pub type SecureBytes = Zeroizing<Vec<u8>>;

/// A `String` that is zeroed on drop — use for passwords, tokens,
/// or any sensitive text.
pub type SecureString = Zeroizing<String>;

/// Fill a buffer with random bytes from the platform entropy source.
///
/// # Errors
///
/// Returns the platform entropy error if the source is unavailable.
pub fn fill_random(dest: &mut [u8]) -> Result<(), entropy::Error> {
    entropy::fill(dest)
}

/// Generate a random nonce/IV of `N` bytes.
///
/// # Panics
///
/// Panics if the platform entropy source is unavailable.
#[must_use]
pub fn generate_nonce<const N: usize>() -> [u8; N] {
    let mut nonce = [0u8; N];
    entropy::fill(&mut nonce).expect("platform entropy source unavailable");
    nonce
}
