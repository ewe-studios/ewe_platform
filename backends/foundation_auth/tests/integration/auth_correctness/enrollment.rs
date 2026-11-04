//! Failed TOTP enrollment codes count toward the account lockout.

use foundation_auth::server::PasswordPolicy;
use foundation_core::valtron::valtron_test;

use crate::integration::tos_login_gate::{config, start};

#[valtron_test(threads = 8, timeout = 60000)]
fn enroll_confirm_attempts_are_limited() {
    let policy = PasswordPolicy {
        max_failed_attempts: 3,
        ..PasswordPolicy::default()
    };
    let env = start(config().with_password_policy(policy));
    env.register("limit@example.com");
    let cookie = env
        .login("limit@example.com")
        .session_cookie()
        .expect("session cookie");

    // Three wrong first codes, each with a fresh start: the third locks the account.
    for attempt in 1..=3 {
        let started = env.post(
            "/auth/v1/mfa/totp/enroll/start",
            &serde_json::json!({}),
            Some(&cookie),
        );
        assert_eq!(started.status, 200, "start {attempt}: {}", started.body);
        let confirmed = env.post(
            "/auth/v1/mfa/totp/enroll/confirm",
            &serde_json::json!({ "code": "000000" }),
            Some(&cookie),
        );
        assert_eq!(
            confirmed.status, 401,
            "confirm {attempt}: {}",
            confirmed.body
        );
    }

    let again = env.post(
        "/auth/v1/mfa/totp/enroll/start",
        &serde_json::json!({}),
        Some(&cookie),
    );
    assert_eq!(again.status, 423, "locked: {}", again.body);
    let login = env.login("limit@example.com");
    assert_eq!(login.status, 423, "the account is locked: {}", login.body);
}
