//! Session cookies are `Secure`, `HttpOnly` and `SameSite`.

use foundation_core::valtron::valtron_test;

use crate::integration::tos_login_gate::{config, start};

#[valtron_test(threads = 8, timeout = 60000)]
fn session_cookies_are_secure_http_only_and_same_site() {
    let env = start(config());
    env.register("cookie@example.com");

    let login = env.login("cookie@example.com");

    assert_eq!(login.status, 202, "{}", login.body);
    let cookies = login.set_cookies();
    let token = cookies
        .iter()
        .find(|c| c.starts_with("session_token="))
        .expect("session token cookie");
    for attribute in ["Secure", "HttpOnly", "SameSite=Lax"] {
        assert!(token.contains(attribute), "{attribute} missing in: {token}");
    }
    let data = cookies
        .iter()
        .find(|c| c.starts_with("session_data="))
        .expect("session data cookie");
    for attribute in ["Secure", "SameSite=Lax"] {
        assert!(data.contains(attribute), "{attribute} missing in: {data}");
    }

    // The cookies that clear the session carry the same attributes.
    let out = env.post(
        "/auth/v1/oidc/logout",
        &serde_json::json!({}),
        login.session_cookie().as_deref(),
    );
    for cookie in out.set_cookies() {
        assert!(cookie.contains("Secure"), "clearing cookie: {cookie}");
    }
}

#[valtron_test(threads = 8, timeout = 60000)]
fn secure_flag_can_be_turned_off_for_local_http_only() {
    let env = start(config().with_secure_cookies(false));
    env.register("insecure@example.com");

    let login = env.login("insecure@example.com");

    let token = login
        .set_cookies()
        .into_iter()
        .find(|c| c.starts_with("session_token="))
        .expect("session token cookie");
    assert!(!token.contains("Secure"), "{token}");
    assert!(
        token.contains("HttpOnly") && token.contains("SameSite=Lax"),
        "{token}"
    );
}
