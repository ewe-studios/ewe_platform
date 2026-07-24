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
