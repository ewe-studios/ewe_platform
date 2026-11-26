//! Helpers for the key-store tests.

use foundation_auth::keystore::{
    Argon2Params, KekSealer, KeyId, KeyKind, KeyRecord, PublicMaterial, SealedBlob, SecretText,
    StoredPrivateKey,
};
use foundation_context::Ctx;
use foundation_core::env::Env;

/// A host context with the real effect sources installed.
pub fn test_ctx() -> (Ctx, Env) {
    let ctx = Ctx::host();
    let env = foundation_core::env::install(&ctx, foundation_core::env::system)
        .expect("install the effect sources");
    (ctx, env)
}

/// A deterministic key encryption key (KEK) sealer.
pub fn kek_sealer(env: &Env) -> KekSealer {
    KekSealer::new("test-kek", [7; 32], env.clone())
}

/// An Argon2id passphrase sealer with the smallest useful cost, for a fast
/// test. It is not the production cost.
pub fn passphrase_sealer(env: &Env) -> foundation_auth::keystore::PassphraseSealer {
    foundation_auth::keystore::PassphraseSealer::new(
        "test-passphrase",
        zeroize::Zeroizing::new("correct horse battery staple".to_string()),
        Argon2Params {
            memory_kib: 8 * 1024,
            iterations: 1,
            lanes: 1,
        },
        env.clone(),
    )
}

/// A sealed blob whose bytes a store does not need to open.
pub fn dummy_blob() -> SealedBlob {
    SealedBlob {
        v: 1,
        alg: "chacha20poly1305".to_string(),
        kek_id: "test-kek".to_string(),
        nonce_b64url: "AAAAAAAAAAAAAAAA".to_string(),
        ciphertext_b64url: "AAAA".to_string(),
        salt_b64url: None,
        argon2: None,
        wrapped_dek_b64url: None,
    }
}

/// A record with a sealed private part for `id` and `kind`.
pub fn sealed_record(id: &str, kind: KeyKind, sealed: SealedBlob) -> KeyRecord {
    KeyRecord {
        id: KeyId::new(id).expect("valid id"),
        kind,
        revision: 1,
        created_at_unix: 1_700_000_000,
        not_after_unix: None,
        public: PublicMaterial {
            algorithm: "ed25519".to_string(),
            public_key_b64url: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            cert_pem: None,
            subject_names: Vec::new(),
        },
        private: StoredPrivateKey::Sealed(sealed),
    }
}

/// A record with a clear-text private part. No writable store accepts it.
pub fn plaintext_record(id: &str, kind: KeyKind) -> KeyRecord {
    KeyRecord {
        id: KeyId::new(id).expect("valid id"),
        kind,
        revision: 1,
        created_at_unix: 1_700_000_000,
        not_after_unix: None,
        public: PublicMaterial {
            algorithm: "ed25519".to_string(),
            public_key_b64url: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
            cert_pem: None,
            subject_names: Vec::new(),
        },
        private: StoredPrivateKey::Plaintext {
            pkcs8_pem: SecretText::new(
                "-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----\n",
            ),
        },
    }
}
