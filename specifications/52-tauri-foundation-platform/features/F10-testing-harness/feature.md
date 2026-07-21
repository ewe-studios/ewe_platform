---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F10-testing-harness"
this_file: "specifications/52-tauri-foundation-platform/features/F10-testing-harness/feature.md"

status: completed
priority: high
created: 2026-07-17

depends_on:
  - "F01-session-backbone"

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# F10 — `#[platform_test]` macro

## Overview

`#[platform_test]` proc macro for in-process platform tests. Extends
`foundation_browser`'s harness pattern. Browser tests use `foundation_browser`
CDP/BiDi — no Docker needed. Docker-based cross-platform tests are F13.

[Decision 09](../decisions/09-testing-strategy.md).

---

### A.1 — Test variants

```rust
#[platform_test]              // in-process Tauri WebView (fast, no browser)
#[platform_test(browser)]     // Chromium via foundation_browser CDP/BiDi
```

### A.2 — PlatformTestSession

```rust
pub struct PlatformTestSession {
    page: TestPage,
    router: TestRouter,
    capabilities: TestCapabilities,
    cache: TestCache,
    events: TestEventCapture,
}
```

### A.3 — Screenshot on failure

JS canvas capture for in-process, CDP `Page.captureScreenshot` for browser.

### A.4 — Coverage targets

| Component | Target |
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

---

## Verification
```bash
cargo test --package foundation_platform --test platform_tests
```
