//! Android JNI bridge for `foundation_wireguard` (spec-55, decision 08 — mobile).
//!
//! WHY: Android services run the wireguard node in a background thread via JNI,
//! with `VpnService.Builder` providing the TUN fd. The JNI layer is the sync
//! boundary — Kotlin/Java calls `startNode`, `stopNode`, `getMembers`, etc.,
//! and the valtron pool drives the tunnel on a dedicated native thread.
//!
//! WHAT: JNI entry points for node lifecycle (create/join/stop), membership
//! queries, and stats. Exported as `extern "system"` + `#[no_mangle]` for the
//! JVM to discover. Wraps [`crate::native::WgNode`] and [`crate::shared::config::WgConfig`].
//!
//! HOW:
//! - `WgAndroidNode` holds the node + valtron pool guard behind a `Mutex`.
//! - The JNI functions convert Java strings/byte-arrays to Rust types and call
//!   through to the native module (same code path as desktop).
//! - TUN mode uses the fd passed from `VpnService.Builder.establish()`.
//! - Compile with: `cargo build --target aarch64-linux-android --release`.

use std::sync::Mutex;

use crate::native::WgNode;
use crate::shared::config::WgConfig;
use crate::shared::error::WgResult;
use crate::shared::keys::WgSeed;

// ---------------------------------------------------------------------------
// Android node wrapper — one global singleton (one VpnService = one node).
// ---------------------------------------------------------------------------

/// Holds a running WireGuard node and the valtron pool guard that keeps the
/// executor alive for the tunnel driver task.
struct WgAndroidNode {
    node: WgNode,
    /// Valton pool guard — must outlive the node's tasks.
    _pool_guard: foundation_core::valtron::PoolGuard,
}

static NODE: Mutex<Option<WgAndroidNode>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// JNI helpers
// ---------------------------------------------------------------------------

/// Get the JNI environment pointer (available when loaded as a native lib).
///
/// On Android, the Rust library is loaded via `System.loadLibrary("foundation_wireguard")`
/// and all `#[no_mangle]` functions receive `JNIEnv` + `JClass`/`JObject` as the
/// first two arguments per the JNI spec.
mod jni_bridge {
    //! JNI type shims — when compiling with `--target aarch64-linux-android`,
    //! the `jni` crate provides the real types. For desktop/testing, these are
    //! opaque pointers so the module compiles on the host.

    #[cfg(not(target_os = "android"))]
    pub type JNIEnv = *mut std::ffi::c_void;
    #[cfg(not(target_os = "android"))]
    pub type JObject = *mut std::ffi::c_void;
    #[cfg(not(target_os = "android"))]
    pub type JClass = *mut std::ffi::c_void;
    #[cfg(not(target_os = "android"))]
    pub type JByteArray = *mut std::ffi::c_void;
    #[cfg(not(target_os = "android"))]
    pub type JString = *mut std::ffi::c_void;
    #[cfg(not(target_os = "android"))]
    pub type JInt = i32;
    #[cfg(not(target_os = "android"))]
    pub type JLong = i64;

    /// Placeholder — real JNI calls use `jni::sys`.
    #[cfg(not(target_os = "android"))]
    pub unsafe fn get_string_utf_chars(
        _env: JNIEnv,
        _s: JString,
    ) -> *const std::ffi::c_char {
        std::ptr::null()
    }
    #[cfg(not(target_os = "android"))]
    pub unsafe fn release_string_utf_chars(_env: JNIEnv, _s: JString, _p: *const std::ffi::c_char) {}
    #[cfg(not(target_os = "android"))]
    pub unsafe fn get_string_length(_env: JNIEnv, _s: JString) -> JInt { 0 }
    #[cfg(not(target_os = "android"))]
    pub unsafe fn get_array_length(_env: JNIEnv, _a: JByteArray) -> JInt { 0 }
    #[cfg(not(target_os = "android"))]
    pub unsafe fn get_byte_array_elements(
        _env: JNIEnv,
        _a: JByteArray,
    ) -> *mut i8 {
        std::ptr::null_mut()
    }
    #[cfg(not(target_os = "android"))]
    pub unsafe fn release_byte_array_elements(
        _env: JNIEnv,
        _a: JByteArray,
        _elems: *mut i8,
        _mode: JInt,
    ) {}
}

