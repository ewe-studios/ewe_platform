//! Registration without a username.

use foundation_core::valtron::valtron_test;

use crate::integration::tos_login_gate::{config, start, Env};

fn register_without_username(env: &Env, email: &str) -> (usize, serde_json::Value) {
    let reply = env.post(
        "/auth/v1/dev/register",
        &serde_json::json!({ "email": email, "password": "ValidPass123!" }),
        None,
    );
    (reply.status, reply.body)
}

#[valtron_test(threads = 8, timeout = 60000)]
fn two_users_without_username_can_register() {
    let env = start(config());

    let first = register_without_username(&env, "nouser-1@example.com");
    let second = register_without_username(&env, "nouser-2@example.com");

    assert_eq!(first.0, 200, "{}", first.1);
    assert_eq!(second.0, 200, "{}", second.1);
    // Both can sign in, and a duplicate username is still refused.
    assert_eq!(env.login("nouser-1@example.com").status, 202);
    assert_eq!(env.login("nouser-2@example.com").status, 202);
}

#[valtron_test(threads = 8, timeout = 60000)]
fn a_taken_username_is_still_refused() {
    let env = start(config());
    let body = |email: &str| {
        serde_json::json!({
            "email": email,
            "password": "ValidPass123!",
            "preferred_username": "alice",
        })
    };

    let first = env.post("/auth/v1/dev/register", &body("a1@example.com"), None);
    let second = env.post("/auth/v1/dev/register", &body("a2@example.com"), None);

    assert_eq!(first.status, 200, "{}", first.body);
    assert_ne!(
        second.status, 200,
        "the username is unique: {}",
        second.body
    );
}
