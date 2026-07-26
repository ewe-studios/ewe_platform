# Android App Testing

## Directives

1. **Check before assuming**: Always check if the app is already installed before installing. Use `adb shell pm list packages | grep <package>` to verify.

2. **Deploy and boot**: Install/update the APK, then launch. Capture logs AND screenshot. Do not skip either.

3. **Validate state**: Only consider the app working when BOTH conditions are met:
   - Logs show no fatal errors (no `SyntaxError`, no `Error: Not found`, no `unreachable`)
   - Screenshot shows the expected content visible on screen

4. **Fix on failure**: If the app state is bad:
   - Identify the root cause from logs
   - Fix the code
   - Rebuild the APK
   - Re-deploy
   - Verify again
   - Do NOT claim something works until both log + screenshot validation pass

## Commands

```bash
# Check if app is installed
adb -s emulator-5554 shell pm list packages | grep com.ewe.platform

# Check if emulator is running
adb devices | grep emulator

# Start emulator (if not running)
/home/darkvoid/Android/Sdk/emulator/emulator -avd test_x86_64 -gpu host &

# Wait for boot
adb wait-for-device && while [ "$(adb shell getprop sys.boot_completed | tr -d '\r')" != "1" ]; do sleep 3; done

# Clear logs and restart app
adb -s emulator-5554 logcat -c
adb -s emulator-5554 shell am force-stop com.ewe.platform
adb -s emulator-5554 shell am start -n com.ewe.platform/.MainActivity

# Wait for app to load
sleep 5

# Capture logs (filter for relevant tags)
adb -s emulator-5554 logcat -d | grep -E "Console|RustStdoutStderr|chromium.*Error|Tauri" | grep -v "WifiStaIface\|WARNING\|bluetooth\|SELinux\|AppOps"

# Capture screenshot
adb -s emulator-5554 exec-out screencap -p > /tmp/android_app.png

# Build APK with Java 17
export JAVA_HOME=$(mise where java@17)
export ANDROID_HOME=/home/darkvoid/Android/Sdk
cd /home/darkvoid/Boxxed/@dev/ewe_platform/examples/platform_android/src-tauri/gen/android
./gradlew :app:assembleDebug -x rustBuildArm64Debug -x rustBuildArmDebug -x rustBuildX86_64Debug -x rustBuildX86Debug

# Build Rust .so for x86_64 (emulator)
export NDK_HOME=$ANDROID_HOME/ndk/27.0.12077973
export TOOLCHAIN=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64
cd /home/darkvoid/Boxxed/@dev/ewe_platform/examples/platform_android/src-tauri
CC_x86_64_linux_android=$TOOLCHAIN/bin/x86_64-linux-android24-clang \
AR_x86_64_linux_android=$TOOLCHAIN/bin/llvm-ar \
CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=$TOOLCHAIN/bin/x86_64-linux-android24-clang \
cargo build --target x86_64-linux-android --lib
cp target/x86_64-linux-android/debug/libplatform_android_lib.so \
   gen/android/app/src/main/jniLibs/x86_64/

# Install APK
adb -s emulator-5554 install -r gen/android/app/build/outputs/apk/x86_64/debug/app-x86_64-debug.apk
```

## Validation checklist

- [ ] `RustStdoutStderr: [platform_android] Session: SessionId(...)` — Rust init OK
- [ ] No `SyntaxError` in Console logs — JS loaded correctly
- [ ] No `Error: Not found` or `404` — assets served correctly  
- [ ] Screenshot shows app content (not blank/white screen)
- [ ] `<script>` tags loaded without errors

## Interacting with WebView content

This app uses Tauri WebViews — `uiautomator dump` CANNOT see HTML-rendered buttons since they live inside the WebView, not the Android native layout tree. Use these approaches instead:

### 1. Inject an E2E test script into the page (preferred)

The WASM app template at `backends/foundation_wasm_ui/src/build_tools/mod.rs` injects a `<script>` block into `index.html`. Add test calls there so they fire on every page load — no need to tap coordinates:

```javascript
// In the template, after bundle.js loads:
setTimeout(function() {
  if (typeof invokeIpc !== 'function') return;
  console.error('[TEST] firing present_modal...');
  invokeIpc('chrome', 'present_modal', {url:'http://ewe.localhost/app/',style:'bottom_sheet',title:'Test'})
    .then(function(r) { console.error('[TEST] MODAL OK ' + JSON.stringify(r)); })
    .catch(function(e) { console.error('[TEST] MODAL FAIL ' + e.message); });
}, 2000);
```

This is the MOST RELIABLE method. The script fires 2s after page load, no coordinate guessing. The build.rs (`WasmBundleGenerator`) picks up changes to the template and regenerates the bundle automatically.

### 2. Tap via adb (fallback)

If you must tap manually, find coordinates by looking at a screenshot and estimating positions relative to the 1080x2400 canvas. WebView buttons are typically at known Y offsets. For the dashboard app:

- `adb shell input tap 540 350` — Present Modal button (centered, ~15% from top)
- Try `adb shell input tap 540 Y` for Y in 200-800 range

Always take a screenshot BEFORE and AFTER the tap to confirm the UI changed.

### 3. Check for SheetWryActivity launch

After tapping, verify the modal actually appeared:

```bash
adb shell dumpsys activity activities | grep "topResumedActivity"
```

If it shows `SheetWryActivity`, the modal launched. If it stays `MainActivity`, either the button wasn't hit or the IPC path is broken.

### 4. Common gotcha: stale .so files

After `cargo clean`, the APK may have stale assets. Always:
- Delete all `.so` files under `gen/android/` and `target/` before rebuilding
- Use `cargo tauri android build --target x86_64 --debug` which handles both Rust and Gradle
- Verify the APK content: `unzip -p app.apk assets/app/v0.1.0/index.html` to confirm your test script is inside
- If the Tauri Console doesn't show your test logs, the APK has STALE assets — nuke `gen/android/app/build` and `public/app/`, then rebuild

### 5. Recreating the emulator

When things get inexplicably broken, delete and recreate:

```bash
adb -s emulator-5554 emu kill
/home/darkvoid/Android/Sdk/cmdline-tools/latest/bin/avdmanager delete avd -n test_x86_64
/home/darkvoid/Android/Sdk/emulator/emulator -avd test_x86_64 -gpu host &
adb wait-for-device
while [ "$(adb shell getprop sys.boot_completed | tr -d '\r')" != "1" ]; do sleep 3; done
```

### 6. build.rs must run for WASM bundles

The WASM bundle (`public/app/v0.1.0/bundle.js`, `index.html`, etc.) is generated by `examples/platform_android/build.rs`. If `public/app/` is empty, build from the root crate first:

```bash
cargo build --manifest-path examples/platform_android/Cargo.toml --target x86_64-linux-android
```

This runs `build.rs` → `WasmBundleGenerator` → populates `src-tauri/public/app/v0.1.0/`. Then build the `.so` via the `src-tauri` workspace.

### 7. Full clean build sequence

```bash
cargo clean --manifest-path examples/platform_android/src-tauri/Cargo.toml
find examples/platform_android -name "*.so" -type f -delete
find examples/platform_android -name "*.apk" -type f -delete
rm -rf examples/platform_android/src-tauri/gen/android/app/build
rm -rf examples/platform_android/src-tauri/public/app

# Build WASM bundles first (runs build.rs)
cargo build --manifest-path examples/platform_android/Cargo.toml --target x86_64-linux-android

# Build APK (handles Rust .so + Gradle)
cd examples/platform_android/src-tauri
cargo tauri android build --target x86_64 --debug
```
