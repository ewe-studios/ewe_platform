# Adding Native Capabilities — foundation_platform

The platform crate provides the **session-level integration** for native capabilities — routing, IPC dispatch, and the platform builder API.

## Registration lifecycle

```
1. App starts → PlatformBuilder.setup(setup_routes)
2. setup_routes(session) runs:
   ├─ Register routes (URL → responder)
   ├─ Register IPC handlers (name → Ipc)
   ├─ Register capabilities (profile-gated)
   └─ Register native modules (foundation_platform_native)
3. PlatformSession dispatches:
   ├─ Navigation → route engine → responder
   ├─ IPC calls → IpcRegistry → handler
   └─ Capabilities → CapabilityRegistry → profile check → handler
```

## PlatformBuilder integration

When adding a native plugin to the platform:

```rust
PlatformBuilder::new()
    .inject_platform_runtimes()
    .plugin(foundation_platform_native::native::plugin::plugin())  // ← Tauri plugin
    .setup(|session| {
        // Register native IPC handlers on the session
        foundation_platform_native::native::modal::register(Arc::clone(&session));
        foundation_platform_native::native::dialog::register(Arc::clone(&session));
    })
```

The `.plugin()` call registers a Tauri mobile plugin (Android/iOS). The `.setup()` closure registers IPC handlers on the session.

## WindowManager + WebViewStack

Native capabilities that create windows (like modals) interact with these session components:

```rust
// WindowManager — creates/destroys Tauri windows
session.window_manager().ensure(&mut pool, &label);
session.window_manager().destroy(&mut pool, &label);

// WebViewStack — navigation state
session.webview_stack().push_slot(slot);
session.webview_stack().pop_with(|label, slot| { /* cleanup */ });
```

See [`foundation_platform_native/docs/adding_native_capabilities/`](../../foundation_platform_native/docs/adding_native_capabilities/) for the end-to-end guide to implementing a new capability module.