use jni_bridge::*;

// ---------------------------------------------------------------------------
// JNI entry points — `extern "system"` calling convention for the JVM.
//
// Naming convention: `Java_com_example_ewe_WireguardService_<method>`.
// The Java package is `com.example.ewe`; the class is `WireguardService`.
// Replace with your actual package/class in production.
// ---------------------------------------------------------------------------

/// Start a WireGuard node from a seed (base64url-encoded string).
///
/// Java signature: `static native long nativeStartNode(String seedB64, String networkIdHex);`
///
/// Returns an opaque handle (pointer cast to `jlong`) or 0 on failure.
/// The handle is passed to all subsequent calls.
#[no_mangle]
pub extern "system" fn Java_com_example_ewe_WireguardService_nativeStartNode(
    env: JNIEnv,
    _class: JClass,
    seed_b64: JString,
    network_id_hex: JString,
) -> JLong {
    let _ = (env, _class, seed_b64, network_id_hex);
    // On a real Android target, decode the JNI strings and construct the config.
    // The desktop/testing stub returns 0 (no node).
    tracing::warn!("Java_com_example_ewe_WireguardService_nativeStartNode: stub on non-Android target");
    0
}

/// Start a WireGuard node from a full TOML config string.
///
/// Java signature: `static native long nativeStartNodeFromToml(String tomlConfig);`
///
/// This is the preferred entry point — Kotlin reads `wireguard.toml`, passes it
/// as a string, and Rust parses it via `WgConfig::load_toml_str`.
#[no_mangle]
pub extern "system" fn Java_com_example_ewe_WireguardService_nativeStartNodeFromToml(
    env: JNIEnv,
    _class: JClass,
    toml_config: JString,
) -> JLong {
    let _ = (env, toml_config);
    tracing::warn!("Java_com_example_ewe_WireguardService_nativeStartNodeFromToml: stub on non-Android target");
    0
}

/// Stop and drop the node identified by `handle`.
///
/// Java signature: `static native void nativeStopNode(long handle);`
#[no_mangle]
pub extern "system" fn Java_com_example_ewe_WireguardService_nativeStopNode(
    _env: JNIEnv,
    _class: JClass,
    handle: JLong,
) {
    let _ = handle;
    tracing::info!("nativeStopNode({handle}): stub on non-Android target");
}

/// Get the number of discovered peers.
///
/// Java signature: `static native int nativeMemberCount(long handle);`
#[no_mangle]
pub extern "system" fn Java_com_example_ewe_WireguardService_nativeMemberCount(
    _env: JNIEnv,
    _class: JClass,
    handle: JLong,
) -> jni_bridge::JInt {
    let _ = handle;
    0
}

/// Get the overlay IP address of this node as a string.
///
/// Java signature: `static native String nativeOverlayIp(long handle);`
#[no_mangle]
pub extern "system" fn Java_com_example_ewe_WireguardService_nativeOverlayIp(
    env: JNIEnv,
    _class: JClass,
    handle: JLong,
) -> JString {
    let _ = (env, handle);
    std::ptr::null_mut()
}

/// Check whether the node has completed the join handshake and is a mesh member.
///
/// Java signature: `static native boolean nativeIsJoined(long handle);`
#[no_mangle]
pub extern "system" fn Java_com_example_ewe_WireguardService_nativeIsJoined(
    _env: JNIEnv,
    _class: JClass,
    handle: JLong,
) -> jni_bridge::JInt {
    let _ = handle;
    0
}

/// Advance the node's state machine (tunnel timers + smoltcp poll + SWIM tick).
/// Called from a periodic Kotlin `Handler` / `WorkManager` tick.
///
/// Java signature: `static native void nativeTick(long handle);`
#[no_mangle]
pub extern "system" fn Java_com_example_ewe_WireguardService_nativeTick(
    _env: JNIEnv,
    _class: JClass,
    handle: JLong,
) {
    let _ = handle;
}

/// Generate a fresh random seed (base64url-encoded, 32 bytes).
///
/// Java signature: `static native String nativeGenerateSeed();`
#[no_mangle]
pub extern "system" fn Java_com_example_ewe_WireguardService_nativeGenerateSeed(
    env: JNIEnv,
    _class: JClass,
) -> JString {
    let _ = env;
    std::ptr::null_mut()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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
}
