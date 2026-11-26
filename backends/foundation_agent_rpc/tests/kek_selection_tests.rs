//! The native key encryption key (KEK) selection and the `agent-rpc keys`
//! commands.

use std::io::IsTerminal as _;
use std::sync::Arc;

use foundation_agent_rpc::keys::{self, KekConfig, KeysCommand};
use foundation_auth::keystore::{FsKeyStore, KeyId, KeyMaterial, KeyProtection, KeyStore};
use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use tempfile::TempDir;

const LEGACY_SIGNING_KEY: &str =
    include_str!("../../foundation_auth/tests/fixtures/test-only-ed25519-key.pem");

/// A host context with the effect sources installed.
fn test_ctx() -> Ctx {
    let ctx = Ctx::host();
    foundation_core::env::install(&ctx, foundation_core::env::system).expect("install the effects");
    ctx
}

/// A secret file that holds 32 bytes.
fn write_kek(dir: &TempDir, name: &str, byte: u8) -> KekConfig {
    let path = dir.path().join(name);
    std::fs::write(&path, [byte; 32]).expect("write the KEK");
    KekConfig::SecretFile { path }
}

#[valtron_test]
async fn kek_selection_uses_the_configured_secret_file() {
    let ctx = test_ctx();
    let env = ctx.require_env().expect("env");
    let dir = TempDir::new().expect("temp dir");
    let config = write_kek(&dir, "kek", 3);

    let sealer = keys::select_native_kek(Some(&config), &env, ctx.clock())
        .await
        .expect("load");
    assert!(sealer.kek_id().starts_with("secret-file:"));
}

#[valtron_test]
async fn kek_selection_refuses_an_environment_variable_without_permission() {
    let ctx = test_ctx();
    let env = ctx.require_env().expect("env");
    let config = KekConfig::EnvironmentVariable {
        name: "EWE_TEST_KEK_SELECTION".to_string(),
        allow_environment_variable: false,
    };
    let Err(error) = keys::select_native_kek(Some(&config), &env, ctx.clock()).await else {
        panic!("an environment variable without permission must be refused");
    };
    assert!(format!("{error}").contains("allow_environment_variable"));
}

#[valtron_test]
async fn kek_selection_with_no_source_lists_the_options() {
    if std::env::var_os("CREDENTIALS_DIRECTORY").is_some()
        || std::path::Path::new(keys::DEFAULT_SECRET_FILE).exists()
        || std::io::stdin().is_terminal()
    {
        // The host has a default source that this test cannot control.
        return;
    }
    let ctx = test_ctx();
    let env = ctx.require_env().expect("env");
    let Err(error) = keys::select_native_kek(None, &env, ctx.clock()).await else {
        panic!("a host with no KEK source must not start");
    };
    let text = format!("{error}");
    assert!(text.contains("systemd"), "names the systemd option");
    assert!(text.contains("secret file"), "names the secret file option");
    assert!(text.contains("passphrase"), "names the passphrase option");
}

#[test]
fn kek_command_parsing_accepts_the_subcommands() {
    let args = |words: &[&str]| {
        words
            .iter()
            .map(|word| (*word).to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        keys::parse_keys_command(&args(&["keys", "migrate"])),
        Ok(KeysCommand::Migrate)
    );
    assert_eq!(
        keys::parse_keys_command(&args(&["migrate"])),
        Ok(KeysCommand::Migrate)
    );
    assert_eq!(
        keys::parse_keys_command(&args(&["keys", "rotate"])),
        Ok(KeysCommand::RotateKey)
    );
    assert_eq!(
        keys::parse_keys_command(&args(&["rotate-kek"])),
        Ok(KeysCommand::RotateKek)
    );
    assert!(keys::parse_keys_command(&args(&["keys", "wrong"])).is_err());
}

#[valtron_test]
async fn kek_migrate_seals_the_legacy_key_and_removes_the_file() {
    let ctx = test_ctx();
    let dir = TempDir::new().expect("temp dir");
    let state = dir.path();
    let legacy = state.join("bootstrap-signing-key.pem");
    std::fs::write(&legacy, LEGACY_SIGNING_KEY).expect("write the legacy key");
    let config = write_kek(&dir, "kek", 7);

    let report = keys::migrate(state, Some(&config), &ctx)
        .await
        .expect("migrate");
    assert_eq!(
        report.migrated,
        vec![KeyId::new(KeyId::SECRET_SIGNING).expect("valid id")]
    );
    assert!(!legacy.exists(), "the clear-text file is removed");

    // The sealed record opens with the same KEK.
    let env = ctx.require_env().expect("env");
    let sealer = keys::select_native_kek(Some(&config), &env, ctx.clock())
        .await
        .expect("load");
    let store = Arc::new(FsKeyStore::new(state));
    let material = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::new(sealer),
        ctx,
    );
    let id = KeyId::new(KeyId::SECRET_SIGNING).expect("valid id");
    material
        .signing_key(&id)
        .await
        .expect("the sealed key opens");
}

