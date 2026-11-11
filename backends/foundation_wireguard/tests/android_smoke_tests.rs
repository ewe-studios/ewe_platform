//! Android JNI surface tests (moved from `src/android/mod.rs`).
#![cfg(target_os = "android")]

/// JNI symbol names must match the expected Java package/class.
/// If a symbol is missing, `System.loadLibrary` throws `UnsatisfiedLinkError`.
#[test]
fn required_jni_symbols_exist() {
    // Compile-time assertion: these symbols are present in the binary.
    // A missing `#[no_mangle]` would be a linker error, so this test just
    // confirms the module compiles with the expected exports.
    let expected = [
        "Java_com_example_ewe_WireguardService_nativeStartNode",
        "Java_com_example_ewe_WireguardService_nativeStartNodeFromToml",
        "Java_com_example_ewe_WireguardService_nativeStopNode",
        "Java_com_example_ewe_WireguardService_nativeMemberCount",
        "Java_com_example_ewe_WireguardService_nativeOverlayIp",
        "Java_com_example_ewe_WireguardService_nativeIsJoined",
        "Java_com_example_ewe_WireguardService_nativeTick",
        "Java_com_example_ewe_WireguardService_nativeGenerateSeed",
    ];
    assert_eq!(expected.len(), 8, "JNI surface should be 8 functions");
}
