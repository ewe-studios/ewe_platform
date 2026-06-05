# Feature 07: Auth UI Package

## Description

Create `foundation_auth_ui` — a crate of auth-specific UI components built on `foundation_wasm_ui`. These components connect to the foundation_auth server's JSON API (spec 38) and provide the frontend for authentication flows.

## Crate Structure

```
backends/foundation_auth_ui/
├── Cargo.toml
│   [dependencies]
│   foundation_wasm_ui = { workspace = true, features = ["full"] }
│   foundation_auth = { workspace = true }  # for types (AuthCredential, etc.)
│   serde = { workspace = true }
│   serde_json = { workspace = true }
│
└── src/
    ├── lib.rs                # Re-exports all components
    ├── login_form.rs         # Login form component
    ├── register_form.rs      # Registration form
    ├── mfa_challenge.rs      # MFA/TOTP input
    ├── session_status.rs     # Session indicator
    ├── user_profile.rs       # User profile display
    └── auth_layout.rs        # Auth page layout wrapper
```

## Components

### LoginForm

```rust
/// Login form component — connects to /auth/v1/login.
pub struct LoginForm {
    email: Signal<String>,
    password: Signal<String>,
    loading: Signal<bool>,
    error: Signal<Option<String>>,
    mfa_required: Signal<bool>,
    mfa_challenge_id: Signal<Option<String>>,
    on_login: Signal<Option<OnLoginCallback>>,
    base_url: Signal<String>,
}

impl Component for LoginForm {
    const TAG_NAME: &'static str = "auth-login-form";

    fn render(&self) -> TemplateResult {
        html! {
            <form on:submit={prevent_default_and_login}>
                <h2>Sign In</h2>

                @if self.error.get().is_some() {
                    <div class="error">{self.error.get().unwrap()}</div>
                }

                <input type="email"
                       .value:bind={self.email}
                       placeholder="Email"
                       required />

                <input type="password"
                       .value:bind={self.password}
                       placeholder="Password"
                       required />

                @if self.mfa_required.get() {
                    <mfa-challenge
                        challenge-id={self.mfa_challenge_id.get().unwrap()}
                        on:verified={handle_mfa_success}
                        on:failed={handle_mfa_failure}
                        base-url={self.base_url.get()}
                    />
                }

                <button type="submit" disabled={self.loading.get()}>
                    @if self.loading.get() {
                        <span class="spinner"></span> Signing in...
                    } @else {
                        Sign In
                    }
                </button>
            </form>
        }
    }
}
```

### RegisterForm

Similar to LoginForm but calls `/auth/v1/register` with email, username, password, password confirmation.

### MfaChallenge

```rust
/// MFA challenge component — TOTP code input with auto-submit.
pub struct MfaChallenge {
    code: Signal<String>,
    error: Signal<Option<String>>,
    loading: Signal<bool>,
    attempts_left: Signal<u32>,
}
```

### SessionStatus

```rust
/// Session status indicator — shows login state, user info, logout button.
pub struct SessionStatus {
    authenticated: Signal<bool>,
    user_email: Signal<Option<String>>,
    on_logout: Signal<Option<OnLogoutCallback>>,
}
```

### AuthLayout

```rust
/// Auth page layout wrapper — centering, branding, slots for content.
pub struct AuthLayout {
    brand_name: Signal<String>,
    logo_url: Signal<Option<String>>,
}
```

## Server Integration

Components use `foundation_netio` (wasm: fetch API) to call the auth server:

```rust
async fn login(base_url: &str, email: &str, password: &str) -> Result<LoginResponse, AuthError> {
    // POST /auth/v1/login
    // { "email": "...", "password": "..." }
    // Returns: { "status": "authenticated" | "mfa_required", ... }
}

async fn submit_mfa(base_url: &str, challenge_id: &str, code: &str) -> Result<MfaResponse, AuthError> {
    // POST /auth/v1/mfa
    // { "challenge_id": "...", "code": "..." }
}
```

## Dependencies

- `foundation_wasm_ui = { workspace = true }`
- `foundation_auth = { workspace = true }` (for shared types)
- `foundation_netio = { workspace = true }` (for HTTP requests in wasm)
- `serde`, `serde_json`

## Testing

- LoginForm renders with email + password inputs
- Submit button disabled while loading
- Error message displayed on failed login
- MFA challenge appears when server returns mfa_required
- SessionStatus shows logged-in state with user email
- Logout button clears session state
