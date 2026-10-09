---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F04-webview-profiles"
this_file: "specifications/52-tauri-foundation-platform/features/F04-webview-profiles/feature.md"

status: completed
priority: high
created: 2026-07-17
updated: 2026-07-21

depends_on:
  - "F00-crate-skeleton"

tasks:
  completed: 4
  uncompleted: 0
  total: 4
  completion_percentage: 100%
---

# F04 — WebView profiles and access gates

## Overview

Implement runtime enforcement of five WebView profiles that gate access to
platform services. Profiles are assigned per-route in `RouteDecision.profile`
and enforced by `ProfileGate::check(service, access)`. Every platform service
call checks the active profile before executing.

[Decision 06](../decisions/06-webview-profiles.md) defines the full profile
taxonomy and per-service access matrix.

---

## Part A — ProfileGate

### A.1 — Service and Access enums

```rust
// foundation_platform/src/profiles.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Service {
    Database,
    Auth,
    NativeApi,
    Http,
    Arrow,
    Signals,
    TauriCommand,
    TauriEvent,
    CustomProtocol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    Execute,
}
```

### A.2 — ProfileGate::check()

```rust
pub struct ProfileGate {
    profile: Profile,
}

impl ProfileGate {
    pub fn new(profile: Profile) -> Self {
        Self { profile }
    }

    pub fn check(&self, service: Service, access: Access) -> Result<(), ProfileError> {
        use Access::*;
        use Profile::*;
        use Service::*;

        match self.profile {
            App => Ok(()), // everything allowed

            TrustedRemote => match (service, access) {
                (Database, Read) => Ok(()),
                (Database, Write) => Err(ProfileError::AccessDenied),
                (Auth, Read) => Ok(()),
                (NativeApi, Execute) => Ok(()), // per-route allowlist checked separately
                (Http, Read) => Ok(()), // allowed origins checked at call site
                (Arrow, Read) => Ok(()),
                (Signals, Read) => Ok(()),
                (TauriCommand, Execute) => Ok(()), // allowlisted subset
                _ => Err(ProfileError::AccessDenied),
            },

            UntrustedRemote => match (service, access) {
                (Http, Read) => Ok(()), // same-origin only, enforced at call site
                _ => Err(ProfileError::AccessDenied),
            },

            Auth => match (service, access) {
                (Auth, _) => Ok(()),
                (NativeApi, Execute) => Ok(()), // biometric only
                (Http, Read) => Ok(()), // auth provider origin only
                _ => Err(ProfileError::AccessDenied),
            },

            Devtools => {
                #[cfg(debug_assertions)] { Ok(()) }
                #[cfg(not(debug_assertions))] { Err(ProfileError::StrippedInProduction) }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("access denied for profile")]
    AccessDenied,
    #[error("devtools profile stripped in production")]
    StrippedInProduction,
}
```

### A.3 — Integration with platform services

```rust
impl DatabaseHandle {
    pub fn query(&self, session: &PlatformSession, sql: &str) -> Result<Rows> {
        session.profile_gate().check(Service::Database, Access::Read)?;
        self.inner.query(sql)
    }
}

impl AuthManager {
    pub fn get_token(&self, session: &PlatformSession) -> Result<AuthToken> {
        session.profile_gate().check(Service::Auth, Access::Read)?;
        self.inner.get_scoped_token(&session.route_identity())
    }
}
```

---

## Part B — Profile assignment

### B.1 — Default profile by RouteSource

```rust
impl RouteSource {
    pub fn default_profile(&self) -> Profile {
        match self {
            RouteSource::WebviewApp => Profile::App,
            RouteSource::IpcShell => Profile::TrustedRemote,
            RouteSource::RemoteServer => Profile::TrustedRemote,
        }
    }
}
```

### B.2 — `RouteDecision` with profile

```rust
impl PlatformSession<R> {
    /// Get the active profile for the current route.
    /// If the route decision has an explicit profile, use it.
    /// Otherwise, fall back to the default for the route source.
    pub fn active_profile(&self) -> Profile {
        self.current_route_decision()
            .map(|d| d.profile)
            .unwrap_or(Profile::UntrustedRemote) // safest default
    }

    pub fn profile_gate(&self) -> ProfileGate {
        ProfileGate::new(self.active_profile())
    }
}
```

---

## Verification

```bash
cargo test --package foundation_platform -- profiles
```

Tests:
- App profile → all services allowed
- UntrustedRemote → all services denied except same-origin HTTP
- TrustedRemote → DB read allowed, DB write denied
- Auth → only Auth + biometric NativeApi allowed
- Devtools → full access in debug, stripped in release
