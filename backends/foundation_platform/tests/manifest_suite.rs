//! F40 — manifest generation, key management, signing, and verification.
//!
//! These tests exercise the chain of trust end to end: a manifest is only
//! useful if a tampered one is rejected, and the only way to know that is to
//! tamper with one and check.
//!
//! `ensure_keys` consults `EWE_OTA_PRIVATE_KEY`, which is process-global, so
//! every test that touches it is tagged `#[parallel(ewe_ota_private_key)]`
//! and the one test that *sets* it is `#[serial(ewe_ota_private_key)]`. They
//! run concurrently with each other but never with the setter — otherwise a
//! test asserting "no private key is available" fails whenever it happens to
//! overlap the one that installs one.

use std::fs;
use std::path::{Path, PathBuf};

use foundation_platform::manifest::{
    self, KeyPair, ManifestError, ManifestSource, MANIFEST_FILENAME, MANIFEST_SCHEMA,
    PRIVATE_KEY_FILE, PUBLIC_KEY_FILE,
};
use tracing_test::traced_test;

// ── Helpers ─────────────────────────────────────────────────────────────

/// A unique scratch directory, removed if a previous run left one behind.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("ewe_f40_manifest_{name}"));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

fn write_file(dir: &Path, relative: &str, contents: &str) {
    let path = dir.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create parent");
    }
    fs::write(path, contents).expect("write file");
}

/// A `public/` tree with two apps and one directory that is not an app.
fn app_tree(root: &Path) -> PathBuf {
    let public = root.join("public");
    write_file(&public, "app/index.html", "<html>app</html>");
    write_file(&public, "app/bundle.js", "console.log('app')");
    write_file(&public, "app/nested/deep.wasm", "\0asm-ish");
    write_file(&public, "app-hello/index.html", "<html>hello</html>");
    // No index.html — build scratch, not an app.
    write_file(&public, "scratch/notes.txt", "ignore me");
    public
}

/// A signable key pair, minted through the same API projects use.
fn signing_keypair(name: &str) -> KeyPair {
    let pair = manifest::ensure_keys(&scratch(&format!("keys_{name}"))).expect("mint keypair");
    assert!(pair.can_sign(), "a freshly minted pair must be signable");
    pair
}

