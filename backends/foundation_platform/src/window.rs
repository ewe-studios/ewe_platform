//! Window management — Tauri WebView lifecycle (F35).
//!
//! WHY: `WebViewPool` tracks labels but doesn't create real windows. This
//! module provides the bridge: a `WindowOps` trait the session calls, and
//! a Tauri-backed implementation injected by `PlatformBuilder`.
//!
//! WHAT: `WindowOps` trait with create, navigate, show, hide, screenshot,
//! and close methods. `ensure_webview()` checks the pool and creates windows
//! lazily. The session calls these through the pool during navigation.
//!
//! HOW: The session holds `Option<Box<dyn WindowOps>>`. In tests it's `None`
//! (pool operates without real windows). In production, `PlatformBuilder`
//! sets a Tauri `AppHandle`-backed implementation.

use std::sync::Mutex;

use crate::stack::{WebViewPool, WebViewState};

// ── WindowOps trait ──────────────────────────────────────────────────────

/// Operations the session needs from the native window system.
/// In production this wraps Tauri's `AppHandle`; in tests it's absent.
pub trait WindowOps: Send + Sync + 'static {
    /// Create a new WebView window with the given label and initial URL.
    fn create(&self, label: &str, url: &str);
    /// Navigate an existing window to a new URL.
    fn navigate(&self, label: &str, url: &str);
    /// Show a window (make visible, bring to front).
    fn show(&self, label: &str);
    /// Hide a window (keep alive, not visible).
    fn hide(&self, label: &str);
    /// Close/destroy a window permanently.
    fn close(&self, label: &str);
    /// Capture a screenshot of the window as PNG bytes.
    fn screenshot(&self, label: &str) -> Vec<u8>;
    /// Evaluate JavaScript in the window.
    fn eval(&self, label: &str, script: &str);
    /// Open a URL in the system browser (external navigation).
    fn open_url(&self, url: &str);
}

// ── Tauri-backed implementation ──────────────────────────────────────────

#[cfg(not(target_family = "wasm"))]
mod tauri_impl {
    use tauri::Manager;
    use super::*;

    /// A `WindowOps` backed by Tauri's `AppHandle`.
    /// Implements the trait by calling Tauri's WebView window API.
    pub struct TauriWindowOps<R: tauri::Runtime = tauri::Wry> {
        app_handle: tauri::AppHandle<R>,
    }

    impl<R: tauri::Runtime> TauriWindowOps<R> {
        pub fn new(app_handle: tauri::AppHandle<R>) -> Self {
            Self { app_handle }
        }

        pub fn into_boxed(self) -> Box<dyn WindowOps> {
            Box::new(self)
        }
    }

    impl<R: tauri::Runtime> WindowOps for TauriWindowOps<R> {
        fn create(&self, label: &str, url: &str) {
            if let Some(window) = self.app_handle.get_webview_window(label) {
                // Window already exists — navigate + return
                let _ = window.eval(&format!("location.href = '{url}'"));
                return;
            }
            // Create a new window
            let builder = tauri::WebviewWindowBuilder::new(
                &self.app_handle,
                label,
                tauri::WebviewUrl::App(url.into()),
            )
            .visible(false)
            .title(format!("ewe — {label}"));

            if let Ok(window) = builder.build() {
                // Wait briefly then show
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    let _ = window.show();
                });
            }
        }

        fn navigate(&self, label: &str, url: &str) {
            if let Some(window) = self.app_handle.get_webview_window(label) {
                let _ = window.eval(&format!("location.href = '{url}'"));
            }
        }

        fn show(&self, label: &str) {
            if let Some(window) = self.app_handle.get_webview_window(label) {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }

        fn hide(&self, label: &str) {
            if let Some(window) = self.app_handle.get_webview_window(label) {
                let _ = window.hide();
            }
        }

        fn close(&self, label: &str) {
            if let Some(window) = self.app_handle.get_webview_window(label) {
                let _ = window.close();
            }
        }

        fn screenshot(&self, _label: &str) -> Vec<u8> {
            // Tauri 2.4+ has capture_screenshot() — use eval(html2canvas) as fallback
            Vec::new()
        }

        fn eval(&self, label: &str, script: &str) {
            if let Some(window) = self.app_handle.get_webview_window(label) {
                let _ = window.eval(script);
            }
        }

        fn open_url(&self, url: &str) {
            // `window.open(url, '_system')` tells the WebView to delegate to
            // the OS. On Android this triggers an Intent.ACTION_VIEW that
            // the system browser handles; on desktop it opens the default
            // browser. More reliable than `_blank` which may open in-app.
            if let Some(window) = self.app_handle.get_webview_window("main") {
                let _ = window.eval(&format!("window.open('{url}','_system')"));
            }
        }
    }
}

