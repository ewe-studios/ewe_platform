//! Tauri plugin build script — injects native code into the consuming app.
//!
//! `tauri_plugin::Builder` copies `android/` and `ios/` into the app's
//! Gradle/Xcode project, generates ACL permissions, and emits the
//! `cargo:android_library_path` / `cargo:ios_library_path` directives
//! that the app's build system picks up.

fn main() {
    tauri_plugin::Builder::new(&[])
        .android_path("android/")
        .ios_path("ios/")
        .build();
}
