# Feature 06: Auth UI Package

Built from the catalog (feature 05), styled after rauthy's frontend.
**Last feature in the spec** — all catalog components and machinery exist
before this is implemented.

## 1. Description

Create `foundation_auth_ui` — a crate of auth-specific UI pages and
components built on `foundation_wasm_ui` and the headless catalog (F1–F8).
These pages connect to an auth server's JSON API (spec 38 / rauthy-compatible)
and provide the complete frontend for authentication flows.

The scope is the **user-facing** pages only — no admin UI. Admin is a
separate concern.

## 2. Rauthy frontend review — what we cover

Every non-admin page in rauthy's Svelte frontend was reviewed
(2026-06-13, `rauthy/frontend/src/routes/`). The table below maps each
page to our component and the catalog primitives it uses:

| rauthy route | our component | catalog deps |
|---|---|---|
| `/` (home) | `AuthHome` | F1 button, layout (ContentCenter) |
| `/oidc/authorize` | `LoginPage` | F1 button, F7 input/password, F4 modal, F6 select, M6 field state |
| `/users/register` | `RegisterPage` | F1 button, F7 form/input, F6 select (providers), F4 modal (ToS) |
| `/users/password_reset` | `PasswordResetRequest` | F1 button, F7 input, F7 form |
| `/users/{id}/reset/reset` | `PasswordSetPage` | F1 button, F7 input/password, F7 field (policy), F2 passkey |
| `/oidc/logout` | `LogoutPage` | F1 button, layout |
| `/device` | `DeviceAuthPage` | F1 button, F7 input, F7 form |
| `/oidc/callback` | `ProviderCallback` | (pass-through, loading indicator only) |
| `/users/{id}/email_confirm` | `EmailConfirmPage` | F1 button, layout |
| `/users/{id}/revoke` | `RevokePage` | F1 button, layout |
| `/account` | `AccountPage` | F3 tabs, F7 form, F2 switch, F4 dialog, F4 toast, F8 progress |
| `/fedcm` | `FedCmPage` | (pass-through, browser-native) |
| `/error` | `ErrorPage` | layout, error display |
| any (layout) | `AuthLayout` | Main shell, ThemeSwitch, LangSelector |

### Shared UI building blocks (from rauthy `lib/`)

These appear on every page and are **catalog components reused**, not
auth-specific:

- **ContentCenter** — centered card layout (F4 popover/dialog positioning
  at page center; or a static layout helper).
- **Button** — F1 button with levels (primary/secondary/tertiary/invisible)
  and loading state (`data-loading`, `aria-busy`).
- **Input / InputPassword** — F7 form controls with field state (M6).
- **Form** — F7 form with submit interception and PoW field injection.
- **ThemeSwitch** — dark/light toggle (static config, toggles a class on
  `<body>`).
- **LangSelector** — language selector (F6 select variant).
- **ClientLogo** — dynamic client branding (F1 avatar variant).
- **TosAccept** — terms-of-service display and acceptance (F4 dialog + F7 checkbox).
- **WebauthnRequest** — passkey/MFA challenge (pass-through to browser
  WebAuthn API; a thin JS shim invoked from a component callback).

## 3. Theme and design tokens

rauthy's `theme.css` uses a small set of HSL tokens. We adopt the same
token names so rauthy CSS drops in as a starting stylesheet:

| token | light | dark | use |
|---|---|---|---|
| `--text` | `200 5% 37%` | `34 5% 75%` | body text |
| `--text-high` | `200 15% 25%` | `34 7% 90%` | headings, important text |
| `--bg` | `34 25% 97%` | `200 40% 6%` | background |
| `--bg-high` | `34 20% 90%` | `200 20% 17%` | elevated surfaces, borders |
| `--action` | `34 100% 40%` | `34 100% 59%` | buttons, links, success |
| `--accent` | `246 60% 53%` | `246 60% 53%` | accents, blockquotes |
| `--error` | `15 100% 37%` | `15 100% 37%` | error text, invalid borders |
| `--btn-text` | `white` | `hsl(var(--bg))` | button text color |
| `--border-radius` | `5px` | `5px` | all rounded corners |

CSS rules: `color: hsl(var(--text))`, `background: hsl(var(--bg))`,
`border-color: hsl(var(--bg-high))`. The theme toggle adds/removes
`.theme-dark` / `.theme-light` on `<body>`. Media query
`prefers-color-scheme: dark` provides the default.