#[valtron_test]
async fn kek_rotate_moves_the_record_to_the_new_key() {
    let ctx = test_ctx();
    let dir = TempDir::new().expect("temp dir");
    let state = dir.path();
    std::fs::write(state.join("bootstrap-signing-key.pem"), LEGACY_SIGNING_KEY)
        .expect("write the legacy key");
    let old = write_kek(&dir, "old-kek", 1);
    let new = write_kek(&dir, "new-kek", 2);

    keys::migrate(state, Some(&old), &ctx)
        .await
        .expect("migrate under the old KEK");
    let report = keys::rotate_kek(state, Some(&new), &[old], &ctx)
        .await
        .expect("rotate");
    assert_eq!(report.rotated, 1);

    // The record now opens with the new KEK alone.
    let env = ctx.require_env().expect("env");
    let sealer = keys::select_native_kek(Some(&new), &env, ctx.clock())
        .await
        .expect("load");
    let store = Arc::new(FsKeyStore::new(state));
    let material = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::new(sealer),
        ctx,
    );
    let id = KeyId::new(KeyId::SECRET_SIGNING).expect("valid id");
    material
        .signing_key(&id)
        .await
        .expect("the rotated key opens");
}

#[valtron_test]
async fn kek_rotate_by_kid_keeps_the_old_verifying_key() {
    let ctx = test_ctx();
    let dir = TempDir::new().expect("temp dir");
    let state = dir.path();
    std::fs::write(state.join("bootstrap-signing-key.pem"), LEGACY_SIGNING_KEY)
        .expect("write the legacy key");
    let config = write_kek(&dir, "kek", 5);

    // Migrate the legacy key, then read the current key.
    keys::migrate(state, Some(&config), &ctx)
        .await
        .expect("migrate");
    let env = ctx.require_env().expect("env");
    let sealer = keys::select_native_kek(Some(&config), &env, ctx.clock())
        .await
        .expect("load");
    let store = Arc::new(FsKeyStore::new(state));
    let material = KeyMaterial::new(
        Arc::clone(&store) as Arc<dyn KeyStore>,
        KeyProtection::new(sealer),
        ctx.clone(),
    );
    let id = KeyId::new(KeyId::SECRET_SIGNING).expect("valid id");
    let old_kid = material
        .signing_key(&id)
        .await
        .expect("old key")
        .verifying_key()
        .kid()
        .map(str::to_string)
        .expect("the old kid");

    // Rotate the signing key by `kid`.
    let report = keys::rotate_signing_key(state, Some(&config), &ctx)
        .await
        .expect("rotate the signing key");
    assert_eq!(report.rotated, 1);
    let new_kid = report.new_kid.expect("a new kid");
    assert_ne!(new_kid, old_kid);

    // The current key is the new one, and the verifier holds both public keys.
    let current_kid = material
        .signing_key(&id)
        .await
        .expect("the new current key")
        .verifying_key()
        .kid()
        .map(str::to_string)
        .expect("the current kid");
    assert_eq!(&current_kid, &new_kid);
    let keys_of_kind = material
        .verifying_keys_of_kind(foundation_auth::keystore::KeyKind::SecretSigning)
        .await
        .expect("verifying keys");
    let kids: Vec<String> = keys_of_kind
        .iter()
        .filter_map(|key| key.kid().map(str::to_string))
        .collect();
    assert!(kids.contains(&old_kid), "the old public key is still there");
    assert!(kids.contains(&current_kid), "the new public key is there");
}
