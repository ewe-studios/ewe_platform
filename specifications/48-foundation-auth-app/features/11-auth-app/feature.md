---
feature: "Auth App"
description: "Combined app in /apps/ — mounts server + UI, native + CF, foundation_testbed E2E"
status: "pending"
priority: "high"
depends_on: ["01-complete-oidc-handlers", "02-login-mfa-handlers", "03-user-registration", "04-password-reset", "05-proof-of-work", "06-webauthn-fido2", "07-terms-of-service", "08-template-config-api", "09-account-management", "10-auth-ui-package"]
estimated_effort: "medium"
created: 2026-06-16
---

# Feature 11: Auth App

## Description

Combine the IdP server (`foundation_auth` with `server` feature) and the WASM UI
(`foundation_auth_ui`) into a single deployable under `/apps/foundation_auth_app/`.

## Location

```
apps/foundation_auth_app/
├── Cargo.toml
│   [dependencies]
│   foundation_auth = { workspace = true, features = ["server"] }
│   foundation_auth_ui = { workspace = true }
│   foundation_http = { workspace = true }
│   foundation_db = { workspace = true }
│   foundation_testbed = { workspace = true }
│
└── src/
    ├── lib.rs           # App factory
    ├── main.rs          # Native binary
    └── cf_worker.rs     # CF Worker
```

## Route dispatching

| Path prefix | Handler |
|---|---|
| `/auth/v1/*` | IdP Server (foundation_auth server module) |
| `/oidc/*` | IdP Server |
| `/.well-known/*` | IdP Server |
| `/` | WASM UI (HTML shell + wasm binary) |
| `/login`, `/register`, `/account` | WASM UI |

## Native binary

```rust
fn main() {
    let db = init_db()?;
    let config = IdpConfig::new("https://auth.example.com".into());
    let server = IdpServer::new(config);
    let mut app = server.http_app();
    app.mount("/", foundation_auth_ui::wasm_handler());
    app.server("127.0.0.1:8080").run()?;
}
```

## foundation_testbed E2E

```rust
#[test]
fn test_login_flow() {
    let app = TestApp::new(AuthApp::in_memory_test());
    app.db().create_test_user("user@test.com", "password123");

    let page = app.navigate("/login");
    page.find_input("email").fill("user@test.com");
    page.find_button("Continue").click();
    page.find_input("password").fill("password123");
    page.find_button("Login").click();

    assert!(page.current_url().starts_with("/oidc/callback"));
    assert!(app.db().has_active_session("user@test.com"));
}
```

## Testing

- Native binary starts, serves API + UI
- E2E login flow (UI → server → session created)
- E2E registration (UI → server → user created)
- E2E wrong password → error displayed
- CF Worker dispatches correctly
