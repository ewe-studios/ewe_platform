//! The route table: each (method, path) goes to one handler, as a whole path.

use foundation_auth::server::handlers::routes::{route_for, Route, ROUTES};
use foundation_core::valtron::valtron_test;
use foundation_http::SimpleMethod;

use crate::integration::tos_login_gate::{config, start};

/// Every route, with a concrete request that must reach it.
fn cases() -> Vec<(SimpleMethod, &'static str, Route)> {
    use SimpleMethod::{DELETE, GET, POST, PUT};
    vec![
        (
            GET,
            "/idp/.well-known/openid-configuration",
            Route::Discovery,
        ),
        (GET, "/.well-known/openid-configuration", Route::Discovery),
        (GET, "/idp/.well-known/jwks.json", Route::Jwks),
        (GET, "/idp/authorize", Route::Authorize),
        (POST, "/idp/token", Route::Token),
        (GET, "/idp/userinfo", Route::Userinfo),
        (POST, "/idp/introspect", Route::Introspect),
        (POST, "/idp/device/authorize", Route::DeviceAuthorize),
        (GET, "/idp/callback", Route::Callback),
        (POST, "/auth/v1/oidc/authorize", Route::Login),
        (POST, "/auth/v1/mfa", Route::Mfa),
        (
            POST,
            "/auth/v1/mfa/totp/enroll/start",
            Route::TotpEnrollStart,
        ),
        (
            POST,
            "/auth/v1/mfa/totp/enroll/confirm",
            Route::TotpEnrollConfirm,
        ),
        (POST, "/auth/v1/oidc/logout", Route::Logout),
        (POST, "/auth/v1/users/register", Route::Register),
        (POST, "/auth/v1/dev/register", Route::DevRegister),
        (
            POST,
            "/auth/v1/users/request_reset",
            Route::RequestPasswordReset,
        ),
        (PUT, "/auth/v1/users/u-1/reset", Route::SetPassword),
        (GET, "/auth/v1/pow", Route::PowChallenge),
        (POST, "/auth/v1/pow", Route::PowSolve),
        (GET, "/auth/v1/templates/config", Route::TemplateConfig),
        (
            GET,
            "/auth/v1/templates/password_policy",
            Route::PasswordPolicy,
        ),
        (GET, "/auth/v1/users/u-1", Route::GetUser),
        (PUT, "/auth/v1/users/u-1", Route::UpdateUser),
        (
            POST,
            "/auth/v1/users/u-1/change_password",
            Route::ChangePassword,
        ),
        (GET, "/auth/v1/users/u-1/sessions", Route::ListSessions),
        (POST, "/auth/v1/users/u-1/revoke", Route::RevokeUser),
        (
            POST,
            "/auth/v1/webauthn/register/start",
            Route::WebauthnRegisterStart,
        ),
        (
            POST,
            "/auth/v1/webauthn/register/finish",
            Route::WebauthnRegisterFinish,
        ),
        (
            POST,
            "/auth/v1/webauthn/login/start",
            Route::WebauthnAuthStart,
        ),
        (
            POST,
            "/auth/v1/webauthn/login/finish",
            Route::WebauthnAuthFinish,
        ),
        (DELETE, "/auth/v1/webauthn/p-1", Route::DeletePasskey),
        (PUT, "/auth/v1/webauthn/p-1", Route::RenamePasskey),
        (
            POST,
            "/auth/v1/passkey/login/start",
            Route::PasskeyLoginStart,
        ),
        (
            POST,
            "/auth/v1/passkey/login/finish",
            Route::PasskeyLoginFinish,
        ),
        (GET, "/auth/v1/tos/latest", Route::TosLatest),
        (POST, "/auth/v1/tos/accept", Route::TosAccept),
    ]
}

#[test]
fn every_route_reaches_its_handler() {
    for (method, path, expected) in cases() {
        assert_eq!(route_for(&method, path), Some(expected), "{method} {path}");
    }
    // Every row of the table is covered by a case above.
    for def in ROUTES {
        assert!(
            cases().iter().any(|(_, _, route)| *route == def.route),
            "no case for {:?}",
            def.route
        );
    }
}

#[test]
fn a_path_matches_as_a_whole_not_by_suffix_or_substring() {
    use SimpleMethod::{GET, POST, PUT};
    for (method, path) in [
        // a fixed path with something in front
        (GET, "/x/auth/v1/tos/latest"),
        (POST, "/evil/auth/v1/oidc/authorize"),
        // a fixed path with something after
        (GET, "/auth/v1/tos/latest/extra"),
        (POST, "/auth/v1/mfa/extra"),
        // `{id}` is one segment
        (GET, "/auth/v1/users/a/b"),
        (PUT, "/auth/v1/users/a/b/reset"),
        (GET, "/auth/v1/users/"),
        // the OIDC name is a whole segment
        (POST, "/idp/mytoken"),
        (GET, "/idp//userinfo"),
        // the right path with the wrong method
        (POST, "/auth/v1/tos/latest"),
        (GET, "/auth/v1/tos/accept"),
    ] {
        assert_eq!(route_for(&method, path), None, "{method} {path}");
    }
}

#[valtron_test(threads = 8, timeout = 60000)]
fn unknown_path_is_404() {
    let env = start(config());

    let unknown = env.get("/auth/v1/nothing/here", None);
    let wrong_method = env.post("/auth/v1/tos/latest", &serde_json::json!({}), None);
    let known = env.get("/auth/v1/tos/latest", None);

    assert_eq!(unknown.status, 404, "{}", unknown.body);
    assert_eq!(wrong_method.status, 404, "{}", wrong_method.body);
    assert_eq!(known.status, 200, "{}", known.body);
}
