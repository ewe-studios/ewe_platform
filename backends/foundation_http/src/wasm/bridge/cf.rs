//! Cloudflare Workers wasm-bindgen bridge: `Request` + `env` → dispatch → `Response`.

use std::sync::{Arc, Mutex, Weak};

use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};
use js_sys;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, Response};

use crate::shared::app::HttpApp;
use crate::shared::context::ContextBag;
use crate::wasm::dispatch::HttpAppCfDispatch;
use crate::wasm::serve_cf::CfServe;
use foundation_db::wasm::bindgen::cf::{D1Database, KVNamespace, R2Bucket};

/// Convert a `Request` to a `SimpleIncomingRequest`.
async fn request_from_cf(req: &Request) -> Result<SimpleIncomingRequest, JsError> {
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

    let text_promise = req.text().map_err(|e| {
        JsError::new(&format!("failed to get request text: {e:?}"))
    })?;
    let body_js = JsFuture::from(text_promise).await.map_err(|e| {
        JsError::new(&format!("failed to read request body: {e:?}"))
    })?;
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

/// Extract typed bindings from the CF env object and store them in the context bag.
fn extract_cf_bindings(env: &JsValue, bag: &ContextBag) {
    if let Ok(db) = D1Database::from_env(env, "DB") {
        bag.store(db);
    }
}

/// Cloudflare Workers `CfHttpApp` wrapper.
#[wasm_bindgen]
pub struct CfHttpApp {
    inner: Arc<HttpApp<Arc<dyn CfServe>>>,
}

#[wasm_bindgen]
impl CfHttpApp {
    /// Create a new empty `CfHttpApp`.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(HttpApp::new_cf()),
        }
    }

    /// Dispatch a `Request` with CF `env` bindings through the app.
    #[wasm_bindgen(js_name = handleRequest)]
    pub async fn handle_request(
        &self,
        req: Request,
        env: JsValue,
    ) -> Result<Response, JsError> {
        let simple_req = request_from_cf(&req).await?;
        let bag = Arc::new(ContextBag::new());
        extract_cf_bindings(&env, &bag);
        Ok(self.inner.dispatch_cf(bag, simple_req).await.map_err(|e| {
            JsError::new(&format!("dispatch failed: {e:?}"))
        })?)
    }
}

impl CfHttpApp {
    /// Create a `CfHttpApp` from an existing `HttpApp<Arc<dyn CfServe>>`.
    pub fn from_app(app: HttpApp<Arc<dyn CfServe>>) -> Self {
        Self {
            inner: Arc::new(app),
        }
    }

    /// Dispatch a `SimpleIncomingRequest` through the app with the given context bag.
    pub async fn dispatch(
        &self,
        bag: Arc<ContextBag>,
        req: SimpleIncomingRequest,
    ) -> Result<Response, foundation_errstacks::ErrorTrace<crate::shared::serve::ServeError>> {
        self.inner.dispatch_cf(bag, req).await
    }

    /// Get the inner `HttpApp` reference for Rust-side route registration.
    pub fn app(&self) -> &HttpApp<Arc<dyn CfServe>> {
        &self.inner
    }
}

impl Default for CfHttpApp {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Singleton ─────────────────────────────────────────────────────

/// Internal app state shared between all guards.
struct AppState {
    app: Arc<CfHttpApp>,
    /// Kept alive so bindings stored during init survive for the app lifetime.
    #[allow(dead_code)]
    bag: Arc<ContextBag>,
}

// Safety: wasm32 is single-threaded with no shared memory.
// `dyn CfServe` is not `Send` by default, but on wasm32 there is
// only one thread, so it is safe to mark `AppState` as `Send + Sync`.
unsafe impl Send for AppState {}
unsafe impl Sync for AppState {}

/// Lifecycle token for the CF HTTP app.
///
/// Holds an `Arc<AppState>` so the app stays alive as long as any guard
/// instance exists. When all guards drop, `AppState` drops naturally.
pub struct CfHttpAppGuard {
    state: Arc<AppState>,
}

impl CfHttpAppGuard {
    /// Dispatch a request with CF env bindings through the app.
    pub async fn fetch(&self, req: Request, env: JsValue) -> Result<Response, JsError> {
        self.state.app.handle_request(req, env).await
    }

