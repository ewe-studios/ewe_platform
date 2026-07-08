# 18 — Native capability contract: Rust API for declaring and registering capabilities

**Date:** 2026-07-04
**Status:** Resolved

### Decision

Capabilities are `foundation_wasm_ui` primitives, not Basecamp-style HTML-injected
bridge components. Rendering is owned by the runtime. Capabilities are declared
in Rust, registered with the platform's capability registry, and invoked through
the session backbone. The wire format is `CapabilityRequest`/`CapabilityResponse`
(decision 01), living in `foundation_ui_traits`.

Three API surfaces compose, same pattern as route handlers:

**A — `Capability` trait (core abstraction). Similar to `WGPU`/`WASI` — the platform
provides the trait, the user implements it, the shell orchestrates.**

**B — `#[platform_capability]` proc macro (sugar for A).**

**C — Capability registry builder (declarative registration).**

### Native bridges: where Tauri falls short

When Tauri's plugin system is insufficient or non-performant for a specific
capability, the platform supports native Swift/Kotlin bridge code. This is
Hotwire Bridge Components repurposed: not HTML-injected, but Rust-declared,
with a native implementation behind the platform's capability contract.

A `NativeBridge` capability wraps platform-specific code behind the same
`Capability` trait:

```rust
#[platform_capability]
struct BiometricAuth {
    #[native(ios = "BiometricAuthIOS", android = "BiometricAuthAndroid")]
    native_impl: NativeBinding,
}

// On iOS: the shell calls into a Swift class via UniFFI or raw FFI.
// On Android: the shell calls into a Kotlin class via JNI.
// On desktop: falls back to a Tauri plugin or a pure-Rust implementation.
// The capability contract is the same on all platforms.
```

### How capability results re-enter the rendering loop

Results are delivered through the session backbone, not through ad-hoc callbacks:

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

### Testing without devices

Pure-Rust capabilities are tested with standard `#[test]` functions. Native
bridge capabilities use Tauri's Android/web test coverage and
`foundation_browser`'s CDP/BiDi driver. iOS strategy: simulators or
`foundation_testbed`'s VM infrastructure. A `#[platform_capability]` can
declare a mock implementation for testing:

```rust
#[platform_capability]
#[mock(impl = "MockCamera")]
struct Camera { ... }
```

### Capability categories

| Category | Examples | Implementation |
|---|---|---|
| **Filesystem** | File picker, document browser, save dialog | Tauri plugin (build-time). Wrapped as capability. |
| **Media** | Camera, microphone, photo library | Native bridge (Swift/Kotlin). Platform-specific hardware. |
| **Biometrics** | Face ID, Touch ID, fingerprint | Native bridge + Tauri plugin where available. |
| **Notifications** | Local notifications, push registration | Tauri plugin. |
| **Clipboard** | Read/write | Tauri plugin. |
| **Share sheet** | Native share dialog | Native bridge. |
| **Secure storage** | Keychain, Keystore | Tauri plugin + native bridge for advanced cases. |
| **Deep links** | URL scheme handling | Platform shell owns; routes through session backbone. |
| **Background tasks** | Periodic fetch, sync | `#[platform_worker]` + OS-specific APIs. |
| **App controls** | Window management, menus, dock icon | Tauri primitives (no capability abstraction needed). |
| **Platform sensors** | Accelerometer, gyroscope, GPS | Tauri plugin or native bridge. |

### Permission model

Capabilities declare required permissions. The session backbone enforces them:

```rust
#[platform_capability(
    permissions = ["camera", "microphone"],
    profile = Profile::TrustedRemote  // minimum profile required
)]
struct MediaCapture { ... }
```

At runtime, the session checks:
1. Is this capability registered? (build-time)
2. Does the requesting page's profile allow it? (runtime, per-route)
3. Has the user granted OS-level permission? (platform-mediated)
4. Is the request scoped to the correct page? (stale-page guard)

### Why three APIs (same pattern as route handlers)

Simple capabilities (clipboard, share sheet): use the proc macro or builder.
Complex capabilities with native bridges: implement the trait directly.
All three register into the same capability registry behind the same
`Capability` trait.
