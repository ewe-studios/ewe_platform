//! Exercise the compact entropy and cipher APIs inside the selected Wasm host.

use foundation_compact::{
    crypto::chacha::{decrypt, encrypt_with_nonce, ChaChaKey},
    entropy,
};

/// Return 0 after a successful round trip, 1 for missing entropy, or 2 for a cipher failure.
#[cfg_attr(
    all(
        target_arch = "wasm32",
        feature = "wasm-bindgen",
        not(feature = "host-abi")
    ),
    wasm_bindgen::prelude::wasm_bindgen
)]
#[cfg_attr(feature = "host-abi", unsafe(no_mangle))]
#[must_use]
pub extern "C" fn check_host_crypto() -> u32 {
    let mut nonce = [0; 12];
    if entropy::fill(&mut nonce).is_err() {
        return 1;
    }
    let key = ChaChaKey::from_bytes([7; 32]);
    let body = b"host entropy probe";
    let Ok(encrypted) = encrypt_with_nonce(&key, nonce, body) else {
        return 2;
    };
    match decrypt(&key, &encrypted) {
        Ok(recovered) if recovered.as_slice() == body => 0,
        _ => 2,
    }
}