// ── Key generation ──────────────────────────────────────────────────────

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn ensure_keys_creates_public_and_private_key_on_first_call() {
    let root = scratch("ensure_creates");
    let pair = manifest::ensure_keys(&root).expect("ensure_keys");

    assert!(
        root.join("keys").join(PUBLIC_KEY_FILE).exists(),
        "public key file must be written on first call"
    );
    assert!(
        root.join("keys").join(PRIVATE_KEY_FILE).exists(),
        "private key file must be written on first call"
    );
    assert!(pair.can_sign(), "a freshly minted pair must be able to sign");
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn ensure_keys_creates_gitignore_covering_the_private_key() {
    let root = scratch("ensure_gitignore");
    manifest::ensure_keys(&root).expect("ensure_keys");

    let gitignore =
        fs::read_to_string(root.join("keys").join(".gitignore")).expect("read .gitignore");
    assert!(
        gitignore.contains(PRIVATE_KEY_FILE),
        "the private key must be gitignored, got: {gitignore:?}"
    );
    assert!(
        !gitignore.contains(PUBLIC_KEY_FILE),
        "the public key is meant to be committed, so it must not be ignored"
    );
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn ensure_keys_is_idempotent_and_does_not_regenerate() {
    let root = scratch("ensure_idempotent");
    let first = manifest::ensure_keys(&root).expect("first call");
    let second = manifest::ensure_keys(&root).expect("second call");

    assert_eq!(
        first.public, second.public,
        "regenerating would invalidate every binary that already baked the old key"
    );
    assert_eq!(first.private, second.private, "private seed must be stable too");
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn ensure_keys_reads_existing_public_key_without_a_private_key() {
    let root = scratch("ensure_verify_only");
    let original = manifest::ensure_keys(&root).expect("mint");
    fs::remove_file(root.join("keys").join(PRIVATE_KEY_FILE)).expect("drop private key");

    let reread = manifest::ensure_keys(&root).expect("reread");
    assert_eq!(reread.public, original.public, "public key must survive");
    assert!(
        !reread.can_sign(),
        "without a seed on disk or in the env the pair must be verify-only"
    );
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn ensure_keys_rejects_a_public_key_that_is_not_32_bytes() {
    let root = scratch("ensure_bad_key");
    fs::create_dir_all(root.join("keys")).expect("mkdir keys");
    fs::write(root.join("keys").join(PUBLIC_KEY_FILE), "dG9vIHNob3J0")
        .expect("write short key");

    let err = manifest::ensure_keys(&root).expect_err("a short key must be rejected");
    assert!(
        matches!(err, ManifestError::InvalidKey { .. }),
        "expected InvalidKey, got {err:?}"
    );
}

#[test]
#[traced_test]
#[serial_test::serial(ewe_ota_private_key)]
fn ensure_keys_derives_the_public_half_from_the_env_seed() {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine as _;

    let root = scratch("ensure_env_seed");
    let seed = [42u8; 32];
    std::env::set_var(manifest::PRIVATE_KEY_ENV, BASE64.encode(seed));
    let pair = manifest::ensure_keys(&root).expect("ensure_keys with env seed");
    std::env::remove_var(manifest::PRIVATE_KEY_ENV);

    assert_eq!(pair.private, Some(seed), "the env seed must be used verbatim");
    assert!(
        !root.join("keys").join(PRIVATE_KEY_FILE).exists(),
        "a seed handed in through the environment belongs to CI's secret store, \
         and must never be written into the working tree"
    );
}

// ── Manifest generation ─────────────────────────────────────────────────

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn generate_app_manifest_records_every_file_recursively() {
    let root = scratch("gen_recursive");
    let public = app_tree(&root);
    let pair = signing_keypair("gen_recursive");

    let manifest =
        manifest::generate_app_manifest(&public.join("app"), "app", "0.1.0", "cdn.test", &pair)
            .expect("generate");

    let paths: Vec<&str> = manifest.apps[0]
        .files
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert_eq!(
        paths,
        vec!["bundle.js", "index.html", "nested/deep.wasm"],
        "every file must be described, nested ones included, in stable order"
    );
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn generate_app_manifest_computes_sha256_and_size_per_file() {
    let root = scratch("gen_hashes");
    let public = app_tree(&root);
    let pair = signing_keypair("gen_hashes");

    let manifest =
        manifest::generate_app_manifest(&public.join("app"), "app", "0.1.0", "cdn.test", &pair)
            .expect("generate");

    let index = manifest.apps[0]
        .files
        .iter()
        .find(|f| f.path == "index.html")
        .expect("index.html entry");

    let bytes = fs::read(public.join("app/index.html")).expect("read index.html");
    assert_eq!(index.sha256, manifest::sha256_hex(&bytes));
    assert_eq!(index.size, bytes.len() as u64);
    assert_eq!(index.sha256.len(), 64, "sha256 must be 64 hex characters");
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn generate_app_manifest_writes_the_manifest_file() {
    let root = scratch("gen_writes");
    let public = app_tree(&root);
    let pair = signing_keypair("gen_writes");

    manifest::generate_app_manifest(&public.join("app"), "app", "0.1.0", "cdn.test", &pair)
        .expect("generate");

    let written = public.join("app").join(MANIFEST_FILENAME);
    assert!(written.exists(), "{MANIFEST_FILENAME} must be written");

    let parsed: manifest::Manifest =
        serde_json::from_str(&fs::read_to_string(&written).expect("read")).expect("parse");
    assert_eq!(parsed.schema, MANIFEST_SCHEMA);
    assert_eq!(parsed.source, ManifestSource::Apk);
    assert_eq!(parsed.sequence, 0, "APK-bundled manifests use sequence 0");
    assert_eq!(parsed.manifest_domain, "cdn.test");
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn generate_app_manifest_never_describes_itself_or_other_dotfiles() {
    let root = scratch("gen_dotfiles");
    let public = app_tree(&root);
    write_file(&public, "app/.hidden", "editor scratch");
    let pair = signing_keypair("gen_dotfiles");

    // Twice: the first run creates the manifest, the second must not pick it
    // up as one of the files it describes.
    manifest::generate_app_manifest(&public.join("app"), "app", "0.1.0", "cdn.test", &pair)
        .expect("first");
    let manifest =
        manifest::generate_app_manifest(&public.join("app"), "app", "0.1.0", "cdn.test", &pair)
            .expect("second");

    assert!(
        !manifest.apps[0]
            .files
            .iter()
            .any(|f| f.path.starts_with('.')),
        "dotfiles must be skipped, got {:?}",
        manifest.apps[0].files
    );
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn generate_app_manifest_signs_when_a_private_key_is_available() {
    let root = scratch("gen_signed");
    let public = app_tree(&root);
    let pair = manifest::ensure_keys(&root).expect("keys");

    let generated =
        manifest::generate_app_manifest(&public.join("app"), "app", "0.1.0", "cdn.test", &pair)
            .expect("generate");

    assert!(generated.signature.is_some(), "every generated manifest is signed");

    // The signature must verify against the written file, not just the
    // in-memory value — that is what a device will actually check.
    let json = fs::read_to_string(public.join("app").join(MANIFEST_FILENAME)).expect("read");
    let verified = manifest::verify_manifest(&json, &pair.public).expect("verify written manifest");
    assert_eq!(verified.apps[0].files.len(), generated.apps[0].files.len());
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn generate_app_manifest_refuses_to_run_without_a_private_key() {
    let root = scratch("gen_unsigned");
    let public = app_tree(&root);
    let verify_only = KeyPair {
        public: [1u8; 32],
        private: None,
    };

    let err = manifest::generate_app_manifest(
        &public.join("app"),
        "app",
        "0.1.0",
        "cdn.test",
        &verify_only,
    )
    .expect_err("an unsigned manifest is not a supported output");
    assert!(matches!(err, ManifestError::NoPrivateKey), "got {err:?}");

    assert!(
        !public.join("app").join(MANIFEST_FILENAME).exists(),
        "refusing must leave nothing behind — a half-written manifest would be \
         worse than none, because tooling treats presence as a promise"
    );
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn generate_all_manifests_covers_every_app_and_skips_non_apps() {
    let root = scratch("gen_all");
    let public = app_tree(&root);
    let pair = signing_keypair("gen_all");

    let manifests = manifest::generate_all_manifests(&public, "0.1.0", "cdn.test", &pair)
        .expect("generate all");

    let ids: Vec<&str> = manifests
        .iter()
        .map(|m| m.apps[0].app_id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["app", "app-hello"],
        "a directory is an app when it has an index.html; `scratch/` does not"
    );
    assert!(
        !public.join("scratch").join(MANIFEST_FILENAME).exists(),
        "non-app directories must not get a manifest"
    );
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn generate_all_manifests_on_a_missing_directory_is_empty_not_an_error() {
    let root = scratch("gen_missing");
    let pair = signing_keypair("gen_missing");
    let manifests =
        manifest::generate_all_manifests(&root.join("does_not_exist"), "0.1.0", "cdn.test", &pair)
            .expect("must not error");
    assert!(manifests.is_empty());
}

// ── Signing and verification ────────────────────────────────────────────

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn sign_then_verify_round_trips() {
    let pair = signing_keypair("roundtrip");
    let unsigned = sample_manifest_json(None);

    let signature = manifest::sign_manifest(&unsigned, &pair.private.unwrap()).expect("sign");
    let signed = sample_manifest_json(Some(&signature));

    let verified = manifest::verify_manifest(&signed, &pair.public).expect("verify");
    assert_eq!(verified.sequence, 42);
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn verify_manifest_rejects_a_tampered_body() {
    let pair = signing_keypair("tampered_body");
    let unsigned = sample_manifest_json(None);
    let signature = manifest::sign_manifest(&unsigned, &pair.private.unwrap()).expect("sign");

    // Same signature, one field changed — the exact shape of a CDN attacker
    // reusing a valid signature over different content.
    let tampered = sample_manifest_json(Some(&signature)).replace("\"sequence\": 42", "\"sequence\": 43");

    let err = manifest::verify_manifest(&tampered, &pair.public)
        .expect_err("a tampered body must not verify");
    assert!(matches!(err, ManifestError::BadSignature), "got {err:?}");
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn verify_manifest_rejects_a_tampered_file_hash() {
    let pair = signing_keypair("tampered_hash");
    let unsigned = sample_manifest_json(None);
    let signature = manifest::sign_manifest(&unsigned, &pair.private.unwrap()).expect("sign");

    // Swapping the declared hash is how an attacker would point a signed
    // manifest at bytes they control.
    let tampered = sample_manifest_json(Some(&signature)).replace(
        &"a".repeat(64),
        &"b".repeat(64),
    );

    assert!(
        manifest::verify_manifest(&tampered, &pair.public).is_err(),
        "the signature must cover the declared hashes"
    );
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn verify_manifest_rejects_the_wrong_public_key() {
    let pair = signing_keypair("wrong_key");
    let unsigned = sample_manifest_json(None);
    let signature = manifest::sign_manifest(&unsigned, &pair.private.unwrap()).expect("sign");
    let signed = sample_manifest_json(Some(&signature));

    let other = [9u8; 32];
    assert!(
        manifest::verify_manifest(&signed, &other).is_err(),
        "a manifest signed by us must not verify against someone else's key"
    );
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn verify_manifest_rejects_an_unsigned_manifest() {
    let pair = signing_keypair("unsigned");
    let err = manifest::verify_manifest(&sample_manifest_json(None), &pair.public)
        .expect_err("unsigned must be rejected when a key is present");
    assert!(matches!(err, ManifestError::MissingSignature), "got {err:?}");
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn verify_manifest_rejects_an_unsupported_schema() {
    let pair = signing_keypair("bad_schema");
    let bumped = sample_manifest_json(None).replace("\"schema\": 1", "\"schema\": 99");
    let signature = manifest::sign_manifest(&bumped, &pair.private.unwrap()).expect("sign");
    let signed = bumped.replace("\"signature\": null", &format!("\"signature\": \"{signature}\""));

    let err = manifest::verify_manifest(&signed, &pair.public)
        .expect_err("an unknown schema must be rejected");
    assert!(
        matches!(err, ManifestError::UnsupportedSchema { schema: 99 }),
        "got {err:?}"
    );
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn verify_manifest_is_insensitive_to_key_order_and_whitespace() {
    let pair = signing_keypair("canonical");
    let signature =
        manifest::sign_manifest(&sample_manifest_json(None), &pair.private.unwrap()).expect("sign");

    // Same document, keys emitted in a different order and re-indented.
    // Canonicalization is what makes this verify; without it a proxy that
    // reserializes JSON would break every update.
    let reordered = format!(
        r#"{{"apps":[{{"app_id":"app","bundle_version":"0.1.2","files":[{{"size":1024,"path":"index.html","sha256":"{hash}"}}]}}],
            "signature":"{signature}","sequence":42,"schema":1,"source":"ota",
            "manifest_domain":"cdn.test","created_at":"2026-07-23T10:00:00Z",
            "base_url":"https://cdn.test/bundles","rollback_to":null,"delete_after":null}}"#,
        hash = "a".repeat(64),
    );

    manifest::verify_manifest(&reordered, &pair.public)
        .expect("canonical form must ignore key order and whitespace");
}

// ── File hash verification ──────────────────────────────────────────────

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn verify_file_hash_accepts_matching_bytes_and_rejects_everything_else() {
    let bytes = b"console.log('bundle')";
    let json = format!(
        r#"{{"schema":1,"source":"ota","manifest_domain":"cdn.test",
            "created_at":"2026-07-23T10:00:00Z","sequence":1,
            "base_url":"https://cdn.test/bundles","rollback_to":null,
            "delete_after":null,"signature":null,
            "apps":[{{"app_id":"app","bundle_version":"0.1.0","files":[
                {{"path":"bundle.js","sha256":"{hash}","size":{size}}}]}}]}}"#,
        hash = manifest::sha256_hex(bytes),
        size = bytes.len(),
    );
    let m: manifest::Manifest = serde_json::from_str(&json).expect("parse");

    assert!(manifest::verify_file_hash(&m, "app", "bundle.js", bytes));
    assert!(
        !manifest::verify_file_hash(&m, "app", "bundle.js", b"tampered"),
        "different bytes must not pass"
    );
    assert!(
        !manifest::verify_file_hash(&m, "app", "unknown.js", bytes),
        "an undeclared file is never trusted, whatever its bytes hash to"
    );
    assert!(
        !manifest::verify_file_hash(&m, "other-app", "bundle.js", bytes),
        "an unknown app is never trusted"
    );
}

#[test]
#[traced_test]
#[serial_test::parallel(ewe_ota_private_key)]
fn sha256_hex_matches_the_known_empty_digest() {
    assert_eq!(
        manifest::sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "a wrong digest here would silently accept every file"
    );
}

// ── Fixtures ────────────────────────────────────────────────────────────

/// A manifest document with `signature` either null or filled in. Written as
/// text rather than serialized so the signing tests can tamper with exact
/// bytes.
fn sample_manifest_json(signature: Option<&str>) -> String {
    let sig = signature.map_or_else(|| "null".to_string(), |s| format!("\"{s}\""));
    format!(
        r#"{{
  "schema": 1,
  "source": "ota",
  "manifest_domain": "cdn.test",
  "created_at": "2026-07-23T10:00:00Z",
  "sequence": 42,
  "base_url": "https://cdn.test/bundles",
  "rollback_to": null,
  "delete_after": null,
  "signature": {sig},
  "apps": [
    {{
      "app_id": "app",
      "bundle_version": "0.1.2",
      "files": [
        {{
          "path": "index.html",
          "sha256": "{hash}",
          "size": 1024
        }}
      ]
    }}
  ]
}}"#,
        hash = "a".repeat(64),
    )
}
