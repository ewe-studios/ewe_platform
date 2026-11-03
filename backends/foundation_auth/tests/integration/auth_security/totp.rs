//! TOTP: enrollment, encryption at rest, key rotation and replay.

use std::sync::Arc;

use foundation_auth::server::services::MfaService;
use foundation_auth::server::TotpKeyRing;
use foundation_auth::TOTPSecret;
use foundation_core::valtron::{valtron_test, BlockingFutureExt};
use foundation_db::KeyValueStore;

use crate::integration::tos_login_gate::{config, start, start_raw, test_keys, Env, Reply};

const ENROLL_START: &str = "/auth/v1/mfa/totp/enroll/start";
const ENROLL_CONFIRM: &str = "/auth/v1/mfa/totp/enroll/confirm";

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Register and log in a user (no `ToS`, no TOTP yet); return the id and the
/// session cookie.
fn signed_in(env: &Env, email: &str) -> (String, String) {
    let id = env.register(email);
    let reply = env.login(email);
    assert_eq!(reply.status, 202, "login: {}", reply.body);
    (id, reply.session_cookie().expect("session cookie"))
}

/// Enroll with the endpoints; return the secret. The confirm code is the
/// current step's code.
fn enroll(env: &Env, cookie: &str) -> TOTPSecret {
    let started = env.post(ENROLL_START, &serde_json::json!({}), Some(cookie));
    assert_eq!(started.status, 200, "start: {}", started.body);
    assert!(started.body["otpauth_uri"]
        .as_str()
        .unwrap()
        .starts_with("otpauth://totp/"));
    let secret = TOTPSecret::from_base32(started.body["secret"].as_str().unwrap()).unwrap();
    let confirmed = env.post(
        ENROLL_CONFIRM,
        &serde_json::json!({ "code": secret.now() }),
        Some(cookie),
    );
    assert_eq!(confirmed.status, 200, "confirm: {}", confirmed.body);
    secret
}

fn mfa_login(env: &Env, email: &str, code: &str) -> Reply {
    let challenge = env.login(email);
    assert_eq!(challenge.status, 200, "login: {}", challenge.body);
    assert_eq!(challenge.body["status"], "mfa_required");
    env.post(
        "/auth/v1/mfa",
        &serde_json::json!({
            "challenge_id": challenge.body["challenge_id"],
            "code": code,
        }),
        None,
    )
}