#[cfg(not(target_family = "wasm"))]
pub use tauri_impl::TauriWindowOps;

// ── Noop implementation (for tests) ──────────────────────────────────────

pub struct NoopWindowOps;

impl WindowOps for NoopWindowOps {
    fn create(&self, _label: &str, _url: &str) {}
    fn navigate(&self, _label: &str, _url: &str) {}
    fn show(&self, _label: &str) {}
    fn hide(&self, _label: &str) {}
    fn close(&self, _label: &str) {}
    fn screenshot(&self, _label: &str) -> Vec<u8> { Vec::new() }
    fn eval(&self, _label: &str, _script: &str) {}
    fn open_url(&self, _url: &str) {}
}

// ── Window manager — high-level operations over WindowOps + Pool ──────────

/// Coordinates `WindowOps` with `WebViewPool`. Callers use these methods
/// instead of raw trait calls — they update pool state alongside windows.
pub struct WindowManager {
    ops: Mutex<Option<Box<dyn WindowOps>>>,
}

impl WindowManager {
    #[must_use]
    pub fn new() -> Self {
        Self { ops: Mutex::new(None) }
    }

    /// Inject the concrete window backend (called by PlatformBuilder at build time).
    pub fn set_ops(&self, ops: Box<dyn WindowOps>) {
        *self.ops.lock().unwrap() = Some(ops);
    }

    /// Ensure a WebView window exists for the given label and is navigating to url.
    /// If the window doesn't exist, creates it lazily. If it exists, navigates.
    pub fn ensure(&self, pool: &mut WebViewPool, label: &str, url: &str) {
        let ops_guard = self.ops.lock().unwrap();
        let ops = match ops_guard.as_ref() {
            Some(o) => o,
            None => return,
        };

        if pool.get(label).is_none() {
            ops.create(label, url);
            pool.get_or_create(label);
            pool.set_state(label, WebViewState::Loading);
            pool.set_route(label, url);
        } else {
            ops.navigate(label, url);
            pool.set_route(label, url);
        }
    }

    /// Show a window and update its pool state to Active.
    pub fn activate(&self, pool: &mut WebViewPool, label: &str) {
        let ops_guard = self.ops.lock().unwrap();
        let ops = match ops_guard.as_ref() {
            Some(o) => o,
            None => return,
        };
        ops.show(label);
        pool.set_state(label, WebViewState::Active);
    }

    /// Hide a window and mark its pool state as Ready.
    pub fn deactivate(&self, pool: &mut WebViewPool, label: &str) {
        let ops_guard = self.ops.lock().unwrap();
        let ops = match ops_guard.as_ref() {
            Some(o) => o,
            None => return,
        };
        ops.hide(label);
        pool.set_state(label, WebViewState::Ready);
    }

    /// Close a window and remove it from the pool.
    pub fn destroy(&self, pool: &mut WebViewPool, label: &str) {
        let ops_guard = self.ops.lock().unwrap();
        let ops = match ops_guard.as_ref() {
            Some(o) => o,
            None => return,
        };
        ops.close(label);
        pool.remove(label);
    }

    /// Navigate an existing window to a new URL.
    pub fn navigate(&self, pool: &mut WebViewPool, label: &str, url: &str) {
        let ops_guard = self.ops.lock().unwrap();
        let ops = match ops_guard.as_ref() {
            Some(o) => o,
            None => return,
        };
        ops.navigate(label, url);
        pool.set_route(label, url);
    }

    /// Open a URL in the system browser via Tauri shell (external navigation).
    pub fn open_external(&self, _pool: &mut WebViewPool, url: &str) {
        let ops_guard = self.ops.lock().unwrap();
        if let Some(ops) = ops_guard.as_ref() {
            ops.open_url(url);
        }
    }

    /// Capture a screenshot of a window as PNG bytes.
    pub fn screenshot(&self, _pool: &mut WebViewPool, label: &str) -> Vec<u8> {
        let ops_guard = self.ops.lock().unwrap();
        if let Some(ops) = ops_guard.as_ref() {
            ops.screenshot(label)
        } else {
            Vec::new()
        }
    }

    /// Check if window ops are available (false in tests).
    #[must_use]
    pub fn is_available(&self) -> bool {
        self.ops.lock().unwrap().is_some()
    }
}

impl Default for WindowManager {
    fn default() -> Self { Self::new() }
}

