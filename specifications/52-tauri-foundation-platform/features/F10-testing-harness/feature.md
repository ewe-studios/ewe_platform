---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F10-testing-harness"
this_file: "specifications/52-tauri-foundation-platform/features/F10-testing-harness/feature.md"

status: pending
priority: high
created: 2026-07-17

depends_on:
  - "F01-session-backbone"

tasks:
  completed: 0
  uncompleted: 5
  total: 5
  completion_percentage: 0%
---

# F10 — Testing harness

## Overview

Create the `#[platform_test]` proc macro for testing platform components.
Extends the existing `foundation_browser` test harness pattern to Tauri
WebView (in-process), browser (CDP/BiDi), and mobile targets (Android/iOS).

[Decision 09](../decisions/09-testing-strategy.md) defines the testing
strategy and coverage targets.

## Dependencies

Depends on:
- `F01-session-backbone` — Tests exercise the session backbone

## Requirements

### 1. `#[platform_test]` proc macro

```rust
// foundation_platform testing
#[platform_test]           // runs in Tauri WebView (in-process, like a unit test)
#[platform_test(browser)]  // runs in Chromium via CDP (like foundation_browser)
#[platform_test(android)]  // runs in Android emulator via ADB
#[platform_test(ios)]      // runs in iOS simulator via simctl

async fn my_test(session: PlatformTestSession) -> Result<()> {
    let page = session.page();
    page.click("#my-button").await?;
    let result = page.locator(".output").text().await?;
    assert_eq!(result, "button clicked");
    Ok(())
}
```

### 2. `PlatformTestSession`

```rust
pub struct PlatformTestSession {
    /// WebView access for DOM interaction
    page: TestPage,

    /// Route handler hooks
    router: TestRouter,

    /// Capability mocking
    capabilities: TestCapabilities,

    /// Cache inspection
    cache: TestCache,

    /// Event capture
    events: TestEventCapture,
}
```

### 3. In-process WebView tests

Tests run inside a Tauri WebView — fast, no browser binary needed. Used for:
- Route handler chain execution
- Protocol encoding/decoding
- Cache store/retrieve/invalidate
- Capability registry and permission checks
- Profile enforcement
- Session lifecycle
- Stale-page guard logic
- Custom protocol response pipeline
- Mutation queue replay

### 4. Test coverage targets

| Component | Coverage target |
|---|---|
| Route handler chain | 100% branch |
| Capability registry | 100% branch |
| Protocol encoding | 100% output comparison |
| Cache store/retrieve | 100% branch |
| Profile enforcement | 100% decision table |
| Session lifecycle | 100% state transition |
| Stale-page guards | 100% edge case |
| Custom protocol pipeline | 100% request/response |
| Mutation queue | 100% branch |

### 5. Screenshot on failure

Macro captures screenshots on test failure (like `foundation_browser`):
- In-process: JS canvas screenshot via `evaluate_script`
- Browser: CDP `Page.captureScreenshot`
- Mobile: ADB `screencap` / simctl screenshot

```rust
#[platform_test]
async fn failing_test(session: PlatformTestSession) -> Result<()> {
    session.page().click("#nonexistent").await?; // panics → screenshot captured
    Ok(())
}
// Output: saved screenshot to test_artifacts/failing_test.png
```

## Tasks

### Proc macro
- [ ] Implement `#[platform_test]` proc macro in foundation_macros
- [ ] Default mode: in-process Tauri WebView
- [ ] Browser mode: Chromium via CDP (reuse foundation_browser harness)
- [ ] Android/iOS modes: ADB/simctl targets
- [ ] Screenshot capture on failure

### PlatformTestSession
- [ ] Define `PlatformTestSession` struct
- [ ] Provide `page` for DOM interaction
- [ ] Provide `router` for route handler hooks
- [ ] Provide `capabilities` for mocking
- [ ] Provide `cache` for inspection
- [ ] Provide `events` for capture

### In-process harness
- [ ] Start Tauri app in test mode (no window needed for logic tests)
- [ ] Create PlatformSession with test configuration
- [ ] Wire route handlers, capabilities, cache
- [ ] Test: route handler chain exercise

### Coverage enforcement
- [ ] Define coverage targets per component
- [ ] Integrate with tarpaulin or cargo-llvm-cov for branch coverage

## Verification Commands

```bash
cargo test --package foundation_platform --test platform_tests
cargo test --package foundation_platform -- coverage
```