#[valtron_test(threads = 8, timeout = 60000)]
fn enroll_then_login_with_totp() {
    let env = start(config());
    let (_, cookie) = signed_in(&env, "enroll@example.com");
    let secret = enroll(&env, &cookie);

    // The first code is spent by the confirm step; the next step's code works
    // (the server accepts one step ahead).
    let reply = mfa_login(&env, "enroll@example.com", &secret.code_at(now_secs() + 30));

    assert_eq!(reply.status, 202, "body: {}", reply.body);
    assert!(reply.session_cookie().is_some());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn totp_secret_is_not_stored_in_plain_text() {
    let env = start(config());
    let (user, cookie) = signed_in(&env, "plain@example.com");
    let secret = enroll(&env, &cookie);

    let storage = Arc::clone(&env.storage);
    let key = format!("mfa:totp:{user}");
    let stored = async move { storage.cache.get::<serde_json::Value>(&key).await }
        .block()
        .unwrap()
        .expect("a stored secret");

    let text = stored.to_string();
    assert!(
        !text.contains(&secret.to_base32()),
        "plain text secret: {text}"
    );
    assert_eq!(stored["key_id"], "k1", "the key id is stored with the data");
    assert!(stored["nonce"].is_string() && stored["ciphertext"].is_string());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn totp_code_cannot_be_used_twice() {
    let env = start(config());
    let user = env.register("twice@example.com");
    let secret = env.enroll(&user);
    let code = secret.now();

    let first = mfa_login(&env, "twice@example.com", &code);
    let second = mfa_login(&env, "twice@example.com", &code);

    assert_eq!(first.status, 202, "first: {}", first.body);
    assert_eq!(second.status, 401, "the same code again: {}", second.body);
    assert!(second.session_cookie().is_none());
}

#[valtron_test(threads = 8, timeout = 60000)]
fn enrollment_requires_session() {
    let env = start(config());
    env.register("nosession@example.com");

    let start_reply = env.post(ENROLL_START, &serde_json::json!({}), None);
    let confirm_reply = env.post(
        ENROLL_CONFIRM,
        &serde_json::json!({ "code": "123456" }),
        None,
    );
    let forged = env.post(
        ENROLL_START,
        &serde_json::json!({}),
        Some("session_token=forged"),
    );

    assert_eq!(start_reply.status, 401, "{}", start_reply.body);
    assert_eq!(confirm_reply.status, 401, "{}", confirm_reply.body);
    assert_eq!(forged.status, 401, "{}", forged.body);
}

#[valtron_test(threads = 8, timeout = 60000)]
fn enrolled_secret_needs_the_current_code_to_be_replaced() {
    let env = start(config());
    let user = env.register("replace@example.com");
    let secret = env.enroll(&user);
    // Sign in with TOTP to get a session.
    let login = mfa_login(&env, "replace@example.com", &secret.now());
    let cookie = login.session_cookie().expect("session");

    let without = env.post(ENROLL_START, &serde_json::json!({}), Some(&cookie));
    let wrong = env.post(
        ENROLL_START,
        &serde_json::json!({ "current_code": "000000" }),
        Some(&cookie),
    );
    assert_eq!(without.status, 401, "{}", without.body);
    assert_eq!(wrong.status, 401, "{}", wrong.body);

    // With a valid, unspent current code (one step ahead; the login above
    // spent the current step) the replacement starts.
    let ok = env.post(
        ENROLL_START,
        &serde_json::json!({ "current_code": secret.code_at(now_secs() + 30) }),
        Some(&cookie),
    );
    assert_eq!(ok.status, 200, "{}", ok.body);
}

#[valtron_test(threads = 8, timeout = 60000)]
fn enrollment_is_off_without_an_encryption_key() {
    let env = start_raw(config());
    let (_, cookie) = signed_in(&env, "nokey@example.com");

    let reply = env.post(ENROLL_START, &serde_json::json!({}), Some(&cookie));

    assert_eq!(reply.status, 500, "{}", reply.body);
}

#[valtron_test(threads = 8, timeout = 60000)]
fn totp_secret_survives_key_rotation() {
    let env = start(config());
    let user = env.register("rotate@example.com");
    let secret = env.enroll(&user); // encrypted with "k1"

    // A new active key "k2"; "k1" stays as a retired key.
    let rotated = TotpKeyRing::new("k2", [9u8; 32]).with_retired("k1", [7u8; 32]);
    let service = MfaService::new(Arc::clone(&env.storage), rotated);
    let (user_c, code) = (user.clone(), secret.now());
    let ok = async move {
        let found = service.totp_secret(&user_c).await.unwrap().expect("secret");
        assert_eq!(
            found.to_base32(),
            TOTPSecret::from_base32(&found.to_base32())
                .unwrap()
                .to_base32()
        );
        service.verify_totp_code(&user_c, &code).await.unwrap()
    }
    .block();
    assert!(ok, "old ciphertext decrypts with the retired key");

    // Without the old key the secret cannot be read (an error, not a guess).
    let no_old = MfaService::new(Arc::clone(&env.storage), TotpKeyRing::new("k2", [9u8; 32]));
    let user_c = user.clone();
    let failed = async move { no_old.totp_secret(&user_c).await.is_err() }.block();
    assert!(failed);

    // The key id in test_keys() is the active one for new enrollments.
    assert_eq!(test_keys().active().unwrap().0, "k1");
}

fn provisioning_uri(env: &Env, email: &str) -> String {
    let (_, cookie) = signed_in(env, email);
    let started = env.post(ENROLL_START, &serde_json::json!({}), Some(&cookie));
    assert_eq!(started.status, 200, "start: {}", started.body);
    started.body["otpauth_uri"].as_str().unwrap().to_string()
}

#[valtron_test(threads = 8, timeout = 60000)]
fn provisioning_uri_names_the_algorithm() {
    let default_env = start(config());
    let uri = provisioning_uri(&default_env, "sha1@example.com");
    assert!(uri.contains("algorithm=SHA1"), "{uri}");
    assert!(
        uri.contains("digits=6") && uri.contains("period=30"),
        "{uri}"
    );

    let sha256_env = start(config().with_totp_algorithm(foundation_auth::TotpAlgorithm::Sha256));
    let uri = provisioning_uri(&sha256_env, "sha256@example.com");
    assert!(uri.contains("algorithm=SHA256"), "{uri}");
}

#[valtron_test(threads = 8, timeout = 60000)]
fn sha256_enrollment_logs_in_with_sha256_codes() {
    let env = start(config().with_totp_algorithm(foundation_auth::TotpAlgorithm::Sha256));
    let (_, cookie) = signed_in(&env, "s256login@example.com");
    let started = env.post(ENROLL_START, &serde_json::json!({}), Some(&cookie));
    let secret = TOTPSecret::from_base32(started.body["secret"].as_str().unwrap())
        .unwrap()
        .with_algorithm(foundation_auth::TotpAlgorithm::Sha256);
    let confirmed = env.post(
        ENROLL_CONFIRM,
        &serde_json::json!({ "code": secret.now() }),
        Some(&cookie),
    );
    assert_eq!(confirmed.status, 200, "{}", confirmed.body);

    let reply = mfa_login(
        &env,
        "s256login@example.com",
        &secret.code_at(now_secs() + 30),
    );

    assert_eq!(reply.status, 202, "{}", reply.body);
}

#[test]
fn records_without_an_algorithm_are_sha256() {
    // Secrets stored before the algorithm field existed were SHA-256.
    let old = r#"{"key_id":"k1","nonce":"AAAA","ciphertext":"BBBB"}"#;
    let record: foundation_auth::server::services::EncryptedSecret =
        serde_json::from_str(old).unwrap();
    assert_eq!(record.algorithm, foundation_auth::TotpAlgorithm::Sha256);
}
