//! Web-standard wasm-bindgen bridge: `web_sys::Request` → `SimpleIncomingRequest` → dispatch → `web_sys::Response`.

use std::sync::{Arc, Mutex, Weak};

use foundation_context::Ctx;
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, Response};

use crate::shared::app::HttpApp;
use crate::shared::serve_web::WebServe;
use crate::wasm::dispatch::HttpAppWebDispatch;

/// Convert a `web_sys::Request` to a `SimpleIncomingRequest`.
async fn request_from_web(req: &Request) -> Result<SimpleIncomingRequest, JsError> {
    let method = SimpleMethod::from(req.method());
    let url = req.url();

    let headers = req.headers();
    let entries = js_sys::Array::from(&headers.entries());
    let mut simple_headers = SimpleHeaders::new();
    for entry in entries.iter() {
        let pair = js_sys::Array::from(&entry);
        if let (Some(key), Some(val)) = (pair.get(0).as_string(), pair.get(1).as_string()) {
            let header = SimpleHeader::from(key.clone());
            simple_headers.entry(header).or_default().push(val);
        }
    }

    let text_promise = req
        .text()
        .map_err(|e| JsError::new(&format!("failed to get request text: {e:?}")))?;
    let body_js = JsFuture::from(text_promise)
        .await
        .map_err(|e| JsError::new(&format!("failed to read request body: {e:?}")))?;
    let body_text = body_js.as_string().unwrap_or_default();
    let body = if body_text.is_empty() {
        None
    } else {
        Some(SendSafeBody::Text(body_text))
    };

    let simple_req = SimpleIncomingRequest::builder()
        .with_parsed_url(url)
        .with_method(method)
        .with_proto(Proto::HTTP11)
        .with_headers(simple_headers)
        .with_some_body(body)
        .build()
        .map_err(|e| JsError::new(&format!("failed to build request: {e}")))?;

    Ok(simple_req)
}

/// JS-compatible wrapper around `HttpApp`.
#[wasm_bindgen]
pub struct WasmHttpApp {
    inner: Arc<HttpApp<Arc<dyn WebServe>>>,
    /// The app's context. Each request forks it. The singleton sets it to
    /// its own context, so init-time bindings survive every request.
    app_ctx: Ctx,
}

#[wasm_bindgen]
impl WasmHttpApp {
    /// Create a new empty `WasmHttpApp`.
    ///
    /// This is a host entry point: it uses the real host clock. A
    /// deterministic host uses [`WasmHttpApp::with_ctx`].
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::with_ctx(Ctx::host())
    }

    /// Dispatch a `web_sys::Request` through the app and return a `web_sys::Response`.
    ///
    /// Each request forks the app's context. The bindings stored at init, the
    /// injected clock and the RNG stream reach the handler. Inserts made
    /// during a request stay private to that request.
    #[wasm_bindgen(js_name = handleRequest)]
    pub async fn handle_request(&self, req: Request) -> Result<Response, JsError> {
        let simple_req = request_from_web(&req).await?;
        let app_ctx = self.app_ctx.fork();
        Ok(self
            .inner
            .dispatch_web(app_ctx, simple_req)
            .await
            .map_err(|e| JsError::new(&format!("dispatch failed: {e:?}")))?)
    }
}

impl WasmHttpApp {
    /// Create an empty `WasmHttpApp` from an explicit valtron [`Ctx`].
    ///
    /// This is the deterministic-test entry point: the app and every request
    /// it dispatches read the injected clock and RNG stream.
    pub fn with_ctx(ctx: Ctx) -> Self {
        let app_ctx = ctx.with_empty_map();
        Self {
            inner: Arc::new(HttpApp::with_context(app_ctx.clone())),
            app_ctx,
        }
    }

    /// Get the inner `HttpApp` reference for Rust-side route registration.
    pub fn app(&self) -> &HttpApp<Arc<dyn WebServe>> {
        &self.inner
    }

