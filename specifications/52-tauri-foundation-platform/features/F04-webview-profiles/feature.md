---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F04-webview-profiles"
this_file: "specifications/52-tauri-foundation-platform/features/F04-webview-profiles/feature.md"

status: pending
priority: high
created: 2026-07-17

depends_on:
  - "F00-crate-skeleton"

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# F04 — WebView profiles and access gates

## Overview

Implement five WebView profiles that gate access to platform services at
runtime. Profiles are assigned per-route in `RouteDecision.profile` and
enforced by the session backbone. Every platform service call checks the
active profile before executing.

[Decision 06](../decisions/06-webview-profiles.md) defines the profile
taxonomy and access gates.

## Dependencies

Depends on:
- `F00-crate-skeleton` — Uses `Profile` enum from `foundation_ui_traits`

Required by:
- `F05-capability-registry` — Capabilities are profile-gated
- `F08-walking-skeleton` — Profiles enforced at runtime

## Requirements

### 1. Profile taxonomy

Five profiles from [decision 06](../decisions/06-webview-profiles.md):

```rust
enum Profile {
    App,              // Bundled local WASM — full platform access
    TrustedRemote,    // App's own backend, authenticated — scoped access
    UntrustedRemote,  // Third-party content — sandboxed, no platform access
    Auth,             // Login screens, OAuth flows — elevated isolation
    Devtools,         // Debug panels — full access, stripped in production
}
```

### 2. Access gate enforcement

Every platform service checks the profile before executing:

```rust
// foundation_platform/src/profiles.rs

pub struct ProfileGate {
    current_profile: Profile,
}

impl ProfileGate {
    pub fn check(&self, service: Service, access: Access) -> Result<(), ProfileError> {
        match (self.current_profile, service, access) {
            // App: everything allowed
            (Profile::App, _, _) => Ok(()),

            // TrustedRemote: read-only DB, allowed capabilities, allowed origins
            (Profile::TrustedRemote, Service::Database, Access::Read) => Ok(()),
            (Profile::TrustedRemote, Service::Database, Access::Write) => {
                Err(ProfileError::AccessDenied)
            }
            (Profile::TrustedRemote, Service::NativeAPI, _) => {
                // Per-route capability allowlist checked separately
                Ok(())
            }

            // UntrustedRemote: nothing except same-origin fetch
            (Profile::UntrustedRemote, Service::Http, Access::Read) => {
                // Same-origin only — enforced at call site
                Ok(())
            }
            (Profile::UntrustedRemote, _, _) => Err(ProfileError::AccessDenied),

            // Auth: login/logout/refresh only
            (Profile::Auth, Service::Auth, _) => Ok(()),
            (Profile::Auth, Service::NativeAPI, Access::Biometric) => Ok(()),
            (Profile::Auth, _, _) => Err(ProfileError::AccessDenied),

            // Devtools: everything, debug only
            (Profile::Devtools, _, _) => {
                #[cfg(debug_assertions)] { Ok(()) }
                #[cfg(not(debug_assertions))] { Err(ProfileError::StrippedInProduction) }
            }
        }
    }
}
```

### 3. Profile assignment

Route handlers assign profiles:

```rust
session.route("/app/*", RouteDecision::webview_app()
    .with_profile(Profile::App));

session.route("/remote/content/*", RouteDecision::remote_fetch()
    .with_profile(Profile::TrustedRemote)
    .with_allowed_capabilities(&[CapabilityId::camera]));

session.route("/remote/embed/*", RouteDecision::remote_fetch()
    .with_profile(Profile::UntrustedRemote));
```

### 4. Default profile assignment

| Route source | Default profile |
|---|---|
| Bundled WASM / local content | `App` |
| Remote, same origin as configured backend | `TrustedRemote` |
| Remote, different origin | `UntrustedRemote` |
| Auth path (`/auth/*` or configured) | `Auth` |

### 5. Cross-profile isolation

- Separate cookie jars per profile
- Separate LocalStorage/SessionStorage per profile
- Capability requests carry profile identity
- Content from different profiles renders in separate WebView contexts

## Tasks

### Profile gate
- [ ] Implement `ProfileGate` struct with `check()` method
- [ ] Define `Service` enum (Database, Auth, NativeAPI, Http, Arrow, Signals)
- [ ] Define `Access` enum (Read, Write, Execute)
- [ ] Implement full access matrix for all 5 profiles
- [ ] Test: App profile → all services allowed
- [ ] Test: UntrustedRemote → all services denied
- [ ] Test: TrustedRemote → DB read allowed, DB write denied

### Profile assignment
- [ ] Add `with_profile()` builder to `RouteDecision`
- [ ] Implement default profile assignment by RouteSource
- [ ] Test: route without explicit profile gets correct default

### Cross-profile isolation
- [ ] Implement separate cookie jars per profile (via Tauri WebView contexts)
- [ ] Tag capability requests with profile identity
- [ ] Profile-gate cache lookups (untrustedRemote can't read app cache)

### Production stripping
- [ ] Gate `Devtools` profile behind `#[cfg(debug_assertions)]`
- [ ] Test: devtools routes are inaccessible in release builds

## Verification Commands

```bash
cargo test --package foundation_platform -- profiles
```
