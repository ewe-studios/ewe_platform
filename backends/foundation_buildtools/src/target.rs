//! Target-triple parsing — extracted from `infrastructure/llama-bindings/build.rs`.

use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsVariant {
    Msvc,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppleVariant {
    MacOS,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetOs {
    Windows(WindowsVariant),
    Apple(AppleVariant),
    Linux,
    Android,
    Wasm(super::WasmFlavor),
}

impl TargetOs {
    #[must_use]
    pub fn is_wasm(self) -> bool {
        matches!(self, Self::Wasm(_))
    }

    #[must_use]
    pub fn is_native(self) -> bool {
        !self.is_wasm()
    }
}

pub struct TargetInfo {
    pub os: TargetOs,
    pub triple: String,
}

impl TargetInfo {
    /// Parse the Cargo `TARGET` env var (available in `build.rs`).
    ///
    /// # Errors
    /// Returns the raw triple string if the target OS cannot be determined.
    ///
    /// # Panics
    /// Panics if `TARGET` is not set (should never happen inside a build script).
    pub fn from_env() -> Result<Self, String> {
        let triple = env::var("TARGET").expect("TARGET env var must be set in build.rs");
        Self::from_triple(&triple)
    }

    /// # Errors
    /// Returns the raw triple string if the target OS cannot be determined.
    pub fn from_triple(triple: &str) -> Result<Self, String> {
        let os = parse_os(triple)?;
        Ok(Self {
            os,
            triple: triple.to_string(),
        })
    }

    #[must_use]
    pub fn is_wasm(&self) -> bool {
        self.os.is_wasm()
    }

    #[must_use]
    pub fn is_native(&self) -> bool {
        self.os.is_native()
    }
}

fn parse_os(triple: &str) -> Result<TargetOs, String> {
    if triple.contains("wasm") {
        return Ok(TargetOs::Wasm(super::WasmFlavor::from_triple(triple)));
    }

    if triple.contains("windows") {
        return if triple.ends_with("-windows-msvc") {
            Ok(TargetOs::Windows(WindowsVariant::Msvc))
        } else {
            Ok(TargetOs::Windows(WindowsVariant::Other))
        };
    }

    if triple.contains("apple") {
        return if triple.ends_with("-apple-darwin") {
            Ok(TargetOs::Apple(AppleVariant::MacOS))
        } else {
            Ok(TargetOs::Apple(AppleVariant::Other))
        };
    }

    if triple.contains("android") {
        return Ok(TargetOs::Android);
    }

    if triple.contains("linux") {
        return Ok(TargetOs::Linux);
    }

    Err(triple.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_triples() {
        let info = TargetInfo::from_triple("x86_64-unknown-linux-gnu").unwrap();
        assert!(matches!(info.os, TargetOs::Linux));
        assert!(info.is_native());

        let info = TargetInfo::from_triple("x86_64-pc-windows-msvc").unwrap();
        assert!(matches!(info.os, TargetOs::Windows(WindowsVariant::Msvc)));

        let info = TargetInfo::from_triple("aarch64-apple-darwin").unwrap();
        assert!(matches!(info.os, TargetOs::Apple(AppleVariant::MacOS)));

        let info = TargetInfo::from_triple("aarch64-apple-ios").unwrap();
        assert!(matches!(info.os, TargetOs::Apple(AppleVariant::Other)));

        let info = TargetInfo::from_triple("aarch64-linux-android").unwrap();
        assert!(matches!(info.os, TargetOs::Android));
    }

    #[test]
    fn wasm_triples() {
        let info = TargetInfo::from_triple("wasm32-unknown-unknown").unwrap();
        assert!(info.is_wasm());
        assert!(matches!(
            info.os,
            TargetOs::Wasm(super::super::WasmFlavor::UnknownUnknown)
        ));
    }

    #[test]
    fn unknown_triple_errors() {
        assert!(TargetInfo::from_triple("riscv64gc-unknown-none-elf").is_err());
    }
}