## 4. Crate structure

```
backends/foundation_auth_ui/
├── Cargo.toml
│   [dependencies]
│   foundation_wasm_ui = { workspace = true, features = ["full"] }
│   foundation_ui_components = { workspace = true }    # headless catalog
│   foundation_signals = { workspace = true }
│   foundation_netio = { workspace = true }           # wasm fetch API
│   serde = { workspace = true }
│   serde_json = { workspace = true }
│
└── src/
    ├── lib.rs              # Re-exports all pages
    ├── layout.rs           # AuthLayout, AuthHome, ErrorPage
    ├── login.rs            # LoginPage (email → password → MFA/WebAuthn flow)
    ├── register.rs         # RegisterPage (dynamic fields, providers, ToS)
    ├── password.rs         # PasswordResetRequest + PasswordSetPage
    ├── logout.rs           # LogoutPage
    ├── device.rs           # DeviceAuthPage
    ├── callback.rs         # ProviderCallback (loading + redirect)
    ├── email_confirm.rs    # EmailConfirmPage
    ├── revoke.rs           # RevokePage
    ├── account.rs          # AccountPage (tabs: info, security, devices, MFA)
    ├── fedcm.rs            # FedCmPage (pass-through)
    ├── api.rs              # HTTP client functions (login, register, etc.)
    ├── types.rs            # Request/response types (serde structs)
    ├── pow.rs              # Proof-of-Work solver (async, worker-like)
    ├── tos.rs              # TosAccept component
    ├── webauthn.rs         # WebAuthN shim (JS FFI bindings)
    └── session.rs          # Session management helpers
```

## 5. Components — detailed specs

### 5.1 AuthLayout

```rust
/// Auth page layout wrapper — ContentCenter shell with theme/lang controls.
/// Static: brand_name, logo_url.
pub fn auth_layout(
    config: AuthLayoutConfig,
    slots: AuthLayoutSlots,   // { header, children, footer }
) -> Html {
    // Wraps children in a centered card, pins ThemeSwitch + LangSelector
    // to the top-right corner. Renders <noscript> fallback.
}
```

### 5.2 AuthHome

```rust
/// Landing page — Account Login, Register (if open), Admin Login (if not hidden).
/// Signals: is_reg_open, hide_admin_button (from Template API).
pub fn auth_home(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    is_reg_open: SignalGetter<bool>,
    hide_admin_button: SignalGetter<bool>,
    on_account_login: Callback,
    on_register: Callback,
    on_admin_login: Callback,
) -> Html {
    // Three stacked buttons, conditional on config signals.
}
```

### 5.3 LoginPage (the main flow — rauthy `/oidc/authorize`)

The most complex page. Multi-step flow driven by server responses:

1. **Email enter** → POST `/authorize` → 400 (needs password) or 202 (done) or 200 (MFA)
2. **Password enter** → POST `/authorize` → 202 (done) or 200 (MFA/WebAuthn) or 4xx (error)
3. **MFA/WebAuthn** → browser passkey prompt → 202 (done) or error
4. **ToS update** → accept/cancel → 202 or restart

```rust
/// Login page — email → password → MFA/WebAuthn, provider login, ToS.
pub fn login_page(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: LoginConfig,    // base_url, client_id, redirect_uri, scopes, etc.
) -> Html {
    // Signals (internal):
    //   email: Signal<String>
    //   password: Signal<String>
    //   needs_password: Signal<bool>    // server returned "need password"
    //   mfa_purpose: Signal<Option<MfaPurpose>>  // WebAuthn challenge active
    //   is_loading: Signal<bool>
    //   error: Signal<Option<String>>
    //   too_many_requests: Signal<bool>  // 429 with countdown
    //   show_reset: Signal<bool>         // password reset inline
    //   providers: Signal<Vec<AuthProvider>>
    //   tos: Signal<Option<TosData>>
    //   is_reg_open: Signal<bool>
    //   client_logo_updated: Signal<i64>
    //   client_name: Signal<String>
    //   client_uri: Signal<String>

    // Static:
    //   client_id, redirect_uri, scopes, nonce, code_challenge, state (from URL)
    //   idp_hint (auto-login via provider)
    //   atproto_id (if Bluesky provider exists)

    // Renders:
    //   - ClientLogo + home link
    //   - Email input (always)
    //   - Password input (when needs_password is true)
    //   - "Forgot password" → inline reset form
    //   - Login button (submit)
    //   - Register link (if is_reg_open)
    //   - Provider buttons (separator + each provider)
    //   - WebAuthnRequest (when mfa_purpose set)
    //   - TosAccept dialog (when tos set)
    //   - Error message
    //   - 429 cooldown overlay
}
```

