//! Platform handles, dispatch traits, and resource registries (F41 Part C).
//!
//! WHY: IPC handlers that call native APIs (camera, biometrics) need OS-level
//! handles — Android Activity, iOS UIViewController, etc. Pure-Rust handlers
//! don't need these. This module provides the handle types, platform-specific
//! dispatch traits, calling context, and the resource registries that map
//! opaque u64 tokens → native resources.
//!
//! WHAT:
//!   - `AndroidHandle`, `IosHandle`, `DesktopHandle` — platform-specific
//!     OS handles injected by `PlatformBuilder` at startup.
//!   - `AndroidIpc`, `IosIpc` — platform-specific dispatch traits. Handlers
//!     implement these to receive the platform handle at invoke time.
//!   - `IpcInvokeContext` — who called this IPC (page, webview label, session).
//!   - `CapabilityHandleRegistry` — maps u64 tokens → native OS resources.
//!     Same pattern as `StreamRegistry` (F28).
//!   - `HostStreamRegistry` — handler creates a stream, WASM polls it.
//!
//! HOW: `PlatformBuilder` stores the handle on `PlatformSession`. The session's
//! `invoke_ipc()` method tries native handlers first (AndroidIpc/IosIpc), then
//! falls back to pure Rust handlers.

use std::any::Any;
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use foundation_ui_traits::PageIdentity;
use foundation_wasm::ipc::{Ipc, IpcError, IpcRequest, IpcResponse};
use crate::PlatformSession;

/// Android-specific OS handle. Injected by PlatformBuilder at startup.
/// Only exists when compiling for Android — `#[cfg(target_os = "android")]`.
#[cfg(target_os = "android")]
pub struct AndroidHandle {
    /// JNI environment pointer (JNIEnv*).
    pub jni_env: *mut std::ffi::c_void,
    /// The Android Activity (jobject).
    pub activity: *mut std::ffi::c_void,
    /// The WebView instance (jobject).
    pub webview: *mut std::ffi::c_void,
}

#[cfg(target_os = "android")]
impl AndroidHandle {
    /// Create a handle from raw JNI pointers. Called by PlatformBuilder
    /// during Android app startup.
    #[must_use]
    pub fn new(
        jni_env: *mut std::ffi::c_void,
        activity: *mut std::ffi::c_void,
        webview: *mut std::ffi::c_void,
    ) -> Self {
        Self {
            jni_env,
            activity,
            webview,
        }
    }
}

/// iOS-specific OS handle. Injected by PlatformBuilder at startup.
/// Only exists when compiling for iOS — `#[cfg(target_os = "ios")]`.
#[cfg(target_os = "ios")]
pub struct IosHandle {
    /// The root UIViewController.
    pub view_controller: *mut std::ffi::c_void,
    /// The UIWindow.
    pub window: *mut std::ffi::c_void,
    /// The WKWebView instance.
    pub webview: *mut std::ffi::c_void,
}

#[cfg(target_os = "ios")]
impl IosHandle {
    /// Create a handle from raw pointers. Called by PlatformBuilder
    /// during iOS app startup.
    #[must_use]
    pub fn new(
        view_controller: *mut std::ffi::c_void,
        window: *mut std::ffi::c_void,
        webview: *mut std::ffi::c_void,
    ) -> Self {
        Self {
            view_controller,
            window,
            webview,
        }
    }
}

/// Desktop handle. Always available on non-mobile targets.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub struct DesktopHandle {
    /// The main window label in Tauri.
    pub window_label: String,
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
impl DesktopHandle {
    #[must_use]
    pub fn new(window_label: &str) -> Self {
        Self {
            window_label: window_label.to_string(),
        }
    }
}

// ── IpcInvokeContext — who called this IPC? ───────────────────────────────

/// Passed to every IPC handler at invoke time. Carries the calling page
/// identity, the WebView label for scoping emits, and the session.
#[derive(Clone)]
pub struct IpcInvokeContext {
    /// Which page invoked this IPC.
    pub page: PageIdentity,
    /// The WebView label for scoping emits and stream delivery.
    pub webview_label: String,
}

// ── Platform dispatch traits ──────────────────────────────────────────────

