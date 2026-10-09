---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F38-presentation-e2e-tests"
this_file: "specifications/52-tauri-foundation-platform/features/F38-presentation-e2e-tests/feature.md"

status: in-progress
priority: critical
created: 2026-07-22

depends_on:
  - "F06-webview-stack"
  - "F35-multi-webview-desktop"
  - "F36-android-tauri-tests"

tasks:
  completed: 4
  uncompleted: 6
  total: 10
  completion_percentage: 40%
---

# F38 — E2E visual presentation tests (desktop + Android emulator)

## Problem

F06/F35 prove the data pipeline (254 tests) but have never been verified
visually. The user needs to see the hybrid model work: Push creates a new
WebView window, the old one gets screenshotted and destroyed, back
navigation shows the screenshot then loads live content.

## Solution

E2E tests that drive the real desktop app and Android emulator, verifying:
1. Desktop: Push creates a second window
2. Desktop: Back destroys the top window, shows previous
3. Android: Push navigates, back shows previous route
4. Screenshot pixel comparison: screenshot before push ≠ screenshot after

### Desktop visual tests

```bash
# 1. Launch the app
./src-tauri/target/debug/platform_android &
APP_PID=$!

# 2. Navigate to /presentation/ (root)
# 3. Click Push → verify new window appears (xdotool search or wmctrl -l)
# 4. Click Back → verify top window closes, previous shown
# 5. Take screenshots at each step for comparison
```

### Android emulator tests

```bash
# 1. Build and install APK
cargo tauri android build --debug
adb -e install -r app.apk

# 2. Launch app, navigate via ADB input
adb shell am start -n com.ewe.platform/.MainActivity
adb shell input tap 500 500  # tap Push button

# 3. Screenshot and compare
adb exec-out screencap -p > before_push.png
adb shell input tap 500 800  # tap Push button
adb exec-out screencap -p > after_push.png

# 4. Verify screenshots are different (page content changed)
```

## Requirements

### R1. Desktop: verify windows with wmctrl ✅
- `wmctrl -l` lists all open windows by title
- Push/ModeReportPage should show "Push Demo" title
- Back should reduce window count

### R2. Desktop: screenshot via imagemagick ✅
- `import -window <id> screenshot.png` captures specific window
- Compare screenshots: `compare before.png after.png diff.png`

### R3. Android: ADB screenshot + compare ✅
- `adb exec-out screencap -p` captures the emulator screen
- Compare before/after navigation

### R4. Android: ADB tap to simulate button clicks 🔄
- Map button coordinates from the presentation page layout
- Tap specific buttons, verify screen content changed

### R5: Presentation mode verification matrix 🔄

| Mode | Desktop | Android | Expected |
|------|---------|---------|----------|
| Push | New window | New WebView | Depth +1, route changes |
| Morph | Same window | Same WebView | Depth unchanged, route changes |
| Replace | Same window | Same WebView | Depth unchanged, route changes |
| Modal | New window | New WebView | Depth +1 (same as push on mobile) |
| Root | All windows close, 1 new | All WebViews close, 1 new | Depth→1 |
| External | System browser | System browser | Depth unchanged |

### R6: Visual regression test in CI 🔄
- GitHub Actions: boot desktop + Android emulator
- Run the E2E script
- Compare screenshots to golden images
- Fail if visual difference > threshold

## Verification

```bash
# Desktop
RUST_LOG=info ./src-tauri/target/debug/platform_android &
sleep 3
# Screenshot the window
import -window "$(xdotool search --name 'Foundation Platform' | head -1)" /tmp/desktop_initial.png
# Check presentation routes via wmctrl
wmctrl -l | grep "Foundation"

# Android (Docker emulator)
docker exec ewe_android adb shell am start -n com.ewe.platform/.MainActivity
sleep 5
docker exec ewe_android adb exec-out screencap -p > /tmp/android_initial.png
```

## Files

| File | Action |
|------|--------|
| `tests/e2e/presentation_desktop.sh` | **NEW** — Desktop visual test script |
| `tests/e2e/presentation_android.sh` | **NEW** — Android visual test script |
| `.github/workflows/presentation-e2e.yml` | **NEW** — CI pipeline |
