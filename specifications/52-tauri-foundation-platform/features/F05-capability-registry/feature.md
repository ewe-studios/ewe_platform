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
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---

# F05 — Capability registry

## Overview

Implement the typed, permissioned, route-scoped capability registry.
Capabilities are registered at build time, invoked through the session
backbone at runtime, and gated by WebView profiles.

[Decision 07](../decisions/07-native-capability-contract.md) defines the
`Capability` trait, `#[platform_capability]` proc macro, and registry.

## Dependencies

Depends on:
- `F01-session-backbone` — Registry lives on the session
- `F04-webview-profiles` — Capabilities are profile-gated

Required by:
- `F08-walking-skeleton` — Capability calls in end-to-end tests

## Requirements

### 1. `Capability` trait

```rust
// foundation_platform/src/capability.rs

pub trait Capability: Send + Sync + 'static {
    /// The capability's unique identifier (e.g., "camera", "biometric_auth")
    fn id(&self) -> CapabilityId;

    /// The minimum profile required to invoke this capability
    fn min_profile(&self) -> Profile;

    /// Execute the capability with the given action and payload
    fn execute(
        &self,
        session: &PlatformSession,
        action: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, CapabilityError>;
}
```

### 2. `#[platform_capability]` proc macro

Sugar for implementing the trait:

```rust
#[platform_capability(
    permissions = ["camera", "microphone"],
    profile = Profile::TrustedRemote
)]
struct MediaCapture {
    #[native(ios = "MediaCaptureIOS", android = "MediaCaptureAndroid")]
    native_impl: NativeBinding,
}
```

Expands to:
- `impl Capability for MediaCapture`
- Registration code for the capability registry

### 3. Capability registry on PlatformSession

```rust
impl<R: Runtime> PlatformSession<R> {
    /// Register a capability handler
    pub fn register_capability<C: Capability>(&self, capability: C) {
        self.capability_registry.insert(capability.id(), Box::new(capability));
    }

    /// Invoke a capability — checks profile, route allowlist, OS permissions
    pub fn invoke_capability(
        &self,
        request: &CapabilityRequest,
    ) -> CapabilityResponse {
        // 1. Profile-level gate
        let profile = self.active_profile();
        let capability = self.capability_registry.get(&request.capability)?;
        if profile < capability.min_profile() {
            return CapabilityResponse::denied("profile too low");
        }

        // 2. Per-route allowlist gate
        let route_decision = self.current_route_decision();
        if !route_decision.capabilities.contains(&capability.id()) {
            return CapabilityResponse::denied("not allowed on this route");
        }

        // 3. Execute
        capability.execute(self, &request.action, request.payload.clone())
    }
}
```

### 4. Capability request/response flow

```
WebView capability request
  → CapabilityRequest { id, page_identity, capability, action, payload }
    → Session backbone routes to registered handler
      → Handler executes (pure Rust, Tauri plugin, or native bridge)
        → CapabilityResponse { id, page_identity, status, payload_or_error }
          → Session delivers to the WebView scoped to the requesting page
            → foundation_wasm_ui runtime receives as structured event/signal
              → UI updates
```

### 5. Safety guards

- **Stale-page guard:** `CapabilityRequest` carries `PageIdentity`. Session
  verifies the requesting page is still active before delivering response.
- **OS permission mediation:** Even if platform allows, OS may deny.
- **Per-route allowlisting:** `RouteDecision.capabilities` lists allowed caps.

## Tasks

### Capability trait
- [ ] Define `Capability` trait with `id()`, `min_profile()`, `execute()`
- [ ] Define `CapabilityId` newtype
- [ ] Define `CapabilityError` type

### Proc macro
- [ ] Implement `#[platform_capability]` proc macro in foundation_macros
- [ ] Generate `impl Capability` from annotated struct
- [ ] Support `#[native(ios = "...", android = "...")]` for native bridges
- [ ] Support `#[mock(impl = "...")]` for test mocks

### Registry
- [ ] Add `capability_registry: HashMap<CapabilityId, Box<dyn Capability>>` to session
- [ ] Implement `register_capability()` and `invoke_capability()`
- [ ] Implement profile-level gate check
- [ ] Implement per-route allowlist gate check
- [ ] Implement stale-page guard

### Wire format
- [ ] Capability requests use `CapabilityRequest`/`CapabilityResponse` from foundation_ui_traits
- [ ] Responses scoped to requesting page identity
- [ ] Test: request from navigated-away page is dropped

### Native bridge stub (post-MVP surface)
- [ ] Define `NativeBinding` type for platform-specific implementations
- [ ] Document Swift/Kotlin bridge interface (implementation post-MVP)

## Verification Commands

```bash
cargo test --package foundation_platform -- capability
cargo test --package foundation_macros -- platform_capability
```
