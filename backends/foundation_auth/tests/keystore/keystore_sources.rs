//! The native key encryption key (KEK) sources.

use base64::Engine as _;
use foundation_auth::keystore::{
    EnvironmentVariableKek, FixedKek, KekSource, SecretFileKek, SystemdCredentialKek,
};
use foundation_core::valtron::valtron_test;
use tempfile::TempDir;

use super::common::test_ctx;

#[valtron_test]
async fn a_secret_file_source_loads_a_32_byte_key() {
    let (_ctx, env) = test_ctx();
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("agent-rpc-kek");
    std::fs::write(&path, [9u8; 32]).expect("write the KEK");

    let source = SecretFileKek::new(&path);
    assert_eq!(source.describe().kind, "secret-file");
    let sealer = source.load(&env).await.expect("load");
    assert!(sealer.kek_id().starts_with("secret-file:"));
}

#[valtron_test]
async fn a_secret_file_source_refuses_a_value_that_is_not_32_bytes() {
    let (_ctx, env) = test_ctx();
    let dir = TempDir::new().expect("temp dir");
    let path = dir.path().join("bad-kek");
    std::fs::write(&path, b"too short").expect("write the bad KEK");

    assert!(SecretFileKek::new(&path).load(&env).await.is_err());
}

#[valtron_test]
async fn an_environment_variable_needs_explicit_permission() {
    let (_ctx, env) = test_ctx();
    let source = EnvironmentVariableKek::new("EWE_TEST_KEK_NOT_ALLOWED", false);
    let Err(error) = source.load(&env).await else {
        panic!("an environment variable without permission must be refused");
    };
    assert!(
        format!("{error}").contains("allow_environment_variable"),
        "the error names the permission"
    );
}

#[valtron_test]
async fn an_allowed_environment_variable_is_read_and_removed() {
    let (_ctx, env) = test_ctx();
    let name = "EWE_TEST_KEK_ALLOWED";
    let value = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode([5u8; 32]);
    std::env::set_var(name, value);

    let source = EnvironmentVariableKek::new(name, true);
    let sealer = source.load(&env).await.expect("load");
    assert!(sealer.kek_id().starts_with("environment-variable:"));
    assert!(
        std::env::var(name).is_err(),
        "the variable is removed from the process environment"
    );
}

#[valtron_test]
async fn a_systemd_credential_needs_the_directory() {
    if std::env::var_os("CREDENTIALS_DIRECTORY").is_some() {
        // A host that runs under systemd sets the variable; this test cannot
        // control the value.
        return;
    }
    let (_ctx, env) = test_ctx();
    let Err(error) = SystemdCredentialKek::new("agent-rpc-kek").load(&env).await else {
        panic!("a systemd credential needs CREDENTIALS_DIRECTORY");
    };
    assert!(format!("{error}").contains("CREDENTIALS_DIRECTORY"));
}

#[valtron_test]
async fn a_fixed_kek_source_makes_the_same_sealer() {
    let (_ctx, env) = test_ctx();
    let source = FixedKek::new("fixed-kek", [4; 32]);
    let sealer = source.load(&env).await.expect("load");
    assert_eq!(sealer.kek_id(), "fixed-kek");
    assert_eq!(source.describe().kek_id, "fixed-kek");
}
