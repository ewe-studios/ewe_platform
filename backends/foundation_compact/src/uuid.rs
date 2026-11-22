//! Version four universally unique identifiers (UUIDs).
//!
//! Use [`UuidV4::from_entropy`] with the entropy source from the host context.
//! Use [`new_v4`] for platform entropy.

use ::uuid::Builder;

pub use ::uuid::Uuid;

/// Generate a random version-4 UUID from the platform entropy source.
///
/// The 16 bytes come from [`crate::entropy::fill`]; [`Builder::from_random_bytes`]
/// sets the version-4 and RFC 4122 variant bits. The result is a plain value, so
/// callers that need the canonical string call `.to_string()`.
///
/// # Errors
///
/// Returns [`crate::entropy::Error`] if the platform entropy source is
/// unavailable.
pub fn new_v4() -> Result<Uuid, crate::entropy::Error> {
    Ok(UuidV4::from_entropy(crate::entropy::fill)?.0)
}

/// A version four UUID with the standard variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(try_from = "String", into = "String"))]
pub struct UuidV4(Uuid);

impl UuidV4 {
    /// Generate an identifier with the entropy source supplied by the caller.
    ///
    /// # Errors
    /// Returns the entropy source error.
    pub fn from_entropy<E>(fill: impl FnOnce(&mut [u8]) -> Result<(), E>) -> Result<Self, E> {
        let mut bytes = [0; 16];
        fill(&mut bytes)?;
        Ok(Self(Builder::from_random_bytes(bytes).into_uuid()))
    }
}

impl core::str::FromStr for UuidV4 {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = "Expected a version four UUID";
        let id = Uuid::try_parse(value).map_err(|_| invalid)?;
        if id.get_version_num() != 4 || id.get_variant() != ::uuid::Variant::RFC4122 {
            return Err(invalid);
        }
        Ok(Self(id))
    }
}

impl core::fmt::Display for UuidV4 {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<String> for UuidV4 {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<UuidV4> for String {
    fn from(value: UuidV4) -> Self {
        value.to_string()
    }
}