    /// Create a `WasmHttpApp` from an existing `HttpApp<Arc<dyn WebServe>>`.
    ///
    /// The wrapper shares the app's own context.
    pub fn from_app(app: HttpApp<Arc<dyn WebServe>>) -> Self {
        let app_ctx = app.context().clone();
        Self {
            inner: Arc::new(app),
            app_ctx,
        }
    }
}

impl Default for WasmHttpApp {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Singleton ─────────────────────────────────────────────────────

struct AppState {
    app: Arc<WasmHttpApp>,
}

// Safety: wasm32 is single-threaded with no shared memory.
// `dyn WebServe` is not `Send` by default, but on wasm32 there is
// only one thread, so it is safe to mark `AppState` as `Send + Sync`.
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

/// Lifecycle token for the web HTTP app.
pub struct WasmHttpAppGuard {
    state: Arc<AppState>,
}

impl WasmHttpAppGuard {
    /// Dispatch a request through the web app.
    pub async fn fetch(&self, req: Request) -> Result<Response, JsError> {
        self.state.app.handle_request(req).await
    }

    /// Get the underlying `WasmHttpApp` for direct access.
    pub fn app(&self) -> &WasmHttpApp {
        &self.state.app
    }
}

impl Clone for WasmHttpAppGuard {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

/// Static tracking of app existence. Never owns the app — only holds a Weak.
static STATE: Mutex<Weak<AppState>> = Mutex::new(Weak::new());

/// Zero-sized namespace for methods that manage the `STATE` static.
pub struct WasmHttpAppSingleton;

impl WasmHttpAppSingleton {
    /// Get a guard to the singleton app.
    ///
    /// Panics if `get_or_init` was not called.
    pub fn get_app() -> WasmHttpAppGuard {
        let weak = STATE.lock().unwrap();
        let state = weak
            .upgrade()
            .expect("WasmHttpApp not initialized — call WasmHttpAppSingleton::get_or_init() first");
        WasmHttpAppGuard { state }
    }

    /// Initialize the app if not already initialized.
    ///
    /// The closure receives a `Ctx`. Store typed bindings in that
    /// context before you build the `HttpApp`. The app uses that context for every
    /// request.
    ///
    /// If already initialized, returns the existing guard and ignores the closure.
    ///
    /// This is a host entry point: it uses the real host clock. A deterministic
    /// host uses [`WasmHttpAppSingleton::get_or_init_with_ctx`].
    pub fn get_or_init<F: FnOnce(Ctx) -> HttpApp<Arc<dyn WebServe>>>(
        builder: F,
    ) -> WasmHttpAppGuard {
        Self::get_or_init_with_ctx(Ctx::host(), builder)
    }

    /// Initialize the app from an explicit valtron [`Ctx`] if not initialized.
    ///
    /// The closure receives a `Ctx` built from `ctx`. Store typed
    /// bindings in that context. Build the app with `HttpApp::with_context`, so
    /// route factories also see the bindings. The singleton gives the context to
    /// the app that the closure returns, so the bindings, the clock and the
    /// RNG reach every request.
    ///
    /// If already initialized, returns the existing guard and ignores `ctx`
    /// and the closure.
    pub fn get_or_init_with_ctx<F: FnOnce(Ctx) -> HttpApp<Arc<dyn WebServe>>>(
        ctx: Ctx,
        builder: F,
    ) -> WasmHttpAppGuard {
        let mut weak = STATE.lock().unwrap();
        if let Some(arc) = weak.upgrade() {
            return WasmHttpAppGuard { state: arc };
        }
        let app_ctx = ctx.with_empty_map();
        let mut app = builder(app_ctx.clone());
        app.ctx = app_ctx.clone();
        let app = Arc::new(WasmHttpApp::from_app(app));
        let state = Arc::new(AppState { app });
        *weak = Arc::downgrade(&state);
        WasmHttpAppGuard { state }
    }

    /// Check if the app has been initialized.
    pub fn is_initialized() -> bool {
        STATE.lock().unwrap().upgrade().is_some()
    }
}
