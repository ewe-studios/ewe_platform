---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F36-android-tauri-tests"
this_file: "specifications/52-tauri-foundation-platform/features/F36-android-tauri-tests/feature.md"

status: pending
priority: critical
created: 2026-07-21

depends_on:
  - "F14-android-example"
  - "F30-docker-test-infra"

tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---

# F36 — Android Tauri: build APK, install on emulator, run tests

## Problem

The Android emulator (ewe-android container) is running with ADB verified,
but `cargo tauri android build` fails with a Gradle version mismatch between
the generated `build.gradle.kts` (expects 8.11.0) and the installed AGP.
The APK can't be built, installed, or tested end-to-end.

## Solution

1. Fix the Gradle version mismatch in the generated Android project
2. Build a debug APK via `cargo tauri android build --debug`
3. Install on the emulator via `adb install`
4. Launch the app via `adb shell am start`
5. Take screenshots to verify the app renders correctly
6. Wire into `TestEnvironment` so Rust tests can drive this flow

## Requirements

### R1. Fix Gradle/AGP version
- Align `build.gradle.kts` AGP version with what's installed on system
- Or install correct Gradle wrapper version in the Android Dockerfile
- `cargo tauri android build --debug` must succeed

### R2. Build APK
- `cargo tauri android build --debug` produces debug APK
- Output: `gen/android/app/build/outputs/apk/debug/app-debug.apk`

### R3. Install on emulator
- `adb -e install app-debug.apk`
- Verify installation: `adb -e shell pm list packages | grep com.ewe`

### R4. Launch and screenshot
- `adb -e shell am start -n com.ewe.platform/.MainActivity`
- Wait for app to render (poll `adb logcat` for session ID)
- Screenshot via `adb exec-out screencap -p` → verify content

### R5. ADB input testing
- `adb shell input tap x y` to interact with the app
- Verify navigation between routes
- Screenshot after each interaction

### R6. Rust TestEnvironment integration
- `TestEnvironment::install_apk(path)` — copies APK, runs adb install
- `TestEnvironment::launch_android_app(package, activity)` — am start
- `TestEnvironment::android_screenshot() -> Vec<u8>` — adb screencap
- `TestEnvironment::android_tap(x, y)` — adb shell input tap

### R7. CI pipeline
- GitHub Actions: build APK, boot emulator, install, screenshot
- Verify app displays the platform dashboard
- Fail if APK doesn't build or app doesn't launch

## Verification

```bash
# Build APK
cargo tauri android build --debug

# Install + launch
adb -e install app-debug.apk
adb -e shell am start -n com.ewe.platform/.MainActivity
sleep 3
adb -e exec-out screencap -p > /tmp/app_screen.png

# Rust: full test
cargo test -p foundation_testbed --features docker-tests -- android_e2e
```

## Files

| File | Action |
|------|--------|
| `examples/platform_android/src-tauri/gen/android/build.gradle.kts` | Fix AGP version |
| `artefacts/dockerfiles/android/Dockerfile` | Add correct Gradle version |
| `backends/foundation_testbed/src/docker.rs` | Add `install_apk()`, `launch_android_app()` |
