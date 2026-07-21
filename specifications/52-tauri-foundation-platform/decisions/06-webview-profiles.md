# 06 — WebView profiles: trust boundaries and platform service access

**Date:** 2026-07-04
**Status:** Resolved

### Decision

Five WebView profiles define trust boundaries for content loaded inside the
platform. Each profile gates access to Tauri primitives, platform services
(`foundation_db`, `foundation_auth`, `foundation_nativeapis`, etc.), and
native capabilities. Profiles are assigned per-route by the session's route
handler and enforced by the platform at runtime.

### Profile taxonomy

| Profile | What loads here | Trust level |
|---|---|---|
| **`app`** | Bundled local WASM, app shell HTML, platform UI components. | Maximum. Full platform access. |
| **`trustedRemote`** | Server-rendered HTML from the application's own backend, authenticated. | High. Scoped platform access with server-requested privileges. |
| **`untrustedRemote`** | Third-party content, embedded pages, user-generated content rendered as HTML. | Minimal. Sandboxed. No platform access. |
| **`auth`** | Login screens, OAuth flows, credential entry. | Elevated isolation. Limited platform access; no credential store access except through auth APIs. |
| **`devtools`** | Developer diagnostics, debug panels, hot-reload interfaces. | Debug-only. Full access for development; stripped from production builds. |

### What each profile gates

**`app` profile — full platform access:**

| Service | Access |
|---|---|
| `foundation_db` | Full — SQLite, Turso, D1, R2, KV. Read/write to app databases. |
| `foundation_auth` | Full — `AuthManager`, credential store, token management, session credentials. |
| `foundation_nativeapis` | Full — IPC channels, native watchers, platform APIs. All Tauri plugins. |
| `foundation_http` | Full — custom protocol, fetch, SSE, WebSocket. All origins controlled by route policy. |
| `foundation_arrow` | Full — Arrow/ArrowIpc encode/decode, zero-copy data lanes. |
| `foundation_signals` | Full — reactive signals, template expansion. |
| Tauri commands | All registered commands accessible. |
| Tauri events | Full emit/listen. |
| Custom protocol | Full — can register protocol handlers, serve resources. |
| CSP | Relaxed — allows inline scripts (for WASM bootstrap), `blob:`, `data:`. Nonce-based script-src. |
| Native capabilities | All registered capabilities callable. |
| Navigation | Can navigate to any route. Can open external URLs (gated by route handler). |

**`trustedRemote` profile — scoped platform access:**

| Service | Access |
|---|---|
| `foundation_db` | Read-only through scoped queries. No schema modification. No D1/R2 admin. |
| `foundation_auth` | Can request auth-protected resources. Cannot access raw credential store. Token attachment handled by shell, not by remote content. |
| `foundation_nativeapis` | Selected capabilities only — those explicitly allowed by local policy. No IPC channel creation. |
| `foundation_http` | Can fetch from allowed origins (configured per-route). SSE/WS to the app's own backend. Custom protocol read-only. |
| `foundation_arrow` | Can receive Arrow/ArrowIpc (read). Cannot initiate encode/decode through native lanes. |
| `foundation_signals` | Can receive signal updates from the backend. Cannot create new signal graphs. |
| Tauri commands | Allowlisted subset — only commands explicitly permitted for this route. |
| Tauri events | Can listen for scoped events (this route's updates). Cannot emit to other routes. |
| Custom protocol | Can request resources through the protocol. Cannot register handlers. |
| CSP | Stricter — `script-src` limited to app origin + trusted CDN. No `unsafe-eval`. |
| Native capabilities | Allowlist per-route. E.g., camera allowed on video call route, denied on content route. |
| Navigation | Can navigate within the trusted origin. External links confirmed by route handler. |

**`untrustedRemote` profile — sandboxed:**

| Service | Access |
|---|---|
| `foundation_db` | None. No database access. |
| `foundation_auth` | None. No auth APIs. Tokens never sent to untrusted origins. |
| `foundation_nativeapis` | None. No native API access. |
| `foundation_http` | Fetch only to the origin that served the content. No custom protocol. No SSE/WS. |
| `foundation_arrow` | None. |
| `foundation_signals` | None. |
| Tauri commands | None. `invoke()` blocked for this WebView context. |
| Tauri events | None. Event bus isolated from untrusted content. |
| Custom protocol | Read-only, same-origin resources only. |
| CSP | Strictest — `default-src 'self'`. No inline scripts. No remote scripts unless same-origin. `sandbox` attribute applied. |
| Native capabilities | None. All capability requests denied. |
| Navigation | Same-origin only. External navigation opens in system browser (not in-app WebView). |

**`auth` profile — elevated isolation:**

| Service | Access |
|---|---|
| `foundation_db` | None (except auth session storage, through auth APIs only). |
| `foundation_auth` | Auth flow APIs only — `login()`, `logout()`, `refresh()`. No raw credential access. OAuth state managed by shell, not by content. |
| `foundation_nativeapis` | Biometric auth only (if configured). No other native APIs. |
| `foundation_http` | Only to the auth provider's origin (OAuth endpoints, token endpoints). No other origins. |
| CSP | Isolated — no shared storage with other profiles. Separate cookie jar. |
| Navigation | Only to auth flow URLs. Post-auth redirect handled by shell, not by auth content. |

**`devtools` profile — full access, dev-only:**

| Access | Same as `app` profile, plus: |
|---|---|
| DevTools | WebView inspector, hot-reload, debug overlays, performance profiler. |
| Production | Entire profile stripped. `#[cfg(debug_assertions)]` gated. Devtools routes don't exist in release builds. |

### How profiles map to Tauri's security model

Tauri's security model operates primarily at build time (`tauri.conf.json`
capabilities, CSP configuration, plugin permissions). WebView profiles add a
**runtime mediation layer** on top:

