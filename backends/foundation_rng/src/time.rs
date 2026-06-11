//! TimeSource trait (re-exported from generator) and platform-specific implementations.

// Re-export the TimeSource trait that Generator uses
pub use crate::generator::TimeSource;

// ─── Native ──────────────────────────────────────────────────────────────────

/// TimeSource using [`std::time::SystemTime`].
#[derive(Clone, Debug, Default)]
pub struct NativeTime;

impl TimeSource for NativeTime {
    fn unix_ts_ms(&mut self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock may have gone backwards")
            .as_millis() as u64
    }
}

// ─── WASM Bindgen ────────────────────────────────────────────────────────────

/// TimeSource using `js_sys::Date::now()` for wasm32-unknown-unknown.
#[cfg(feature = "js-wasmbindgen")]
#[derive(Clone, Debug, Default)]
pub struct WasmBindgenTime;

#[cfg(feature = "js-wasmbindgen")]
impl TimeSource for WasmBindgenTime {
    fn unix_ts_ms(&mut self) -> u64 {
        js_sys::Date::now() as u64
    }
}

// ─── WASM Runtime ────────────────────────────────────────────────────────────

/// TimeSource using `foundation_wasm` for wasm32-unknown-unknown.
#[cfg(feature = "wasm-runtime")]
#[derive(Clone, Debug, Default)]
pub struct WasmRuntimeTime;

#[cfg(feature = "wasm-runtime")]
impl TimeSource for WasmRuntimeTime {
    fn unix_ts_ms(&mut self) -> u64 {
        foundation_wasm::time::now_ms()
    }
}

// ─── Platform selection ─────────────────────────────────────────────────────

#[cfg(feature = "native")]
pub fn platform_time_source() -> NativeTime {
    NativeTime
}

#[cfg(all(feature = "js-wasmbindgen", not(feature = "native")))]
pub fn platform_time_source() -> WasmBindgenTime {
    WasmBindgenTime
}

#[cfg(all(feature = "wasm-runtime", not(feature = "native"), not(feature = "js-wasmbindgen")))]
pub fn platform_time_source() -> WasmRuntimeTime {
    WasmRuntimeTime
}
