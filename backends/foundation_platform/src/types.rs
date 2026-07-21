//! Platform types — re-exports from `foundation_wasm`.
//!
//! F05 previously defined `CapabilityRequest`/`CapabilityResponse` here with
//! `serde_json::Value` payloads. F23 moved portable bytes-based types to
//! `foundation_wasm::capability`. Platform capability security lives in
//! `capability.rs` (wrapping `WasmCapability` + `PlatformCapability`).
