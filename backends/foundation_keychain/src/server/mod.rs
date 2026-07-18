//! Server entry points — platform-gated (spec-57, F009/F010).

#[cfg(not(target_family = "wasm"))]
pub mod native;
#[cfg(target_family = "wasm")]
pub mod cloudflare;