```
Tauri build-time config (static)
  │
  │ defines: CSP base, plugin allowlists, capability permissions,
  │          protocol schemes, filesystem scope
  │
  ▼
Platform WebView profiles (runtime)
  │
  │ defines: per-route trust level, service access gates,
  │          dynamic capability allowlisting, origin policies
  │
  ▼
Session backbone (runtime)
  │
  │ enforces: route-scoped identity, stale-page guards,
  │           per-request permission checks
```

**What's build-time only (Tauri):**
- CSP base policy (profiles tighten from the base, never loosen).
- Plugin installation (which plugins are available at all).
- Filesystem scope paths.
- Custom protocol scheme registration.
- WebView creation attributes (data directory, incognito mode, autofill).

**What's runtime-configurable (platform):**
- Per-route profile assignment (the route handler picks the profile).
- Service access gates (which foundation crates a route can touch).
- Capability allowlisting per route.
- Origin restrictions per route.
- CSP tightening per route (adding restrictions, never removing them).
- Navigation policy (same-origin, allowed origins, external handling).

### How profiles integrate with platform services

Each platform service exposes a profile-aware access layer. The session
backbone checks the active profile before allowing service access:

```rust
// foundation_db: profile-gated database access
impl DatabaseHandle {
    pub fn query(&self, session: &PlatformSession, sql: &str) -> Result<Rows> {
        session.check_profile_access(Service::Database, Access::Read)?;
        // session routes are checked: only app + trustedRemote can query
        self.inner.query(sql)
    }
}

// foundation_auth: profile-gated auth APIs
impl AuthManager {
    pub fn get_token(&self, session: &PlatformSession) -> Result<AuthToken> {
        session.check_profile_access(Service::Auth, Access::Read)?;
        // trustedRemote can access auth-protected resources
        // but cannot read raw credentials — that's app profile only
        self.inner.get_scoped_token(&session.route_identity)
    }
}

// foundation_nativeapis: profile-gated platform capabilities
impl NativeAPI {
    pub fn invoke(&self, session: &PlatformSession, capability: &CapabilityId) -> Result<()> {
        session.check_profile_access(Service::NativeAPI, Access::Execute)?;
        session.check_capability_allowed(capability)?;
        // even within a profile, capabilities are per-route allowlisted
        self.inner.execute(capability)
    }
}
```

### Profile assignment

The route handler assigns profiles in the `RouteDecision`:

```rust
session.route("/app/*", RouteDecision::webview_app()
    .with_profile(Profile::App));

session.route("/remote/content/*", RouteDecision::remote_fetch()
    .with_profile(Profile::TrustedRemote)
    .with_allowed_capabilities(&[CapabilityId::camera]));

session.route("/remote/embed/*", RouteDecision::remote_fetch()
    .with_profile(Profile::UntrustedRemote));

session.route("/auth/*", RouteDecision::remote_fetch()
    .with_profile(Profile::Auth)
    .with_auth_origin("https://auth.myapp.com"));
```

If no profile is specified, the platform default applies: `TrustedRemote` for
remote routes, `App` for local routes. The user can override the default.

### Cross-profile isolation

- **Separate cookie jars** — `auth` profile has an isolated cookie store.
  `untrustedRemote` cannot read `app` cookies.
- **Separate storage** — each profile has its own LocalStorage/SessionStorage
  namespace in the WebView.
- **No shared DOM** — content from different profiles renders in separate
  WebView contexts or, within the same WebView, in origin-isolated frames.
- **Capability requests carry profile identity** — a capability request from
  `untrustedRemote` is denied even if the capability is globally enabled.
  The session checks both the global allowlist AND the per-route profile.

### Default profiles (if user doesn't configure)

| Route source | Default profile |
|---|---|
| Bundled WASM / local content | `app` |
| Remote, same origin as configured backend | `trustedRemote` |
| Remote, different origin | `untrustedRemote` |
| Auth path (`/auth/*` or configured) | `auth` |

Users can override every default. The defaults are safe — they err toward
less access.