**Server response handling:**

| status | meaning | action |
|---|---|---|
| 202 | authenticated | redirect to `Location` header |
| 200 | WebAuthn required | show passkey prompt |
| 205 | needs profile update | show modal → navigate to account |
| 206 | ToS update needed | show ToS dialog |
| 400 | bad request | show error |
| 403 | forbidden (group prefix / password expired) | show specific error |
| 406 | client forces MFA, user has none | show error + link to account |
| 429 | too many requests | show countdown from `x-retry-not-before` |
| else (first try, no password) | password needed | reveal password input |
| else (wrong credentials) | auth failed | show error + "forgot password" link |

### 5.4 RegisterPage

```rust
/// Registration page — dynamic fields, providers, ToS, PoW.
pub fn register_page(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: RegisterConfig,   // base_url, redirect_uri
) -> Html {
    // Signals (internal):
    //   email: Signal<String>
    //   preferred_username: Signal<Option<String>>    // conditional
    //   given_name, family_name: Signal<Option<String>>  // conditional
    //   user_values: Signal<UserValuesRequest>         // birthdate, tz, street, zip, city, country, phone
    //   is_loading: Signal<bool>
    //   error: Signal<Option<String>>
    //   success: Signal<bool>
    //   providers: Signal<Vec<AuthProvider>>
    //   tos: Signal<Option<TosData>>
    //   restricted_domain: Signal<Option<String>>      // email domain restriction
    //   config_data: Signal<Option<UserValuesConfig>>  // which fields are required/optional/hidden

    // Static:
    //   redirect_uri (from URL param)

    // Renders:
    //   - Title + domain restriction notice (if applicable)
    //   - Form with dynamic fields (driven by UserValuesConfig)
    //   - Submit button
    //   - Success message → auto-redirect after 3s
    //   - Error message
    //   - Provider registration buttons (separator + each)
    //   - TosAccept dialog (force-accept on registration)
}
```

**Dynamic field logic:** `UserValuesConfig` from the server declares each
field as `required`, `optional`, or `hidden`. Required fields render with
`required` attribute and `*` label. Optional fields render normally.
Hidden fields are omitted. If ≥5 fields are visible, the form switches to
a 2-column grid at ≥35rem viewport.

### 5.5 PasswordResetRequest

```rust
/// Password reset request — enter email, receive reset link.
pub fn password_reset_request(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: PwResetRequestConfig,  // base_url, email_hint (from URL)
) -> Html {
    // Signals:
    //   email: Signal<String>
    //   is_loading: Signal<bool>
    //   error: Signal<Option<String>>
    //   success: Signal<bool>

    // Renders:
    //   - ClientLogo
    //   - Title + description
    //   - Email input (pre-filled from email_hint if present)
    //   - Submit button
    //   - Success message
    //   - Error message
}
```

### 5.6 PasswordSetPage

```rust
/// Set new password or register passkey — from magic link.
pub fn password_set_page(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: PwSetConfig,  // base_url, user_id, magic_link_id, csrf_token
) -> Html {
    // Signals:
    //   password: Signal<String>
    //   password_confirm: Signal<String>
    //   passkey_name: Signal<String>
    //   account_type_new: Signal<Option<AccountType>>  // "passkey" | "password"
    //   mfa_purpose: Signal<Option<MfaPurpose>>        // if needs_mfa
    //   is_loading: Signal<bool>
    //   error: Signal<Option<String>>
    //   success: Signal<bool>
    //   policy_accepted: Signal<bool>

    // Static:
    //   request_type: "new_user" | "password_reset" (from URL)
    //   password_policy: PasswordPolicy (from template API)
    //   needs_mfa: bool

    // Renders:
    //   - For new_user: choice between passkey and password
    //     - Passkey: passkey name input + register button + WebAuthn
    //     - Password: password + confirm inputs + policy checker + generate button
    //   - For password_reset: password + confirm inputs + policy checker
    //   - WebAuthnRequest if needs_mfa
    //   - Success message → auto-redirect after 5s
    //   - Error message
}
```

