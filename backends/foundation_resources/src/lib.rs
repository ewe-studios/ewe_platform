//! Package application files under stable names.
#[cfg(feature = "build")]
pub mod build;

/// A portable relative resource name.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResourceName(String);

/// A resource name cannot identify a file outside the resource set.
#[derive(Clone, Debug, PartialEq, Eq, derive_more::Display, derive_more::Error)]
#[display("Invalid resource name: {name}")]
pub struct InvalidName {
    pub name: String,
}

impl ResourceName {
    /// Validate a logical path for a browser or mobile package.
    ///
    /// # Errors
    /// Returns an error for empty, relative, reserved, or nonportable components.
    pub fn new(name: &str) -> Result<Self, InvalidName> {
        if name.split('/').any(|part| {
            part.is_empty()
                || part == "."
                || part == ".."
                || part.ends_with([' ', '.'])
                || part.starts_with(".ewe-resource")
                || part
                    .chars()
                    .any(|c| c.is_control() || "\\:*?\"<>|#%".contains(c))
                || matches!(
                    part.split('.')
                        .next()
                        .unwrap_or("")
                        .to_ascii_uppercase()
                        .as_str(),
                    "CON"
                        | "PRN"
                        | "AUX"
                        | "NUL"
                        | "COM1"
                        | "COM2"
                        | "COM3"
                        | "COM4"
                        | "COM5"
                        | "COM6"
                        | "COM7"
                        | "COM8"
                        | "COM9"
                        | "LPT1"
                        | "LPT2"
                        | "LPT3"
                        | "LPT4"
                        | "LPT5"
                        | "LPT6"
                        | "LPT7"
                        | "LPT8"
                        | "LPT9"
                )
        }) {
            return Err(InvalidName {
                name: name.to_owned(),
            });
        }
        Ok(Self(name.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(feature = "runtime")]
mod runtime;
#[cfg(feature = "runtime")]
pub use runtime::*;

#[cfg(all(feature = "runtime", target_arch = "wasm32"))]
mod browser;
#[cfg(all(feature = "runtime", target_arch = "wasm32"))]
pub use browser::BrowserReader;

#[cfg(all(feature = "browser-bindings", target_arch = "wasm32"))]
pub use browser::{BrowserResourceStream, BrowserResources};

#[cfg(all(feature = "server", not(target_family = "wasm")))]
pub mod server;
