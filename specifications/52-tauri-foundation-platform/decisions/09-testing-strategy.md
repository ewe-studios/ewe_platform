# 09 — Testing strategy: macro-driven test harness, browser + device coverage

**Date:** 2026-07-04
**Status:** Resolved

### Decision

`foundation_platform` follows the established testing patterns from
`foundation_browser` and `foundation_testbed`. Tests are simple Rust functions
annotated with a proc macro. The macro handles: server startup, browser/WebView
launch, deterministic teardown (on success, error, or panic), and screenshot
capture on failure. The same test can target browser (CDP/BiDi), Tauri WebView
(in-process), Android emulator, or iOS simulator.

### The established pattern (from foundation_browser)

`foundation_browser` already provides a clean test harness (spec-43):

```rust
// foundation_browser/src/test/harness.rs
#[wasm_ui_server]
async fn my_test(page: Page) -> Result<()> {
    // page is a live browser page with the app loaded
    // server is running in background, browser connected via CDP
    page.wait_for_selector("#app-ready").await?;
    let text = page.locator(".result").text().await?;
    assert_eq!(text, "expected");
    Ok(())
}
// Harness handles: server setup, browser launch, CDP connection,
// panic capture, screenshot on failure, deterministic teardown.
```

`foundation_platform` extends this to Tauri and mobile targets through the
same macro-driven approach:

```rust
// foundation_platform testing
#[platform_test]           // runs in Tauri WebView (in-process, like a unit test)
#[platform_test(browser)]  // runs in Chromium via CDP (like foundation_browser)
#[platform_test(android)]  // runs in Android emulator via ADB
#[platform_test(ios)]      // runs in iOS simulator via simctl
async fn my_test(session: PlatformTestSession) -> Result<()> {
    // session provides: WebView access, route handler hooks,
    // capability mocking, cache inspection, event capture
    let page = session.page();
    page.click("#my-button").await?;
    let result = page.locator(".output").text().await?;
    assert_eq!(result, "button clicked");
    Ok(())
}
```

### Test targets

| Target | What runs | What's tested | Infrastructure |
|---|---|---|---|
| `#[platform_test]` | In-process Tauri WebView (Wry) | Core platform APIs, session backbone, route policy, protocol rendering | Pure Rust, no external dependencies. Runs in CI without a display. |
| `#[platform_test(browser)]` | Chromium via CDP | Full end-to-end: browser rendering, JS runtime, real paint/layout, trusted input | `foundation_browser`'s CDP engine. Requires Chromium in CI. |
| `#[platform_test(bidi)]` | Firefox/BiDi (future) | Cross-browser rendering verification | `foundation_browser`'s BiDi engine. Future. |
| `#[platform_test(android)]` | Android emulator via ADB | Mobile WebView behavior, mobile lifecycle, mobile-specific capabilities, Tauri Android integration | Android SDK + emulator in CI. Tauri's Android test infrastructure. |
| `#[platform_test(ios)]` | iOS simulator via simctl | Mobile WebView behavior, native bridges, iOS-specific lifecycle, Tauri iOS integration | Xcode + simulator in CI. |

### What the macro provides

```rust
#[platform_test]
#[platform_test(
    target = "android",
    profile = "trustedRemote",     // WebView profile for the test
    routes = ["/app/test"],        // routes to register
    capabilities = [Camera],       // capabilities to mock
    cache_policy = "cache-first",  // cache behavior
    headless = true,               // headless mode
    window_size = (800, 800),      // viewport
)]
async fn my_test(session: PlatformTestSession) -> Result<()> { ... }
```

**Setup (before test runs):**
1. Start test server with app content (inline HTML, pre-built WASM bundle, or static assets).
2. Launch the target (browser process, Tauri WebView, or emulator/simulator).
3. Connect session (CDP WebSocket, in-process handle, or ADB/simctl bridge).
4. Register route handlers and mock capabilities.
5. Navigate to the test page.
6. Wait for the app to signal readiness.

**Test body:** User code interacts with the running app — clicks, assertions,
capability calls, navigation, cache lookups, event verification.

**Teardown (always runs, even on panic):**
1. Capture screenshot on failure (while page is still alive).
2. Collect console logs, network requests, capability call traces.
3. Close browser/WebView/emulator.
4. Stop test server.
5. Assertions framework reports results.

Field declaration order = drop order = teardown order (browser before server),
same as `foundation_browser::Harness`.

### Test session API