### 5.7 LogoutPage

```rust
/// Logout confirmation — confirm or cancel.
pub fn logout_page(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: LogoutConfig,  // post_logout_redirect_uri, id_token_hint, state
) -> Html {
    // Signals:
    //   is_loading: Signal<bool>

    // Renders:
    //   - Title + confirmation message
    //   - Logout button (POST /oidc/logout)
    //   - Cancel button (redirects to home)
    //   - Error message
}
```

### 5.8 DeviceAuthPage

```rust
/// Device authorization — enter user code, accept/decline scopes.
pub fn device_auth_page(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: DeviceAuthConfig,  // base_url, code (from URL param)
) -> Html {
    // Signals:
    //   user_code: Signal<String>
    //   user_code_length: Signal<u8>    // from template API (default 8)
    //   scopes: Signal<Option<Vec<String>>>
    //   is_accepted: Signal<bool>
    //   is_declined: Signal<bool>
    //   is_loading: Signal<bool>
    //   error: Signal<Option<String>>
    //   session: Signal<Option<SessionInfo>>

    // Renders (state machine):
    //   1. No scopes yet: user code input + submit
    //   2. Scopes received: list scopes + accept/decline buttons
    //   3. Accepted: success message → redirect to account
    //   4. Declined: decline message
    //   - Error message throughout
}
```

### 5.9 ProviderCallback

```rust
/// Provider OAuth callback — loading indicator while redirecting.
pub fn provider_callback(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: CallbackConfig,  // base_url, code, state (from URL)
) -> Html {
    // Renders: loading spinner → POST to backend → redirect on response.
    // Minimal UI: "Processing..." with spinner.
}
```

### 5.10 EmailConfirmPage

```rust
/// Email change confirmation — shows old → new email, login button.
pub fn email_confirm_page(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: EmailConfirmConfig,  // old_email, new_email (from template API)
) -> Html {
    // Renders:
    //   - Title
    //   - "Email changed from X to Y"
    //   - "You can now log in"
    //   - Login button → /account
}
```

### 5.11 RevokePage

```rust
/// Account revocation — shows revocation confirmation.
pub fn revoke_page(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: RevokeConfig,  // from template API
) -> Html {
    // Renders: revocation confirmation message + optional re-register link.
}
```

### 5.12 AccountPage

```rust
/// Account management dashboard — tabs for info, security, devices, MFA.
pub fn account_page(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: AccountConfig,  // base_url
) -> Html {
    // Signals:
    //   user: Signal<Option<UserResponse>>
    //   web_id_data: Signal<Option<WebIdResponse>>
    //   active_tab: Signal<String>
    //   is_loading: Signal<bool>
    //   error: Signal<Option<String>>

    // Renders:
    //   - Tabs: Info, Security, Devices, MFA, WebID, Other
    //   - Info tab: email, username, display name, profile picture
    //   - Security tab: password change, passkeys (add/remove/name), sessions
    //   - Devices tab: list of active sessions/devices, revoke
    //   - MFA tab: TOTP setup, passkey management
    //   - WebID tab: WebID configuration
    //   - Other tab: account deletion, data export
    //   - ThemeSwitch + LangSelector
}
```

### 5.13 ErrorPage

```rust
/// Generic error page — displays error message, back-to-home link.
pub fn error_page(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    config: ErrorConfig,  // status_code, message, request_id
) -> Html {
    // Renders: error code, message, optional details, home link.
}
```

### 5.14 FedCmPage

```rust
/// FedCM pass-through — minimal, browser handles the flow.
pub fn fedcm_page(
    ctx: &Context, rcv: &SharedInstructionReceiver,
) -> Html {
    // Renders: loading indicator → browser FedCM API handles the rest.
}
```

## 6. API layer

### 6.1 HTTP client (`api.rs`)

All components use `foundation_netio` (wasm `fetch` API) for HTTP calls.
No server-side HTTP in this crate — it's a wasm-only consumer.

