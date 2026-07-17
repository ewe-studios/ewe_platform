---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F09-walking-skeleton"
this_file: "specifications/52-tauri-foundation-platform/features/F09-walking-skeleton/feature.md"

status: pending
priority: critical
created: 2026-07-17

depends_on:
  - "F01-session-backbone"
  - "F02-route-handler"
  - "F03-ewe-protocol"
  - "F06-webview-stack"
  - "F07-cache-tiers"

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# F09 — Walking skeleton

## Overview

Wire everything together: boot the platform, create a WebView, register routes,
handle a navigation, render content, and verify the full flow works end-to-end.
This is the "it all works" feature.

The walking skeleton proves the execution contract from [decision
02](../decisions/02-route-policy-model.md#execution-contract-what-happens-after-a-decision)
works — every step in the 9-step chain is exercised.

## Dependencies

Depends on: all preceding features (F00 through F08).

Required by: Nothing — this is the integration gate.

## Requirements

### 1. Bootstrap flow

```
App launch
  → Tauri boots
  → setup() hook fires
    → PlatformSession::initialize()
    → Transport lanes pre-wired
    → Capability registry populated
    → Cache database opened
    → Route handlers registered
    → WebView created, foundation-wasm-ui.js injected
    → User's #[platform_bin] main(session) called
    → Rendering loop begins
```

### 2. End-to-end trace: link click

```
1. User clicks <a href="/remote/dashboard"> in WebView
2. Tauri's on_navigation() fires
   → Session constructs NavigationIntent
3. Route handler chain runs → returns RouteDecision
4. Cache check: NetworkFirst → skip cache, go to backend
5. Presentation: Push → stack manager pushes new screen
6. Source: RemoteServer → session opens HTTP connection
7. Backend responds with content bytes
8. Protocol: selected, content encoded with Content-Type
9. Transport: response delivered through ewe:// custom protocol
10. Render: foundation-wasm-ui runtime receives bytes, renders screen
11. Post-render: navigation recorded, screenshot captured, event emitted
```

### 3. Implementation

```rust
#[cfg(test)]
mod walking_skeleton_tests {
    use foundation_platform::*;

    #[platform_test]  // in-process Tauri WebView
    async fn link_click_navigates_to_remote_route() -> Result<()> {
        // 1. Set up platform with a route handler
        let session = PlatformSession::initialize(app_handle);

        session.route("/remote/*", RouteDecision::remote_fetch()
            .with_presentation(Presentation::Push)
            .with_cache_policy(CachePolicy::NetworkFirst));

        // 2. Simulate a link click navigation
        let intent = NavigationIntent {
            url: "ewe://localhost/remote/dashboard".parse()?,
            method: Method::Get,
            source: IntentSource::LinkClick,
            referrer: None,
        };

        // 3. Resolve route
        let decision = session.resolve_route(&intent);

        // 4. Verify decision
        assert_eq!(decision.source, RouteSource::RemoteServer);
        assert_eq!(decision.presentation, Presentation::Push);

        // 5. Execute decision → cache check, backend query, protocol, transport, render
        session.execute_decision(&decision).await?;

        // 6. Verify navigation recorded
        assert_eq!(session.active_page().unwrap().route, "/remote/dashboard");

        Ok(())
    }
}
```

### 4. Verify the full execution contract

Tests for the 9-step chain from [decision 02](../decisions/02-route-policy-model.md):

```rust
// Step 1: Navigation interception — intent constructed from URL
#[platform_test]
async fn intercepts_link_click() { ... }

// Step 2: Route handler chain — first match wins
#[platform_test]
async fn handler_chain_first_match_wins() { ... }

// Step 3: Cache check — CacheFirst serves cached, NetworkFirst skips
#[platform_test]
async fn cache_check_respects_policy() { ... }

// Step 4: Presentation — Push creates new slot, screenshot captured
#[platform_test]
async fn push_presentation_creates_new_slot() { ... }

// Step 5: Backend query — source resolution
#[platform_test]
async fn remote_server_source_fetches_over_http() { ... }

// Step 6: Protocol selection — hint priority chain
#[platform_test]
async fn protocol_selection_honors_hint() { ... }

// Step 7: Content encoding — correct Content-Type
#[platform_test]
async fn columnar_content_gets_correct_content_type() { ... }

// Step 8: View instantiation — WebView created/delivered
#[platform_test]
async fn webview_renders_content() { ... }

// Step 9: Post-render — navigation recorded, screenshot captured
#[platform_test]
async fn post_render_records_navigation() { ... }
```

### 5. Minimal demo app

A working Tauri app with:
- `#[platform_bin]` entrypoint
- 3 routes: `/app/*` (WebviewApp), `/remote/*` (RemoteServer), `/cached/*` (CacheFirst)
- Navigation between screens
- Back button returns to previous screen with screenshot
- Offline: cached routes still render

## Tasks

### Walking skeleton tests
- [ ] Test: platform boots and session is initialized
- [ ] Test: link click intercepted, NavigationIntent constructed
- [ ] Test: route handler chain resolves, first match wins
- [ ] Test: cache policy respected (CacheFirst serves, NetworkFirst skips)
- [ ] Test: source resolution (RemoteServer opens transport)
- [ ] Test: protocol selection and content encoding
- [ ] Test: full 9-step chain end-to-end

### Demo app
- [ ] Create minimal Tauri app using PlatformBuilder
- [ ] Register 3 routes with different sources
- [ ] Verify navigation between screens
- [ ] Verify screenshot-based back navigation
- [ ] Verify offline: cached routes serve, online-only routes error

## Verification Commands

```bash
cargo test --package foundation_platform -- walking_skeleton
cargo run --example platform_demo
```
