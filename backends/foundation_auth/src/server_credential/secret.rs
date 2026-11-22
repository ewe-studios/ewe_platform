//! A redacted, zeroizing secret.

use std::fmt;

use derive_more::From;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zeroize::Zeroizing;

/// The scheme of a bearer credential, with its separating space (RFC 9110).
pub(crate) const BEARER_PREFIX: &[u8] = b"Bearer ";

/// A text that must stay private.
///
/// It is zeroized on drop. `Debug` and `Display` print `<redacted>`, so a log
/// line, an error message or a panic cannot show it. Comparison is
/// constant-time.
///
/// Use [`Secret::expose`] only where the text leaves the process. Every call
/// site is a place where the secret may leak; keep them few.
#[derive(Clone, From)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    /// A secret from a value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(Zeroizing::new(value.into()))
    }

    /// The text. Every call site is a place where the secret may leak; keep
    /// them few.
    #[must_use]
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    /// Whether the secret holds no text.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Constant-time equality.
    #[must_use]
    pub fn ct_eq(&self, other: &Secret) -> bool {
        crate::shared::constant_time_eq(self.expose().as_bytes(), other.expose().as_bytes())
    }

    /// The secret from an `Authorization: Bearer` value (RFC 9110).
    ///
    /// The scheme is case-insensitive. The token must be one or more visible
    /// ASCII characters (`0x21..=0x7E`), so a header with a control byte or a
    /// space in the token is refused and never reaches a verifier.
    ///
    /// # Errors
    ///
    /// `None` when the header is absent, when the scheme is not `Bearer`, or
    /// when the token is empty or not visible ASCII.
    #[must_use]
    pub fn from_bearer(header: Option<&str>) -> Option<Self> {
        let header = header?;
        let bytes = header.as_bytes();
        if bytes.len() < BEARER_PREFIX.len()
            || !bytes[..BEARER_PREFIX.len()].eq_ignore_ascii_case(BEARER_PREFIX)
        {
            return None;
        }
        let token = header.get(BEARER_PREFIX.len()..)?;
        if token.is_empty() || !token.bytes().all(|byte| (0x21..=0x7E).contains(&byte)) {
            return None;
        }
        Some(Self::new(token))
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other)
    }
}

impl Eq for Secret {}

impl Serialize for Secret {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.expose())
    }
}

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(Self::new(value))
    }
}

/// The old name of [`Secret`].
///
/// It is an alias for one release, then it is removed. Use `Secret` in new
/// code.
pub type ConfidentialText = Secret;
