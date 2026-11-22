//! Identity of a Todo record across hosts, storage, and retries.
use foundation_compact::uuid::Uuid;
use foundation_context::{env::Env, Ctx};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

/// A version four universally unique identifier (UUID).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TodoId(Uuid);

impl TodoId {
    /// Generate an identifier using the entropy source installed by the host.
    ///
    /// # Errors
    /// Returns an error if the entropy source is missing or fails.
    pub fn generate(ctx: &Ctx) -> Result<Self, String> {
        let mut bytes = [0; 16];
        ctx.get::<Env>()
            .ok_or("Missing Todo entropy source")?
            .entropy()
            .fill(&mut bytes)
            .map_err(|error| error.to_string())?;
        bytes[6] = bytes[6] & 0x0f | 0x40;
        bytes[8] = bytes[8] & 0x3f | 0x80;
        Ok(Self(Uuid::from_bytes(bytes)))
    }
}

impl FromStr for TodoId {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = "Expected a version four Todo UUID";
        let id = Uuid::try_parse(value).map_err(|_| invalid)?;
        if id.get_version_num() != 4 || id.as_bytes()[8] & 0xc0 != 0x80 {
            return Err(invalid);
        }
        Ok(Self(id))
    }
}

impl fmt::Display for TodoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl TryFrom<String> for TodoId {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<TodoId> for String {
    fn from(value: TodoId) -> Self {
        value.to_string()
    }
}
