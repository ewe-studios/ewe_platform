//! Logout revokes the session; a good login resets the failed attempts.

use foundation_auth::TOTPSecret;
use foundation_core::valtron::valtron_test;

use crate::integration::tos_login_gate::{config, start, Env, Reply};

/// A session-only endpoint: 200 with a valid session, 401 without.
const NEEDS_SESSION: &str = "/auth/v1/mfa/totp/enroll/start";

fn bad_login(env: &Env, email: &str) -> Reply {
    env.post(
        "/auth/v1/oidc/authorize",
        &serde_json::json!({ "email": email, "password": "Wrong-Password-1!" }),
        None,
    )
}

#[valtron_test(threads = 8, timeout = 60000)]
fn logout_revokes_the_session() {
    let env = start(config());
    env.register("logout@example.com");
    let login = env.login("logout@example.com");
    let cookie = login.session_cookie().expect("session cookie");
    let before = env.post(NEEDS_SESSION, &serde_json::json!({}), Some(&cookie));
    assert_eq!(before.status, 200, "the session works: {}", before.body);

    let out = env.post(
        "/auth/v1/oidc/logout",
        &serde_json::json!({}),
        Some(&cookie),
    );

    assert_eq!(out.status, 200, "{}", out.body);
    assert_eq!(out.body["status"], "logged_out");
    let cleared = out
        .set_cookies()
        .into_iter()
        .find(|c| c.starts_with("session_token="))
        .expect("the session cookie is cleared");
    assert!(
        cleared.starts_with("session_token=;") || cleared.contains("Max-Age=0"),
        "cleared: {cleared}"
    );
    let after = env.post(NEEDS_SESSION, &serde_json::json!({}), Some(&cookie));
    assert_eq!(
        after.status, 401,
        "the old cookie no longer works: {}",
        after.body
    );

    // Logout with no session, or an unknown one, is not an error.
    let none = env.post("/auth/v1/oidc/logout", &serde_json::json!({}), None);
    let unknown = env.post(
        "/auth/v1/oidc/logout",
        &serde_json::json!({}),
        Some("session_token=unknown"),
    );
    assert_eq!(none.status, 200);
    assert_eq!(unknown.status, 200);
}

#[valtron_test(threads = 8, timeout = 60000)]
fn successful_login_resets_failed_attempts() {
    let env = start(config());
    env.register("attempts@example.com");

    assert_eq!(
        bad_login(&env, "attempts@example.com").body["attempts_remaining"],
        4
    );
    assert_eq!(
        bad_login(&env, "attempts@example.com").body["attempts_remaining"],
        3
    );
    let good = env.login("attempts@example.com");
    assert_eq!(good.status, 202, "{}", good.body);

    // The count starts again: 4 left after one new failure (it was 2 before).
    assert_eq!(
        bad_login(&env, "attempts@example.com").body["attempts_remaining"],
        4
    );
}

#[valtron_test(threads = 8, timeout = 60000)]
fn mfa_login_resets_failed_attempts_only_after_the_code() {
    let env = start(config());
    let user = env.register("mfaattempts@example.com");
    let secret: TOTPSecret = env.enroll(&user);

    assert_eq!(
        bad_login(&env, "mfaattempts@example.com").body["attempts_remaining"],
        4
    );
    assert_eq!(
        bad_login(&env, "mfaattempts@example.com").body["attempts_remaining"],
        3
    );

    // The password alone is not a full login: the count stays.
    let challenge = env.login("mfaattempts@example.com");
    assert_eq!(
        challenge.body["status"], "mfa_required",
        "{}",
        challenge.body
    );
    assert_eq!(
        bad_login(&env, "mfaattempts@example.com").body["attempts_remaining"],
        2
    );

    // The code completes the login: the count is reset.
    let done = env.post(
        "/auth/v1/mfa",
        &serde_json::json!({
            "challenge_id": challenge.body["challenge_id"],
            "code": secret.now(&env.storage.ctx),
        }),
        None,
    );
    assert_eq!(done.status, 202, "{}", done.body);
    assert_eq!(
        bad_login(&env, "mfaattempts@example.com").body["attempts_remaining"],
        4
    );
}