```rust
// Core auth functions:
async fn login(ctx: &AuthContext, req: &LoginRequest) -> Result<LoginResponse, AuthError>;
async fn register(ctx: &AuthContext, req: &RegisterRequest) -> Result<RegisterResponse, AuthError>;
async fn request_password_reset(ctx: &AuthContext, req: &ResetRequest) -> Result<(), AuthError>;
async fn reset_password(ctx: &AuthContext, req: &PwResetRequest) -> Result<(), AuthError>;
async fn logout(ctx: &AuthContext, req: &LogoutRequest) -> Result<(), AuthError>;
async fn verify_device(ctx: &AuthContext, req: &DeviceVerifyRequest) -> Result<DeviceVerifyResponse, AuthError>;
async fn session_info(ctx: &AuthContext) -> Result<SessionInfoResponse, AuthError>;
async fn fetch_user(ctx: &AuthContext, user_id: &str) -> Result<UserResponse, AuthError>;

// Template/config fetchers (used to hydrate static/conditional config):
async fn fetch_tos_latest(ctx: &AuthContext) -> Result<Option<TosResponse>, AuthError>;
async fn fetch_providers(ctx: &AuthContext) -> Result<Vec<AuthProvider>, AuthError>;
async fn fetch_user_values_config(ctx: &AuthContext) -> Result<UserValuesConfig, AuthError>;
async fn fetch_password_policy(ctx: &AuthContext) -> Result<PasswordPolicy, AuthError>;
```

### 6.2 Request types (`types.rs`)

```rust
#[derive(Serialize)]
struct LoginRequest {
    email: String,
    password: Option<String>,
    client_id: String,
    redirect_uri: String,
    state: Option<String>,
    nonce: Option<String>,
    scopes: Vec<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    pow: String,
}

#[derive(Serialize)]
struct RegisterRequest {
    email: String,
    preferred_username: Option<String>,
    given_name: Option<String>,
    family_name: Option<String>,
    user_values: Option<UserValuesRequest>,
    redirect_uri: Option<String>,
    pow: String,
}

#[derive(Serialize)]
struct UserValuesRequest {
    birthdate: Option<String>,
    tz: Option<String>,
    street: Option<String>,
    zip: Option<String>,
    city: Option<String>,
    country: Option<String>,
    phone: Option<String>,
}

#[derive(Deserialize)]
struct LoginResponse {
    status: LoginStatus,  // Authenticated, MfaRequired, TosRequired, ...
    // MfaRequired fields:
    user_id: Option<String>,
    mfa_code: Option<String>,
    // TosRequired fields:
    tos_await_code: Option<String>,
}
```

### 6.3 Proof of Work (`pow.rs`)

rauthy uses a PoW challenge on every auth mutation. The solver runs
asynchronously (off-main-thread via a worker pattern, or cooperatively
on the main thread with yield points).

```rust
async fn solve_pow(ctx: &AuthContext) -> Option<String>;
// Fetches the PoW challenge from /auth/v1/pow, solves it, returns the solution.
// Runs in a tight loop with cooperative yielding for wasm.
```

### 6.4 Session management (`session.rs`)

```rust
// Creates/validates a session, manages CSRF token in localStorage,
// provides the browser_id for PoW correlation.
async fn create_session(ctx: &AuthContext) -> Result<SessionInfo, AuthError>;
fn save_csrf_token(token: &str);
fn get_csrf_token() -> Option<String>;
fn purge_storage();   // clear all localStorage on logout
```

### 6.5 WebAuthn shim (`webauthn.rs`)

Thin FFI bindings to the browser's WebAuthn API (`navigator.credentials`).

```rust
// Registration:
async fn webauthn_register(user_id: &str, passkey_name: &str, ...) -> Result<(), String>;

// Authentication (MFA challenge):
async fn webauthn_login(user_id: &str, purpose: MfaPurpose) -> Result<WebAuthnResult, String>;

// Result contains either:
//   - redirect location (HTTP 202 case)
//   - MFA code to submit with the login
//   - ToS await code
```

### 6.6 ToS component (`tos.rs`)

```rust
/// Terms of Service display and acceptance.
/// Shown as a modal dialog (F4) with the ToS markdown rendered inline.
pub fn tos_accept(
    ctx: &Context, rcv: &SharedInstructionReceiver,
    tos: SignalGetter<TosData>,
    tos_accept_code: SignalGetter<Option<String>>,  // Some = login flow, None = registration
    on_accept: Callback,   // called with the ToS acceptance result
    on_cancel: Callback,
    force_accept: bool,    // skip "cancel" for registration
    skip_request: bool,    // registration mode: no code needed
) -> Html {
    // Renders ToS content (from MD rendered via a worker) + accept/cancel buttons.
    // On accept: POST to /tos/accept with the code (if login flow).
}
```

