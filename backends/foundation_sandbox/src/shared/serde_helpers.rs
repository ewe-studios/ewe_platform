//! Serde helpers for the shared types.
//!
//! A `Duration` field on the wire is an integer number of milliseconds. Bytes
//! on the wire are base64 text.

use std::time::Duration;

use base64::engine::general_purpose::STANDARD;
use serde::{Deserialize, Deserializer, Serializer};

/// Serializes a `Duration` as integer milliseconds.
pub mod duration_millis {
    use super::{Deserialize, Deserializer, Duration, Serializer};

    /// Writes the duration as integer milliseconds.
    ///
    /// # Errors
    /// Returns the serializer error.
    pub fn serialize<S>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // A millisecond count above `u64::MAX` is not reachable in practice.
        serializer.serialize_u64(u64::try_from(value.as_millis()).unwrap_or(u64::MAX))
    }

    /// Reads integer milliseconds.
    ///
    /// # Errors
    /// Returns the deserializer error.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

/// Serializes an optional `Duration` as integer milliseconds, or `null`.
pub mod duration_millis_option {
    use super::{Deserialize, Deserializer, Duration, Serializer};

    /// Writes the duration as integer milliseconds, or `null`.
    ///
    /// # Errors
    /// Returns the serializer error.
    pub fn serialize<S>(value: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(value) => {
                serializer.serialize_some(&u64::try_from(value.as_millis()).unwrap_or(u64::MAX))
            }
            None => serializer.serialize_none(),
        }
    }

    /// Reads integer milliseconds, or `null`.
    ///
    /// # Errors
    /// Returns the deserializer error.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = Option::<u64>::deserialize(deserializer)?;
        Ok(millis.map(Duration::from_millis))
    }
}

/// Serializes bytes as base64 text.
pub mod base64_bytes {
    use base64::Engine as _;

    use super::{Deserialize, Deserializer, Serializer, STANDARD};

    /// Writes the bytes as base64 text.
    ///
    /// # Errors
    /// Returns the serializer error.
    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    /// Reads base64 text into bytes.
    ///
    /// # Errors
    /// Returns the deserializer error when the text is not base64.
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let text = String::deserialize(deserializer)?;
        STANDARD
            .decode(text.as_bytes())
            .map_err(serde::de::Error::custom)
    }
}
