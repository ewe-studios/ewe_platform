# 19 — Remote UI security: capability mediation layer + red team

**Date:** 2026-07-04
**Status:** Resolved

### Decision

`foundation_platform` is a capability mediation layer on top of Tauri's build-time
security model. Whatever Tauri locks at build time is free game at runtime — the
platform designs how the backend (HTTP server, WebSocket endpoint, IPC process)
can instruct the app via communication payloads to configure behavior.

Local compiled policy is the final authority. The backend can request, suggest,
or reduce privileges — never grant new ones that local policy denies. This is
explicitly NOT a Tauri wrapper. It is a runtime mediation layer that Tauri does
not provide.

### Threat model: what are we defending against?

| Attacker | Capability | Goal |
|---|---|---|
| Compromised remote server | Can send arbitrary HTML/JS/DomOps to the WebView | Steal auth tokens, invoke native capabilities, exfiltrate local data, navigate user to phishing page |
| Malicious third-party content | Embedded in an iframe or loaded via user-generated content | Escape sandbox, read app cookies, access platform services |
| Man-in-the-middle | Intercepts remote server communication | Inject malicious payloads, downgrade security |
| Compromised WebView render process | RCE in the WebView's process (browser exploit) | Escape to native process, access filesystem, invoke capabilities |
| Malicious native plugin/bridge | Malicious Swift/Kotlin code in a native bridge component | Access other capabilities, exfiltrate data, escalate privileges |
| Physical device access | Attacker has the unlocked device | Access cached tokens, local database, offline content |

### Defense layers

**Layer 1: Transport security (defends against MITM)**

| Defense | Mechanism |
|---|---|
| TLS for all remote connections | Enforced by the shell. The `trustedRemote` profile only allows HTTPS. Certificate pinning for the app's own backend. |
| `ewe://` protocol isolation | `ewe://` responses are served by the platform shell, not by remote servers. Remote content cannot intercept or modify `ewe://` traffic. |
| Binary integrity | WASM modules, frontend assets, and cached content are validated by hash before loading. Corrupted or tampered content is discarded. |
| CSP enforcement | Content-Security-Policy headers are applied per WebView profile. `connect-src` restricts which origins the WebView can contact. |

**Layer 2: Profile sandboxing (defends against compromised remote server)**

| Defense | Mechanism |
|---|---|
| WebView profile assignment | Every route runs in a profile (`app`, `trustedRemote`, `untrustedRemote`, `auth`). The profile gates access to ALL platform services. |
| `untrustedRemote` sandbox | No Tauri commands. No `ewe://` access. No `foundation_db`. No `foundation_auth`. No native capabilities. Same-origin fetch only. Separate cookie jar. `sandbox` attribute on iframes. |
| `trustedRemote` allowlisting | Capabilities are per-route allowlisted. A video call route can request camera; a content route cannot. Server-suggested capability grants are DENIED if local policy doesn't allow them. |
| `auth` profile isolation | Separate cookie jar. No shared storage with `app` profile. Auth tokens managed by shell, never exposed to the WebView JS context. Post-auth redirect handled by shell, not by auth content. |

**Layer 3: Capability gating (defends against unauthorized native access)**

| Defense | Mechanism |
|---|---|
| Build-time registration | Capabilities are registered at build time. A capability not in the registry cannot be invoked, even if the server requests it. |
| Per-route allowlisting | `RouteDecision.capabilities` lists allowed capabilities per route. A capability request from an unlisted route is denied. |
| Profile gating | A capability declares a minimum profile. `Camera` requires `trustedRemote`. `untrustedRemote` cannot invoke it, even if allowlisted. |
| Stale-page guard | `CapabilityRequest` carries `PageIdentity { session_id, route, visit_id }`. The session checks that the requesting page is still active before delivering the response. A navigated-away page cannot receive capability results. |
| OS permission mediation | Even if the platform allows a capability, the OS may deny it (camera permission not granted, biometrics not enrolled). The platform forwards OS denials as capability errors. |

**Layer 4: Data isolation (defends against data exfiltration)**

| Defense | Mechanism |
|---|---|
| Token isolation | Auth tokens live in Tauri's secure storage (iOS Keychain, Android Keystore). The shell attaches them to requests. Tokens NEVER enter the WebView's JavaScript context. The WebView receives scoped results, not raw tokens. |
| Database access gating | `foundation_db` access is profile-gated. `untrustedRemote` has zero DB access. `trustedRemote` has read-only scoped queries. Only `app` has full read/write. |
| Cross-profile storage isolation | Each profile has separate LocalStorage/SessionStorage. `auth` cookies don't leak to `trustedRemote`. `untrustedRemote` cannot read `app` cookies. |
| Cache scoping | Cached content is scoped by route and profile. An `untrustedRemote` page cannot read cached content from a `trustedRemote` route. |
| Clipboard isolation | Clipboard reads require a capability request. The capability is profile-gated and per-route allowlisted. An `untrustedRemote` page cannot read the clipboard silently. |

**Layer 5: Navigation security (defends against phishing/redirect attacks)**