## 7. Server integration map

The components call these rauthy-compatible API endpoints:

| Component | Method | Endpoint | Purpose |
|---|---|---|---|
| LoginPage | POST | `/auth/v1/oidc/authorize` | Login (email, optional password) |
| LoginPage | POST | `/auth/v1/oidc/authorize/refresh` | Refresh existing session |
| LoginPage | POST | `/auth/v1/oidc/session` | Create session (dev mode) |
| LoginPage | POST | `/auth/v1/dev/browser_id` | Browser ID (dev mode) |
| LoginPage | GET | `/auth/v1/tos/latest` | Fetch current ToS |
| LoginPage | POST | `/auth/v1/users/request_reset` | Request password reset |
| RegisterPage | POST | `/auth/v1/users/register` | Register new user |
| RegisterPage | POST | `/auth/v1/dev/register` | Register (dev mode) |
| RegisterPage | GET | (template) | UserValuesConfig, providers, ToS |
| PasswordResetRequest | POST | `/auth/v1/users/request_reset` | Request reset |
| PasswordSetPage | PUT | `/auth/v1/users/{id}/reset` | Set new password |
| PasswordSetPage | POST | `/auth/v1/users/{id}/webid/register` | Register passkey |
| LogoutPage | POST | `/auth/v1/oidc/logout` | Logout |
| DeviceAuthPage | GET | `/auth/v1/oidc/sessioninfo` | Session check |
| DeviceAuthPage | POST | `/auth/v1/oidc/device/verify` | Verify device code |
| AccountPage | GET | `/auth/v1/users/{id}` | Fetch user info |
| AccountPage | GET | `/auth/v1/users/{id}/webid/data` | Fetch WebID config |
| All | POST | `/auth/v1/pow` | PoW challenge |
| All | GET | `/auth/v1/templates/...` | Template data (config values) |

**Dev mode:** When `IS_DEV` is true, endpoints are prefixed with
`/auth/v1/dev/` instead of `/auth/v1/`.

## 8. Testing

Each page component gets:

1. **Render test** — produces the expected DOM shape (inputs, buttons, labels).
2. **Interaction test** — click submit, verify loading state, error display.
3. **API test** (mocked) — mock the fetch responses, verify correct
   behavior for each HTTP status code (200, 202, 204, 205, 206, 400, 403,
   404, 406, 429).
4. **Morph test** — verify the page survives a DOM morph (content changes
   without full re-render).
5. **Styled example** — with the rauthy CSS applied, the page visually
   matches the reference screenshots.

Specific test cases:

- LoginForm renders email + password inputs, submit button
- Submit button disabled while loading (`data-loading`)
- Error message displayed on 400/403 response
- MFA/WebAuthn prompt appears on HTTP 200 response
- ToS dialog appears on HTTP 206 response
- 429 response shows countdown from `x-retry-not-before` header
- Register page shows dynamic fields based on `UserValuesConfig`
- Register page shows provider buttons when providers exist
- SessionStatus shows logged-in state with user email
- Logout button clears session state and redirects
- Device page accepts user code of correct length, shows scopes for approval

## 9. Dependencies

- `foundation_wasm_ui` — core rendering, mounting, App bootstrap
- `foundation_ui_components` — headless catalog (F1 button, F7 form/input,
  F4 dialog/modal, F3 tabs, etc.)
- `foundation_signals` — reactive signals
- `foundation_netio` — HTTP fetch API (wasm only)
- `serde`, `serde_json` — serialization for API types

## 10. What is NOT in scope

- **Admin UI** — rauthy's `/admin/*` routes (users management, clients,
  config, events, etc.) are a separate spec.
- **Custom theming engine** — we use the rauthy CSS token system as-is;
  a design system layer sits above this crate.
- **i18n runtime** — translations are a separate concern; the components
  use static English strings for now, with i18n hooks left as extension
  points.
- **Server-side rendering** — this crate is wasm-only. SSR of auth pages
  is handled by the auth server itself (rauthy ships HTML).