    /// Get the underlying `CfHttpApp` for direct access.
    pub fn app(&self) -> &CfHttpApp {
        &self.state.app
    }
}

impl Clone for CfHttpAppGuard {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

/// Static tracking of app existence. Never owns the app — only holds a Weak.
static STATE: Mutex<Weak<AppState>> = Mutex::new(Weak::new());

/// Zero-sized namespace for methods that manage the `STATE` static.
///
/// The app is initialized exactly once per isolate lifecycle. When all
/// `CfHttpAppGuard` instances drop, the app is reclaimed and can be
/// re-initialized.
pub struct CfHttpAppSingleton;

impl CfHttpAppSingleton {
    /// Get a guard to the singleton app.
    ///
    /// Panics if neither `get_or_init` nor `get_or_init_with_env` was called.
    pub fn get_app() -> CfHttpAppGuard {
        let weak = STATE.lock().unwrap();
        let state = weak
            .upgrade()
            .expect("CfHttpApp not initialized — call CfHttpAppSingleton::get_or_init() or get_or_init_with_env() first");
        CfHttpAppGuard { state }
    }

    /// Initialize the app if not already initialized.
    ///
    /// The closure receives an `Arc<ContextBag>` so you can store typed
    /// bindings (D1, R2, KV, secrets) before building the `CfHttpApp`.
    ///
    /// If already initialized, returns the existing guard and ignores the closure.
    pub fn get_or_init<F: FnOnce(Arc<ContextBag>) -> CfHttpApp>(builder: F) -> CfHttpAppGuard {
        let mut weak = STATE.lock().unwrap();
        if let Some(arc) = weak.upgrade() {
            return CfHttpAppGuard { state: arc };
        }
        let bag = Arc::new(ContextBag::new());
        let app = Arc::new(builder(bag.clone()));
        let state = Arc::new(AppState { app, bag });
        *weak = Arc::downgrade(&state);
        CfHttpAppGuard { state }
    }

    /// Initialize the app from a raw CF env `JsValue`.
    ///
    /// Auto-extracts D1/R2/KV bindings from `env` by convention:
    /// - `DB` → `D1Database`
    /// - `BUCKET` → `R2Bucket`
    /// - `KV` → `KVNamespace`
    ///
    /// The closure can store additional bindings before building the app.
    /// If already initialized, returns the existing guard and ignores the closure.
    pub fn get_or_init_with_env<F: FnOnce(Arc<ContextBag>) -> CfHttpApp>(
        env: &JsValue,
        builder: F,
    ) -> CfHttpAppGuard {
        let mut weak = STATE.lock().unwrap();
        if let Some(arc) = weak.upgrade() {
            return CfHttpAppGuard { state: arc };
        }
        let bag = Arc::new(ContextBag::new());
        if let Ok(db) = D1Database::from_env(env, "DB") {
            bag.store(db);
        }
        if let Ok(bucket) = R2Bucket::from_env(env, "BUCKET") {
            bag.store(bucket);
        }
        if let Ok(kv) = KVNamespace::from_env(env, "KV") {
            bag.store(kv);
        }
        let app = Arc::new(builder(bag.clone()));
        let state = Arc::new(AppState { app, bag });
        *weak = Arc::downgrade(&state);
        CfHttpAppGuard { state }
    }

    /// Check if the app has been initialized.
    pub fn is_initialized() -> bool {
        STATE.lock().unwrap().upgrade().is_some()
    }

    /// Force-reset the singleton state. cfg-gated to `#[cfg(test)]`.
    #[cfg(test)]
    pub fn reset() {
        let mut weak = STATE.lock().unwrap();
        *weak = Weak::new();
    }
}
