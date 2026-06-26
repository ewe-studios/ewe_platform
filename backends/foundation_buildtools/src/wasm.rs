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