/// An IPC handler with an Android native implementation.
/// Only compilable on Android — the handle type doesn't exist otherwise.
#[cfg(target_os = "android")]
pub trait AndroidIpc: Ipc<Vec<u8>, Vec<u8>> {
    /// Invoke this IPC on Android with the platform handle.
    /// Called by the session's two-tier dispatch when a native handler
    /// is registered for this IPC name.
    fn invoke_android(
        &self,
        ctx: &IpcInvokeContext,
        session: &PlatformSession,
        handle: &AndroidHandle,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError>;
}

/// An IPC handler with an iOS native implementation.
/// Only compilable on iOS — the handle type doesn't exist otherwise.
#[cfg(target_os = "ios")]
pub trait IosIpc: Ipc<Vec<u8>, Vec<u8>> {
    /// Invoke this IPC on iOS with the platform handle.
    fn invoke_ios(
        &self,
        ctx: &IpcInvokeContext,
        session: &PlatformSession,
        handle: &IosHandle,
        request: &IpcRequest<Vec<u8>>,
    ) -> Result<IpcResponse<Vec<u8>>, IpcError>;
}

// ── CapabilityHandleRegistry ──────────────────────────────────────────────

/// Maps opaque u64 tokens → native OS resources.
/// Same pattern as `StreamRegistry` (F28) — WASM gets a token, native owns
/// the real thing. Used by handlers like camera (token → `CameraSession`)
/// or file picker (token → `PendingFileResult`).
pub struct CapabilityHandleRegistry {
    next_id: Mutex<u64>,
    handles: Mutex<BTreeMap<u64, Box<dyn Any + Send>>>,
}

impl CapabilityHandleRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: Mutex::new(1),
            handles: Mutex::new(BTreeMap::new()),
        }
    }

    /// Insert a native resource and return its opaque token.
    /// The token is returned to WASM, which passes it back on subsequent
    /// calls to operate on the same resource.
    pub fn insert<T: Send + 'static>(&self, resource: T) -> u64 {
        let mut id_guard = self.next_id.lock().unwrap();
        let id = *id_guard;
        *id_guard = id_guard.wrapping_add(1);
        drop(id_guard);
        self.handles.lock().unwrap().insert(id, Box::new(resource));
        id
    }

    /// Apply a function to a resource by token. Returns the function's result
    /// if the token exists and the type matches. Re-inserts the resource
    /// after the function runs.
    ///
    /// This avoids the `MutexGuard` lifetime problem — the resource is
    /// temporarily removed, operated on, and re-inserted.
    pub fn with<T: Send + 'static, R>(&self, id: u64, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        let mut guard = self.handles.lock().unwrap();
        let mut resource = guard.remove(&id)?;
        let result = resource.downcast_mut::<T>().map(f);
        guard.insert(id, resource);
        result
    }

    /// Remove a resource by token. Returns the resource if found.
    pub fn remove(&self, id: u64) -> Option<Box<dyn Any + Send>> {
        self.handles.lock().unwrap().remove(&id)
    }

    /// Check if a token is still valid.
    pub fn contains(&self, id: u64) -> bool {
        self.handles.lock().unwrap().contains_key(&id)
    }
}

impl Default for CapabilityHandleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── HostStreamRegistry ────────────────────────────────────────────────────

/// Handler-owned streams delivered to WASM.
///
/// The handler creates a stream via `create()`, writes chunks to the queue,
/// and returns the stream ID in the IPC response. WASM polls via the
/// `host_ipc_stream_read` import.
///
/// Uses `ConcurrentQueue` — same as `StreamRegistry` (F28), lock-free,
/// wasm32-safe.
pub struct HostStreamRegistry {
    next_id: Mutex<u64>,
    streams: Mutex<BTreeMap<u64, HostStreamQueue>>,
}

/// A queue of chunks from the host to WASM.
/// Handler writes chunks; WASM polls and drains.
pub type HostStreamQueue =
    std::sync::Arc<Mutex<std::collections::VecDeque<Result<Vec<u8>, IpcError>>>>;

