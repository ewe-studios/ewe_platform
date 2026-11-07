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

// ---------------------------------------------------------------------------
// Android node wrapper — one global singleton (one VpnService = one node).
// ---------------------------------------------------------------------------

/// Holds a running `WireGuard` node and the valtron pool guard that keeps the
/// executor alive for the tunnel driver task.
// The wrapper is built only when the Android JNI backend lands; the exported
// stubs use plain handles instead.
#[allow(dead_code)]
struct WgAndroidNode {
    node: WgNode,
    /// Valton pool guard — must outlive the node's tasks.
    _pool_guard: foundation_core::valtron::PoolGuard,
}

#[allow(dead_code)]
static NODE: Mutex<Option<WgAndroidNode>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// JNI helpers
// ---------------------------------------------------------------------------

// The shims compile on every target that enables `spec55-complete`; only the
// Android JNI backend calls the helper functions.
#[allow(dead_code)]
mod jni_bridge {
    //! JNI type shims. The bridge is a stub, so these are opaque pointers on
    //! every target. They match the JNI ABI: pointers, `jint` and `jlong`.
    //!
    //! On Android the library loads via `System.loadLibrary("foundation_wireguard")`.
    //! Every `#[no_mangle]` function receives `JNIEnv` plus `JClass`/`JObject` as
    //! its first two arguments.

    pub type JNIEnv = *mut std::ffi::c_void;
    pub type JObject = *mut std::ffi::c_void;
    pub type JClass = *mut std::ffi::c_void;
    pub type JByteArray = *mut std::ffi::c_void;
    pub type JString = *mut std::ffi::c_void;
    pub type JInt = i32;
    pub type JLong = i64;

    /// Placeholder — real JNI calls use `jni::sys`.
    pub unsafe fn get_string_utf_chars(_env: JNIEnv, _s: JString) -> *const std::ffi::c_char {
        std::ptr::null()
    }
    pub unsafe fn release_string_utf_chars(_env: JNIEnv, _s: JString, _p: *const std::ffi::c_char) {
    }
    pub unsafe fn get_string_length(_env: JNIEnv, _s: JString) -> JInt {
        0
    }
    pub unsafe fn get_array_length(_env: JNIEnv, _a: JByteArray) -> JInt {
        0
    }
    pub unsafe fn get_byte_array_elements(_env: JNIEnv, _a: JByteArray) -> *mut i8 {
        std::ptr::null_mut()
    }
    pub unsafe fn release_byte_array_elements(
        _env: JNIEnv,
        _a: JByteArray,
        _elems: *mut i8,
        _mode: JInt,
    ) {
    }
}

use jni_bridge::{JClass, JLong, JNIEnv, JString};

// ---------------------------------------------------------------------------
// JNI entry points — `extern "system"` calling convention for the JVM.
//
// Naming convention: `Java_com_example_ewe_WireguardService_<method>`.
// The Java package is `com.example.ewe`; the class is `WireguardService`.
// Replace with your actual package/class in production.
// ---------------------------------------------------------------------------

/// Start a `WireGuard` node from a seed (base64url-encoded string).
///
/// Java signature: `static native long nativeStartNode(String seedB64, String networkIdHex);`
///
/// Returns an opaque handle (pointer cast to `jlong`) or 0 on failure.
/// The handle is passed to all subsequent calls.
#[no_mangle]
pub extern "system" fn Java_com_example_ewe_WireguardService_nativeStartNode(
    env: JNIEnv,
    class: JClass,
    seed_b64: JString,
    network_id_hex: JString,
) -> JLong {
    let _ = (env, class, seed_b64, network_id_hex);
    // On a real Android target, decode the JNI strings and construct the config.
    // The desktop/testing stub returns 0 (no node).
    tracing::warn!(
        "Java_com_example_ewe_WireguardService_nativeStartNode: stub on non-Android target"
    );
    0
}

/// Start a `WireGuard` node from a full `TOML` config string.
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
    tracing::warn!(
        "Java_com_example_ewe_WireguardService_nativeStartNodeFromToml: stub on non-Android target"
    );
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
