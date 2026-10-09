# Android proof-of-concept

This is a hand-written Java `MainActivity` that loads hardcoded HTML into a raw
Android `WebView`. It does NOT use foundation_platform, foundation_wasm_ui, or
any Rust code. It is a placeholder proving the Android emulator, SDK toolchain,
and APK build pipeline work.

The real foundation_platform Android path is Tauri v2 mobile:
  1. `cargo tauri android init` — generates the Tauri Android project
  2. Foundation_platform compiles to a native `.so` via NDK
  3. Tauri's JNI bridge loads the Rust code
  4. The WebView runs foundation-wasm-ui.js

Build (standalone, no Tauri):
  javac --release 11 -cp $ANDROID_HOME/platforms/android-34/android.jar \
    -d classes java/com/ewe/platform/MainActivity.java
  d8 --lib $ANDROID_HOME/platforms/android-34/android.jar --output . classes/com/ewe/platform/MainActivity.class
  aapt package -f -M AndroidManifest.xml -I $ANDROID_HOME/platforms/android-34/android.jar -S res -F unsigned.apk
  zip -0 unsigned.apk classes.dex
  zipalign -f 4 unsigned.apk aligned.apk
  apksigner sign --ks ~/.android/debug.keystore aligned.apk
  adb install aligned.apk