impl HostStreamRegistry {
    /// Create an empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            next_id: Mutex::new(1),
            streams: Mutex::new(BTreeMap::new()),
        }
    }

    /// Create a stream and return (stream_id, queue).
    /// The handler retains the queue handle and writes chunks to it.
    /// WASM receives the stream_id in the IPC response and polls.
    pub fn create(&self) -> (u64, HostStreamQueue) {
        let mut id_guard = self.next_id.lock().unwrap();
        let id = *id_guard;
        *id_guard = id_guard.wrapping_add(1);
        drop(id_guard);

        let queue = Arc::new(Mutex::new(VecDeque::new()));
        self.streams.lock().unwrap().insert(id, queue.clone());
        (id, queue)
    }

    /// Read the next chunk from a stream. Returns `None` if the stream
    /// doesn't exist or has no more chunks. Callers re-poll.
    pub fn read(&self, stream_id: u64) -> Option<Result<Vec<u8>, IpcError>> {
        let streams = self.streams.lock().unwrap();
        let queue = streams.get(&stream_id).cloned()?;
        drop(streams);
        let mut guard = queue.lock().unwrap();
        let chunk = guard.pop_front();
        drop(guard);
        chunk
    }

    /// Write a chunk to a stream. Used by the handler.
    pub fn write(&self, stream_id: u64, chunk: Result<Vec<u8>, IpcError>) -> bool {
        let streams = self.streams.lock().unwrap();
        match streams.get(&stream_id) {
            Some(queue) => {
                queue.lock().unwrap().push_back(chunk);
                true
            }
            None => false,
        }
    }

    /// Close a stream and remove it from the registry.
    /// Returns false if the stream was already closed or doesn't exist.
    pub fn close(&self, stream_id: u64) -> bool {
        self.streams.lock().unwrap().remove(&stream_id).is_some()
    }

    /// Number of active streams.
    pub fn active_count(&self) -> usize {
        self.streams.lock().unwrap().len()
    }
}

impl Default for HostStreamRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_registry_insert_with_remove() {
        let reg = CapabilityHandleRegistry::new();
        let token = reg.insert("hello".to_string());

        // with() operates on the resource
        let result = reg.with::<String, _>(token, |s| {
            assert_eq!(s, "hello");
            *s = "updated".to_string();
            42
        });
        assert_eq!(result, Some(42));

        // Verify the update persisted
        let result2 = reg.with::<String, _>(token, |s| s.clone());
        assert_eq!(result2, Some("updated".to_string()));

        // Remove and verify gone
        let removed = reg.remove(token).unwrap();
        assert_eq!(removed.downcast_ref::<String>().unwrap(), "updated");
        assert!(reg.with::<String, ()>(token, |_| ()).is_none());
    }

    #[test]
    fn handle_registry_type_mismatch() {
        let reg = CapabilityHandleRegistry::new();
        let token = reg.insert(42u64);
        // Trying to access as wrong type returns None
        assert!(reg.with::<String, ()>(token, |_| ()).is_none());
        // Correct type works
        let result = reg.with::<u64, _>(token, |v| *v);
        assert_eq!(result, Some(42));
    }

    #[test]
    fn handle_registry_monotonic_ids() {
        let reg = CapabilityHandleRegistry::new();
        let t1 = reg.insert(1u32);
        let t2 = reg.insert(2u32);
        let t3 = reg.insert(3u32);
        assert!(t1 < t2);
        assert!(t2 < t3);
    }

    #[test]
    fn host_stream_create_read_close() {
        let reg = HostStreamRegistry::new();
        let (id, queue) = reg.create();

        // Handler writes chunks
        queue.lock().unwrap().push_back(Ok(b"chunk1".to_vec()));
        queue.lock().unwrap().push_back(Ok(b"chunk2".to_vec()));

        // WASM side reads
        assert_eq!(reg.read(id).unwrap().unwrap(), b"chunk1");
        assert_eq!(reg.read(id).unwrap().unwrap(), b"chunk2");
        assert!(reg.read(id).is_none()); // queue empty

        // Close and verify
        assert!(reg.close(id));
        assert!(!reg.close(id)); // already removed
    }

    #[test]
    fn host_stream_error_propagation() {
        let reg = HostStreamRegistry::new();
        let (id, queue) = reg.create();

        queue
            .lock()
            .unwrap()
            .push_back(Err(IpcError::ExecutionFailed));

        match reg.read(id).unwrap() {
            Err(IpcError::ExecutionFailed) => {}
            other => panic!("expected ExecutionFailed, got {other:?}"),
        }
    }

    #[test]
    fn host_stream_write_via_registry() {
        let reg = HostStreamRegistry::new();
        let (id, _queue) = reg.create();

        // Write through registry helper
        assert!(reg.write(id, Ok(b"hello".to_vec())));
        assert_eq!(reg.read(id).unwrap().unwrap(), b"hello");
    }

    #[test]
    fn host_stream_nonexistent() {
        let reg = HostStreamRegistry::new();
        assert!(reg.read(999).is_none());
        assert!(!reg.close(999));
        assert!(!reg.write(999, Ok(vec![])));
    }
}
