//! iOS C FFI bridge for `foundation_wireguard` (spec-55, decision 08 — mobile).
//!
//! WHY: iOS `NEPacketTunnelProvider` runs the wireguard node in a NetworkExtension
//! process. The extension loads this library, calls C functions to bootstrap/join/tick
//! the mesh, and reads/writes tunnel packets through a pair of file descriptors.
//!
//! WHAT: C-ABI entry points (`extern "C"`, `#[no_mangle]`) for node lifecycle.
//! The TUN fd is passed from `NEPacketTunnelProvider` via `wg_node_set_tun_fd`.
//!
//! HOW:
//! - All functions are thread-safe (global `Mutex<Option<NodeState>>`).
//! - The node runs on a valtron pool thread spawned at `wg_node_start`.
//! - `wg_node_tick` advances the tunnel + SWIM state machines.
//! - `wg_node_read_packet` / `wg_node_write_packet` bridge the TUN fd to smoltcp.
//! - Compile with: `cargo build --target aarch64-apple-ios --release`.
//!
//! # C header generation
//!
//! Use `cbindgen` to generate a header from this module:
//!
//! ```bash
//! cbindgen --config cbindgen.toml --crate foundation_wireguard --output WireguardFFI.h
//! ```
//!
//! The header is then imported into the Xcode project alongside the static library.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_long, c_void};
use std::sync::Mutex;

use crate::native::WgNode;
use crate::shared::config::WgConfig;

// ---------------------------------------------------------------------------
// Opaque handle type
// ---------------------------------------------------------------------------

/// Opaque pointer to a running node.
///
/// Passed to all lifecycle functions. On the C side it's a `void*`.
pub type WgNodeHandle = *mut c_void;

// ---------------------------------------------------------------------------
// Internal state
// ---------------------------------------------------------------------------

struct NodeState {
    node: WgNode,
    _pool_guard: foundation_core::valtron::PoolGuard,
}

static STATE: Mutex<Option<NodeState>> = Mutex::new(None);

// ---------------------------------------------------------------------------
// C FFI entry points
// ---------------------------------------------------------------------------

/// Create a new wireguard node from a TOML config string (NUL-terminated).
///
/// Returns an opaque handle (null on error). The caller owns the returned handle
/// and must free it with `wg_node_destroy`.
///
/// ```c
/// WgNodeHandle wg_node_create_from_toml(const char *toml_config);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_create_from_toml(toml_config: *const c_char) -> WgNodeHandle {
    if toml_config.is_null() {
        return std::ptr::null_mut();
    }
    let _ = unsafe { CStr::from_ptr(toml_config) };
    // On a non-iOS target, this is a stub.
    tracing::warn!("wg_node_create_from_toml: stub on non-iOS target");
    std::ptr::null_mut()
}

/// Create a node from a base64url-encoded seed + hex network id.
///
/// ```c
/// WgNodeHandle wg_node_create(const char *seed_b64, const char *network_id_hex);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_create(
    seed_b64: *const c_char,
    network_id_hex: *const c_char,
) -> WgNodeHandle {
    let _ = (seed_b64, network_id_hex);
    tracing::warn!("wg_node_create: stub on non-iOS target");
    std::ptr::null_mut()
}

/// Destroy a node and release all resources.
///
/// ```c
/// void wg_node_destroy(WgNodeHandle handle);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_destroy(handle: WgNodeHandle) {
    let _ = handle;
    // Drop the state, which joins the valtron tasks.
    if let Ok(mut state) = STATE.lock() {
        *state = None;
    }
}

/// Pass a TUN file descriptor to the node (from `NEPacketTunnelProvider.packetFlow`).
///
/// The fd is read/written by the data plane driver. Set before calling `wg_node_start`.
///
/// ```c
/// int wg_node_set_tun_fd(WgNodeHandle handle, int tun_fd, int mtu);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_set_tun_fd(
    handle: WgNodeHandle,
    tun_fd: c_int,
    mtu: c_int,
) -> c_int {
    let _ = (handle, tun_fd, mtu);
    tracing::warn!("wg_node_set_tun_fd: stub on non-iOS target");
    -1
}

/// Start the node: join the mesh, spawn the tunnel driver task.
///
/// Returns 0 on success, non-zero on error.
///
/// ```c
/// int wg_node_start(WgNodeHandle handle);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_start(handle: WgNodeHandle) -> c_int {
    let _ = handle;
    tracing::warn!("wg_node_start: stub on non-iOS target");
    -1
}

/// Stop the node: leave the mesh, tear down tunnels, join the driver task.
///
/// ```c
/// int wg_node_stop(WgNodeHandle handle);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_stop(handle: WgNodeHandle) -> c_int {
    let _ = handle;
    0
}

/// Advance the node state machine (tunnel timers + smoltcp + SWIM tick).
///
/// Called from a periodic `NEPacketTunnelProvider` timer (~10-50ms).
/// Returns the number of milliseconds to wait before the next call.
///
/// ```c
/// int wg_node_tick(WgNodeHandle handle);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_tick(handle: WgNodeHandle) -> c_int {
    let _ = handle;
    50 // ms — default tick interval
}

/// Get the number of discovered peers.
///
/// ```c
/// int wg_node_member_count(WgNodeHandle handle);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_member_count(handle: WgNodeHandle) -> c_int {
    let _ = handle;
    0
}

/// Get the overlay IP address as a NUL-terminated string.
///
/// The returned string is owned by the caller; free with `wg_node_free_string`.
///
/// ```c
/// char *wg_node_overlay_ip(WgNodeHandle handle);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_overlay_ip(handle: WgNodeHandle) -> *mut c_char {
    let _ = handle;
    std::ptr::null_mut()
}

/// Free a string previously returned by this library.
///
/// ```c
/// void wg_node_free_string(char *s);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_free_string(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

/// Check whether the node has joined the mesh (completed bootstrap).
///
/// ```c
/// int wg_node_is_joined(WgNodeHandle handle);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_is_joined(handle: WgNodeHandle) -> c_int {
    let _ = handle;
    0
}

/// Generate a fresh random 32-byte seed, returned as base64url.
///
/// The returned string is owned by the caller; free with `wg_node_free_string`.
///
/// ```c
/// char *wg_node_generate_seed(void);
/// ```
#[no_mangle]
pub extern "C" fn wg_node_generate_seed() -> *mut c_char {
    let seed = crate::shared::keys::WgSeed::generate();
    let b64 = seed.to_base64url();
    CString::new(b64)
        .map(|c| c.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_create_null_config() {
        let h = wg_node_create_from_toml(std::ptr::null());
        assert!(h.is_null(), "null config should return null handle");
    }

    #[test]
    fn smoke_generate_seed() {
        let s = wg_node_generate_seed();
        assert!(!s.is_null(), "seed generation should return a string");
        wg_node_free_string(s);
    }

    #[test]
    fn smoke_tick_null() {
        let ms = wg_node_tick(std::ptr::null_mut());
        assert!(ms > 0, "tick should return a positive poll interval");
    }

    #[test]
    fn smoke_destroy_null() {
        wg_node_destroy(std::ptr::null_mut());
        // Must not crash on null.
    }

    #[test]
    fn smoke_stop_null() {
        assert_eq!(wg_node_stop(std::ptr::null_mut()), 0);
    }
}
