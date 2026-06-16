---
feature: "Auth UI Package"
description: "WASM UI components — all auth pages built on spec 42 catalog (moved from spec 42 feature 06)"
status: "pending"
priority: "high"
depends_on: ["01-complete-oidc-handlers", "02-login-mfa-handlers", "03-user-registration", "04-password-reset", "05-proof-of-work", "06-webauthn-fido2", "07-terms-of-service", "08-template-config-api", "09-account-management", "spec-42 F00-F05"]
estimated_effort: "large"
created: 2026-06-16
---

# Feature 10: Auth UI Package

## Description

**Moved from spec 42 feature 06.** Placed AFTER all server endpoints (F01–F09)
so the UI has real API endpoints to drive.

Authoritative detailed spec: `specifications/42-ui-component/features/06-auth-ui-package/feature.md`

## Pages

| Page | Description |
|---|---|
| `AuthLayout` | Auth page layout shell with theme/lang controls |
| `AuthHome` | Landing page (login/register buttons) |
| `LoginPage` | Multi-step: email → password → MFA/WebAuthn |
| `RegisterPage` | Dynamic fields, providers, ToS |
| `PasswordResetRequest` | Request reset via email |
| `PasswordSetPage` | Set new password from magic link |
| `LogoutPage` | Logout confirmation |
| `DeviceAuthPage` | Device authorization flow |
| `AccountPage` | Dashboard (tabs: info, security, devices, MFA) |
| `ErrorPage` | Generic error display |

## Support modules

| Module | Description |
|---|---|
| `api.rs` | HTTP client via `foundation_netio` (wasm fetch) |
| `types.rs` | Request/response serde types |
| `pow.rs` | Client-side PoW solver (async, cooperative yielding) |
| `session.rs` | localStorage CSRF, purge on logout |
| `webauthn.rs` | WebAuthn JS FFI shim |
| `tos.rs` | ToS display/acceptance |

## Crate structure

```
backends/foundation_auth_ui/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── layout.rs
    ├── login.rs
    ├── register.rs
    ├── password.rs
    ├── logout.rs
    ├── device.rs
    ├── account.rs
    ├── callback.rs
    ├── api.rs
    ├── types.rs
    ├── pow.rs
    ├── webauthn.rs
    ├── session.rs
    └── tos.rs
```

## Testing

Per spec 42 feature 06:
1. Render test — expected DOM shape
2. Interaction test — click, loading state, error display
3. API test (mocked) — mock fetch responses for each HTTP status
4. Morph test — survives DOM morph
5. Styled example — rauthy CSS applied
