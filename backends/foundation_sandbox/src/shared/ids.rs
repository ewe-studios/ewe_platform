//! Identifiers and the Unix domain socket path limit.

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use foundation_compact::rng::Rng;
use foundation_context::Ctx;
use serde::{Deserialize, Serialize};

use crate::shared::error::{SandboxError, SandboxResult};

macro_rules! hex_id {
    ($(#[$meta:meta])* $name:ident, $digits:literal, $mask:expr, $field:literal) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            /// A new identifier from the random number generator of `ctx`.
            #[must_use]
            pub fn generate(ctx: &Ctx) -> Self {
                let value = ctx
                    .rng()
                    .lock()
                    .expect("the Ctx RNG lock is not poisoned")
                    .next_u64()
                    & $mask;
                Self(format!("{:0width$x}", value, width = $digits))
            }

            /// The identifier as lower-case hexadecimal text.
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = SandboxError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                let shape_is_valid = value.len() == $digits
                    && value
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'));
                if !shape_is_valid {
                    return Err(SandboxError::InvalidSpec {
                        field: $field.to_string(),
                        reason: format!(
                            "must be {} lower-case hexadecimal characters",
                            $digits
                        ),
                    });
                }
                Ok(Self(value.to_string()))
            }
        }

        impl TryFrom<String> for $name {
            type Error = SandboxError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                value.parse()
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

hex_id!(
    /// A sandbox identifier: 12 lower-case hexadecimal characters.
    SandboxId,
    12,
    0x0000_ffff_ffff_ffff,
    "sandbox_id"
);

hex_id!(
    /// An execution identifier: 16 lower-case hexadecimal characters.
    ExecId,
    16,
    u64::MAX,
    "exec_id"
);

hex_id!(
    /// An upload identifier: 16 lower-case hexadecimal characters.
    UploadId,
    16,
    u64::MAX,
    "upload_id"
);

/// The driver that runs a substrate.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DriverId {
    /// The Firecracker microVM manager.
    Firecracker,
    /// The libkrun microVM library.
    Libkrun,
    /// The Docker Engine.
    Docker,
    /// A Cloudflare container.
    CloudflareContainer,
    /// A Cloudflare Durable Object that serves the slave itself: the turn runs
    /// against `OpenRouter` or the Cloudflare Model API. It gives no operating
    /// system isolation (spec 77 decision D16).
    CloudflareDo,
}

impl fmt::Display for DriverId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Firecracker => "firecracker",
            Self::Libkrun => "libkrun",
            Self::Docker => "docker",
            Self::CloudflareContainer => "cloudflare-container",
            Self::CloudflareDo => "cloudflare-do",
        };
        formatter.write_str(name)
    }
}

impl FromStr for DriverId {
    type Err = SandboxError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "firecracker" => Ok(Self::Firecracker),
            "libkrun" => Ok(Self::Libkrun),
            "docker" => Ok(Self::Docker),
            "cloudflare-container" => Ok(Self::CloudflareContainer),
            "cloudflare-do" => Ok(Self::CloudflareDo),
            other => Err(SandboxError::InvalidSpec {
                field: "driver".to_string(),
                reason: format!("unknown driver {other}"),
            }),
        }
    }
}

/// The longest Unix domain socket path in bytes, without the end zero byte.
///
/// The kernel limit is 104 bytes on macOS and 108 bytes on Linux, and the end
/// zero byte takes one. 103 bytes fits both.
pub const MAX_SOCKET_PATH_BYTES: usize = 103;

/// Refuses a Unix domain socket path that is too long for the kernel.
///
/// # Errors
/// [`SandboxError::SocketPathTooLong`] when the path is longer than
/// [`MAX_SOCKET_PATH_BYTES`] bytes.
pub fn check_socket_path(path: &Path) -> SandboxResult<()> {
    let length = path.as_os_str().as_encoded_bytes().len();
    if length > MAX_SOCKET_PATH_BYTES {
        return Err(SandboxError::SocketPathTooLong {
            path: path.display().to_string(),
            max: MAX_SOCKET_PATH_BYTES,
        }
        .into());
    }
    Ok(())
}