```rust
struct PlatformTestSession {
    page: Page,  // foundation_browser-style Page API (click, type, locate, wait)
}

impl PlatformTestSession {
    // Route handler inspection
    fn route_was_called(&self, route: &str) -> bool;
    fn last_route_decision(&self) -> Option<RouteDecision>;

    // Capability mocking and inspection
    fn mock_capability<C: Capability>(&self, response: CapabilityResponse);
    fn capability_was_requested(&self, name: &str) -> bool;
    fn capability_requests(&self) -> Vec<CapabilityRequest>;

    // Cache inspection
    fn cache_contains(&self, route: &str) -> bool;
    fn cache_entry(&self, route: &str) -> Option<CachedPage>;

    // Event capture
    fn events_of_type(&self, event: &str) -> Vec<PlatformEvent>;
    fn emitted_to_webview(&self, event: &str) -> bool;

    // Navigation
    fn navigate(&self, route: &str) -> Result<()>;
    fn current_route(&self) -> String;

    // Lifecycle simulation (mobile)
    fn simulate_background(&self);
    fn simulate_foreground(&self);
    fn simulate_offline(&self);
    fn simulate_online(&self);
}
```

### What can be tested in-process (no browser)

`#[platform_test]` runs Tauri's WebView in-process via Wry. It does not require
a real browser. This is fast — like a unit test. It covers:

- Route handler chain execution (in-process session)
- Protocol encoding/decoding (DomOps, Arrow/ArrowIpc, JSON, HTML through the
  `UriSchemeProtocol` handler)
- Cache store/retrieve/invalidate (in-process SQLite)
- Capability registry and permission checks (without actual device hardware)
- Profile enforcement (in-process access gates)
- Session lifecycle (create, navigate, shutdown)
- Stale-page guard logic (route identity verification)
- Custom protocol response pipeline

### What requires a real browser

`#[platform_test(browser)]` via CDP covers:

- Real DOM rendering (paint, layout, CSS, fonts)
- JS runtime behavior (`foundation-wasm-ui.js`, Turbo-style navigation,
  morphing, event handling)
- User interaction (click, type, scroll, focus)
- Visual regression (screenshot comparison)
- CSP enforcement (real browser CSP engine)
- Cross-origin behavior
- Service worker registration (in browser context)

### What requires mobile targets

`#[platform_test(android)]` and `#[platform_test(ios)]` cover:

- Mobile WebView behavior differences (WKWebView vs Android System WebView)
- Mobile lifecycle (background/foreground, memory pressure, app suspension)
- Native bridge invocation (Swift/Kotlin code paths)
- Platform-specific capability behavior (biometrics, camera, share sheet)
- Mobile network conditions (cellular, offline, slow connection)
- App store update flow (WASM hot-swap)
- Mobile-specific CSP and security behavior

### CI strategy

| Target | CI requirement | Runtime per test |
|---|---|---|
| `#[platform_test]` | Rust toolchain only | ~100ms |
| `#[platform_test(browser)]` | Chromium installed | ~2-5s |
| `#[platform_test(android)]` | Android SDK + emulator | ~30-60s |
| `#[platform_test(ios)]` | macOS + Xcode + simulator | ~30-60s |

In-process tests run on every commit. Browser tests run on PR. Mobile tests
run on merge to main or release branch. All tests are deterministic — no
flakiness tolerated. Flaky tests are quarantined and must be fixed before
un-quarantining.

### Integration with foundation_testbed

`foundation_testbed` already provides `vms` (QEMU/KVM cross-platform VM build
and test) and `wasm` (browser via CDP/BiDi, Deno, Cloudflare Workers).
`foundation_platform` tests integrate with the existing `wasm` harness for
browser tests and extend it with Tauri in-process and mobile targets. The
`foundation_testbed::wasm` module is the entrypoint for all browser-driven
platform tests.

### Test coverage expectations

| Component | Coverage target | Test type |
|---|---|---|
| Route handler chain | 100% branch | In-process |
| Capability registry | 100% branch | In-process |
| Protocol encoding | 100% output comparison | In-process + browser |
| Cache store/retrieve | 100% branch | In-process |
| Profile enforcement | 100% decision table | In-process |
| Session lifecycle | 100% state transition | In-process |
| Stale-page guards | 100% edge case | In-process |
| Custom protocol pipeline | 100% request/response | In-process |
| Mutation queue | 100% branch | In-process |
| Background workers | Key scenarios | In-process |
| DOM rendering | Key scenarios | Browser |
| JS runtime behavior | Key scenarios | Browser |
| Mobile lifecycle | Key scenarios | Android/iOS |
| Native bridges | Per-bridge | Android/iOS |
