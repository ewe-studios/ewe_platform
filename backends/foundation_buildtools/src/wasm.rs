//! Wasm target classification — the four flavors relevant to the spec's target matrix.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmFlavor {
    /// `wasm32-unknown-unknown` — CF Workers, browser SPA. No libc, no std time.
    UnknownUnknown,
    /// `wasm32-unknown-emscripten` — browser local inference, WebGPU, pthreads.
    Emscripten,
    /// `wasm32-wasip1` — WASI preview 1, std works, wasi-threads.
    Wasip1,
    /// `wasm32-wasip2` — WASI 0.2 component model.
    Wasip2,
}

impl WasmFlavor {
    #[must_use]
    pub fn from_triple(triple: &str) -> Self {
        if triple.contains("emscripten") {
            Self::Emscripten
        } else if triple.contains("wasip2") {
            Self::Wasip2
        } else if triple.contains("wasip1") || triple.contains("wasi") && !triple.contains("wasip") {
            Self::Wasip1
        } else {
            Self::UnknownUnknown
        }
    }

    #[must_use]
    pub fn triple(self) -> &'static str {
        match self {
            Self::UnknownUnknown => "wasm32-unknown-unknown",
            Self::Emscripten => "wasm32-unknown-emscripten",
            Self::Wasip1 => "wasm32-wasip1",
            Self::Wasip2 => "wasm32-wasip2",
        }
    }

    #[must_use]
    pub fn has_libc(self) -> bool {
        !matches!(self, Self::UnknownUnknown)
    }

    #[must_use]
    pub fn has_std_time(self) -> bool {
        !matches!(self, Self::UnknownUnknown)
    }

    #[must_use]
    pub fn has_threads(self) -> bool {
        matches!(self, Self::Emscripten | Self::Wasip2)
    }

    #[must_use]
    pub fn is_js_hosted(self) -> bool {
        matches!(self, Self::UnknownUnknown | Self::Emscripten)
    }

    #[must_use]
    pub fn supports_native_c_toolchain(self) -> bool {
        matches!(self, Self::Emscripten)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flavor_from_triple() {
        assert_eq!(
            WasmFlavor::from_triple("wasm32-unknown-unknown"),
            WasmFlavor::UnknownUnknown
        );
        assert_eq!(
            WasmFlavor::from_triple("wasm32-unknown-emscripten"),
            WasmFlavor::Emscripten
        );
        assert_eq!(
            WasmFlavor::from_triple("wasm32-wasip1"),
            WasmFlavor::Wasip1
        );
        assert_eq!(
            WasmFlavor::from_triple("wasm32-wasip2"),
            WasmFlavor::Wasip2
        );
    }

    #[test]
    fn capability_queries() {
        assert!(!WasmFlavor::UnknownUnknown.has_libc());
        assert!(WasmFlavor::Emscripten.has_libc());
        assert!(WasmFlavor::Wasip1.has_libc());

        assert!(!WasmFlavor::UnknownUnknown.has_threads());
        assert!(WasmFlavor::Emscripten.has_threads());
        assert!(!WasmFlavor::Wasip1.has_threads());
        assert!(WasmFlavor::Wasip2.has_threads());

        assert!(WasmFlavor::UnknownUnknown.is_js_hosted());
        assert!(WasmFlavor::Emscripten.is_js_hosted());
        assert!(!WasmFlavor::Wasip1.is_js_hosted());

        assert!(!WasmFlavor::UnknownUnknown.supports_native_c_toolchain());
        assert!(WasmFlavor::Emscripten.supports_native_c_toolchain());
        assert!(!WasmFlavor::Wasip1.supports_native_c_toolchain());
    }

    #[test]
    fn round_trip_triple() {
        for flavor in [
            WasmFlavor::UnknownUnknown,
            WasmFlavor::Emscripten,
            WasmFlavor::Wasip1,
            WasmFlavor::Wasip2,
        ] {
            assert_eq!(WasmFlavor::from_triple(flavor.triple()), flavor);
        }
    }
}
