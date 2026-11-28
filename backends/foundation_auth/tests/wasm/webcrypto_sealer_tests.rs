//! The `WebCryptoSealer` in a browser page: the sealer contract and a refused
//! key export (spec 92 feature 00, task T6).
//!
//! WHY: `WebCryptoSealer` imports the raw key encryption key (KEK) bytes into a
//! non-extractable WebCrypto key. Only a browser proves that the key stays
//! non-extractable.
//!
//! WHAT: seal a payload, open it back, refuse a different record, and prove
//! that `crypto.subtle.exportKey` rejects the sealer's key.
//!
//! HOW: `wasm-bindgen-test` runs this case in a headless Chromium page. The
//! entropy source is seeded, so the test reads no real secret.

#![cfg(all(target_family = "wasm", feature = "wasm-kek"))]

use std::sync::Arc;

use foundation_auth::keystore::webcrypto::WebCryptoSealer;
use foundation_auth::keystore::KeySealer;
use foundation_compact::crypto::SecureBytes;
use foundation_context::env::Env;
use foundation_core::env::{CounterSource, SeededEntropy, ValtronTimer};
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test;

/// The effect sources of the test. The entropy source is seeded.
fn test_env() -> Env {
    Env::new(
        Arc::new(ValtronTimer),
        Arc::new(SeededEntropy::new(92)),
        Arc::new(CounterSource::new(0)),
    )
}

#[wasm_bindgen_test]
async fn seals_opens_and_refuses_export() {
    let sealer = WebCryptoSealer::import(
        "browser-sealer".to_string(),
        SecureBytes::new(vec![7u8; 32]),
        test_env(),
    )
    .await
    .expect("import the KEK");
    assert_eq!(sealer.kek_id(), "browser-sealer");

    let aad = b"record:ca:1";
    let plaintext = b"a private key in PKCS#8 DER";
    let sealed = sealer.seal(aad, plaintext).await.expect("seal");
    assert_eq!(sealed.alg, "aes256gcm");
    assert_eq!(sealed.kek_id, "browser-sealer");

    let opened = sealer.open(aad, &sealed).await.expect("open");
    assert_eq!(opened.as_slice(), plaintext);

    // A different record cannot open the blob.
    assert!(sealer.open(b"record:ca:2", &sealed).await.is_err());

    // The imported key is non-extractable, so the browser refuses the export.
    let subtle = web_sys::window()
        .expect("a browser window")
        .crypto()
        .expect("WebCrypto")
        .subtle();
    let export = subtle
        .export_key("raw", sealer.crypto_key())
        .expect("exportKey returns a promise");
    assert!(
        JsFuture::from(export).await.is_err(),
        "the browser exported the non-extractable KEK"
    );
}
