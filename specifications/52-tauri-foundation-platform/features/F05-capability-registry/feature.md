---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F05-capability-registry"
this_file: "specifications/52-tauri-foundation-platform/features/F05-capability-registry/feature.md"

status: pending
priority: high
created: 2026-07-17

depends_on:
  - "F01-session-backbone"
  - "F04-webview-profiles"

tasks:
  completed: 0
  uncompleted: 14
  total: 14
  completion_percentage: 0%
---

# F05 — Capability registry

## Overview

Implement `Capability` trait, `#[platform_capability]` proc macro, and
capability registry on `PlatformSession`. Capabilities are registered at
build time and invoked at runtime through the session backbone with
profile gating, per-route allowlisting, and stale-page guards.

[Decision 07](../decisions/07-native-capability-contract.md).

---

## Part A — `Capability` trait

```rust
// foundation_platform/src/capability.rs

pub trait Capability: Send + Sync + 'static {
    fn id(&self) -> &CapabilityId;
    fn min_profile(&self) -> Profile;
    fn execute(
        &self,
        session: &PlatformSession,
        action: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, CapabilityError>;
}
```

### A.2 — Registry

```rust
impl<R: Runtime> PlatformSession<R> {
    pub fn register_capability<C: Capability>(&self, capability: C) {
        self.capability_registry.write().unwrap()
            .insert(capability.id().0.clone(), Box::new(capability));
    }

    pub fn invoke_capability(&self, request: &CapabilityRequest) -> CapabilityResponse {
        // 1. Stale-page guard
        if !self.is_active_page(&request.page_identity) {
            return CapabilityResponse { id: request.id.clone(), page_identity: request.page_identity.clone(), status: Err("stale page".into()) };
        }
        // 2. Look up handler
        let registry = self.capability_registry.read().unwrap();
        let handler = match registry.get(&request.capability) {
            Some(h) => h,
            None => return CapabilityResponse { id: request.id.clone(), page_identity: request.page_identity.clone(), status: Err(format!("unknown: {}", request.capability)) },
        };
        // 3. Profile gate
        if let Err(e) = self.profile_gate().check(Service::NativeApi, Access::Execute) {
            return CapabilityResponse { id: request.id.clone(), page_identity: request.page_identity.clone(), status: Err(e.to_string()) };
        }
        // 4. Per-route allowlist gate
        let route = self.current_route_decision();
        if let Some(decision) = route {
            if !decision.capabilities.is_empty() && !decision.capabilities.contains(handler.id()) {
                return CapabilityResponse { id: request.id.clone(), page_identity: request.page_identity.clone(), status: Err("not allowed on this route".into()) };
            }
        }
        // 5. Execute
        CapabilityResponse { id: request.id.clone(), page_identity: request.page_identity.clone(), status: handler.execute(self, &request.action, request.payload.clone()) }
    }
}
```

### A.3 — `#[platform_capability]` proc macro

```rust
// foundation_macros — expands to impl Capability + registration code
#[platform_capability(permissions = ["camera"], profile = Profile::TrustedRemote)]
struct CameraCapability { ... }
```

### A.4 — 5-layer defense

1. Profile-level gate — is this profile allowed to use native APIs?
2. Capability registration gate — is the capability registered?
3. Per-route allowlist — is it in `RouteDecision.capabilities`?
4. OS permission gate — has the user granted the OS permission?
5. Stale-page guard — is the requesting page still active?

---

## Verification

```bash
cargo test --package foundation_platform -- capability
cargo test --package foundation_macros -- platform_capability
```
