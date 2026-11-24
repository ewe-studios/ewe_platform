//! Where a private key comes from.

use base64::Engine;
use foundation_compact::crypto::SecureBytes;

#[cfg(not(target_family = "wasm"))]
use super::KeyLoadError;

/// Where a private key comes from. The operator makes the key; the host reads
/// it one time, at start.
///
/// The enum has the same shape on every target, and so does the read. A file is
/// read through the file system that the host installs (the virtual file
/// system (VFS) trait of `foundation_nativeapis`), never through `std::fs`. A
/// host with no file system, such as a Worker, gets
/// [`KeyLoadError::NotFound`] for `File` and takes the key as an inline value
/// or a Worker secret.
#[derive(Clone)]
pub enum KeySource {
    /// A path in the host's file system. On Unix the host refuses a file that
    /// the group or others can read, as OpenSSH does ("UNPROTECTED PRIVATE KEY
    /// FILE").
    ///
    /// The path is a UTF-8 string because the VFS trait takes a string. A
    /// native host converts its path flag value once, at the boundary.
    File(String),
    /// The key text from an environment variable or a Worker secret: the P-
    /// Enhanced Mail (PEM) text, or the PEM text in standard base64 (for a
    /// system that cannot keep line breaks in a variable).
    Inline(SecureBytes),
}

impl std::fmt::Debug for KeySource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::File(path) => f.debug_tuple("KeySource::File").field(path).finish(),
            Self::Inline(_) => f.write_str("KeySource::Inline(<redacted>)"),
        }
    }
}

impl KeySource {
    /// An inline key text.
    #[must_use]
    pub fn inline(text: impl AsRef<[u8]>) -> Self {
        Self::Inline(SecureBytes::new(text.as_ref().to_vec()))
    }

    /// Reads an environment variable as the native host does with
    /// `EWE_AGENT_RPC_SIGNING_KEY`: a value that starts with `-----BEGIN` is
    /// PEM; a value that decodes from base64 to PEM is base64; any other value
    /// is a file path.
    ///
    /// # Errors
    /// [`KeyLoadError::NotFound`] when the variable is not set or is empty.
    #[cfg(not(target_family = "wasm"))]
    pub fn from_env_var(name: &str) -> Result<Self, KeyLoadError> {
        let value = std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| KeyLoadError::NotFound(name.to_string()))?;
        Ok(Self::from_value(&value))
    }

    /// A key source from a variable value: PEM, base64 PEM, or a file path.
    ///
    /// Use it for a Worker secret or a configuration value, which holds the PEM
    /// text or the PEM text in standard base64. A path is the native form; a
    /// host with no file system turns it into `KeyLoadError::NotFound` at load
    /// time.
    #[must_use]
    pub fn from_value(value: &str) -> Self {
        let trimmed = value.trim();
        if trimmed.starts_with("-----BEGIN") {
            return Self::inline(trimmed);
        }
        if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(trimmed) {
            if decoded.starts_with(b"-----BEGIN") {
                return Self::Inline(SecureBytes::new(decoded));
            }
        }
        Self::File(trimmed.to_string())
    }
}