| Defense | Mechanism |
|---|---|
| Session-owned navigation | Every link click, form submit, and redirect flows through the session backbone. The route handler decides: allow, deny, redirect, open externally. |
| External navigation policy | `untrustedRemote` content opening an external link opens in the system browser, not in the app's WebView. No `target=_blank` escape. |
| Cross-origin redirect detection | The shell detects cross-origin redirects (like Hotwire Native's `resolveRedirect`). Server-side redirects to unexpected origins are intercepted and routed through the session's navigation policy. |
| Deep link validation | Deep links (custom URL schemes) are validated by the shell before routing. Only registered URL schemes are handled. Unknown schemes are ignored. |

**Layer 6: Runtime integrity (defends against WebView compromise)**

| Defense | Mechanism |
|---|---|
| WebView process isolation | On platforms that support it (macOS, iOS), the WebView runs in a separate process. A WebView compromise does not grant native process access. |
| WebView render process termination | Tauri's `on_web_content_process_terminate` callback notifies the shell. The shell recreates the WebView and restores state from the session. |
| No raw memory sharing | The WebView never receives raw native memory pointers. Binary data is delivered as `ArrayBuffer` through the custom protocol — a copy, not shared memory. |
| CSP `script-src` restrictions | `untrustedRemote`: no inline scripts, no `unsafe-eval`. `trustedRemote`: only app origin + trusted CDN. Only `app` profile allows inline scripts (for WASM bootstrap). |

### Server-instructed configuration

The backend can send instructions to the app via communication payloads. These
are suggestions, not commands — local policy always has veto power.

| What the server can suggest | Constraint |
|---|---|
| Cache policy per route | Local policy can override. Server cannot disable caching for a route the local policy requires to be cacheable. |
| Route presentation hints | Server can suggest "this page should open as a modal." Shell validates against local policy. |
| New component definitions | Server sends a new component. Shell verifies it's from a trusted origin. Renders through `mount-data`/`mount-stream`. |
| Capability requests | Server can request a capability on behalf of its content. Shell checks: profile allows it? Route allowlists it? OS permission granted? |
| Navigation suggestions | Server can suggest a redirect or a new route. Session's route handler chain makes the final decision. |
| CSP additions | Server can ADD restrictions (tighten CSP). Server can NEVER remove restrictions. |
| Profile downgrade | Server can request a LOWER profile (trustedRemote → downgrade to untrustedRemote for specific content). Server can NEVER request a higher profile. |

### Red team findings (pre-implementation threat analysis)

**Attack 1: Server sends HTML that attempts to invoke `tauri::invoke()` directly.**

Mitigation: `untrustedRemote` profile blocks all Tauri commands. `trustedRemote`
has an allowlisted subset. The `invoke()` function is wrapped by the shell; it
checks the active profile before forwarding to Tauri. A raw `invoke()` call from
an untrusted page returns an error.

**Attack 2: Server sends a capability request with a forged `PageIdentity`.**

Mitigation: `PageIdentity` carries `session_id` and `visit_id` generated by the
shell, not by the WebView. The WebView cannot forge them. The session validates
the identity against its internal state before delivering the response.

**Attack 3: Server's HTML navigates the user to a phishing page via `window.location`.**

Mitigation: Navigation is intercepted by the session backbone. Raw
`window.location` changes in `trustedRemote` go through the route handler.
External origins open in the system browser. `untrustedRemote` cannot navigate
at all — same-origin only.

**Attack 4: Server sends a `mount-data` payload that injects a script tag.**

Mitigation: `mount-data` and `mount-stream` deliver content through the runtime's
DOM applicator, not through `innerHTML`. The runtime parses the content and
applies it via `MorphDom`/DomOps. Script injection through data payloads is
blocked at the DOM operation level.

**Attack 5: Compromised native bridge (malicious Swift/Kotlin) accesses other capabilities.**

Mitigation: The capability registry scopes each native bridge to its declared
capability. A `Camera` bridge cannot invoke `BiometricAuth`. The session routes
capability requests by name — a bridge for one capability cannot intercept
requests for another.

**Attack 6: Cached content from a previous session contains stale auth tokens.**

Mitigation: Cached content is stored WITHOUT auth tokens. Tokens are stripped
before caching. When cached content is served, the shell re-attaches fresh
tokens (if the session is still valid). Stale tokens in cached content are
never sent to the server.

**Attack 7: Man-in-the-middle downgrades TLS or injects malicious WASM.**

Mitigation: TLS is enforced for all `trustedRemote` connections. WASM modules
are integrity-checked by hash before instantiation. The update service serves
signed manifests. The shell validates the signature before hot-swapping WASM.
Certificate pinning for the app's backend prevents MITM even with a compromised CA.

**Attack 8: Physical device access — attacker reads the local SQLite database.**

Mitigation: `foundation_db` supports encryption at rest (SQLCipher via Turso/
libsql). The platform enables encryption by default for cached content and
local databases. The encryption key is stored in the platform's secure storage
(iOS Keychain, Android Keystore), which requires biometric auth to access.

### What we do NOT protect against (out of scope)

| Threat | Why out of scope |
|---|---|
| OS-level compromise (jailbreak/root) | If the attacker has kernel access, they own the device. No app-level defense is possible. |
| Physical device access + biometric coercion | If the attacker can force the user to unlock the device and authenticate, they can access local data. This is a physical security problem, not a software one. |
| Supply chain attack on Tauri itself | We trust Tauri's release artifacts. This is addressed by Tauri's own security practices and our dependency auditing. |
| Side-channel attacks (timing, power analysis) | Out of scope for an application platform. |
