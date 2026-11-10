//! Cloudflare Workers wasm-bindgen bridge: `Request` + `env` → dispatch → `Response`.

use std::sync::{Arc, Mutex, Weak};

use foundation_core::valtron::Ctx;
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};
use js_sys;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, Response};

use crate::shared::app::HttpApp;
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

/// Extract typed bindings from the CF env object and store them in the context.
fn extract_cf_bindings(env: &JsValue, app_ctx: &Ctx) {
    if let Ok(db) = D1Database::from_env(env, "DB") {
        app_ctx.store(db);
    }
}

/// Cloudflare Workers `CfHttpApp` wrapper.
#[wasm_bindgen]
pub struct CfHttpApp {
    inner: Arc<HttpApp<Arc<dyn CfServe>>>,
    /// The app's context. Each request forks it. The singleton sets it to
    /// its own context, so init-time bindings survive every request.
    app_ctx: Ctx,
}

#[wasm_bindgen]
impl CfHttpApp {
    /// Create a new empty `CfHttpApp`.
    ///
    /// This is a host entry point: it uses the real host clock. A
    /// deterministic host uses [`CfHttpApp::with_ctx`].
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::with_ctx(Ctx::host())
    }

    /// Dispatch a `Request` with CF `env` bindings through the app.
    ///
    /// Each request forks the app's context. The bindings stored at init, the
    /// injected clock and the RNG stream reach the handler. The `env` bindings
    /// and any inserts made during the request stay private to that request.
    #[wasm_bindgen(js_name = handleRequest)]
    pub async fn handle_request(&self, req: Request, env: JsValue) -> Result<Response, JsError> {
        let simple_req = request_from_cf(&req).await?;
        let app_ctx = self.app_ctx.fork();
        extract_cf_bindings(&env, &app_ctx);
        Ok(self
            .inner
            .dispatch_cf(app_ctx, simple_req)
            .await
            .map_err(|e| JsError::new(&format!("dispatch failed: {e:?}")))?)
    }
}

impl CfHttpApp {
    /// Create an empty `CfHttpApp` from an explicit valtron [`Ctx`].
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

    /// Create a `CfHttpApp` from an existing `HttpApp<Arc<dyn CfServe>>`.
    ///
    /// The wrapper shares the app's own context.
    pub fn from_app(app: HttpApp<Arc<dyn CfServe>>) -> Self {
        let app_ctx = app.context().clone();
        Self {
            inner: Arc::new(app),
            app_ctx,
        }
    }

    /// Dispatch a `SimpleIncomingRequest` through the app with the given context.
    pub async fn dispatch(
        &self,
        app_ctx: Ctx,
        req: SimpleIncomingRequest,
    ) -> Result<Response, foundation_errstacks::ErrorTrace<crate::shared::serve::ServeError>> {
        self.inner.dispatch_cf(app_ctx, req).await
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
    /// The closure receives a `Ctx` so you can store typed
    /// bindings (D1, R2, KV, secrets) before building the `CfHttpApp`. The app
    /// uses that context for every request.
    ///
    /// If already initialized, returns the existing guard and ignores the closure.
    ///
    /// This is a host entry point: it uses the real host clock. A deterministic
    /// host uses [`CfHttpAppSingleton::get_or_init_with_ctx`].
    pub fn get_or_init<F: FnOnce(Ctx) -> CfHttpApp>(builder: F) -> CfHttpAppGuard {
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
    pub fn get_or_init_with_ctx<F: FnOnce(Ctx) -> CfHttpApp>(
        ctx: Ctx,
        builder: F,
    ) -> CfHttpAppGuard {
        let mut weak = STATE.lock().unwrap();
        if let Some(arc) = weak.upgrade() {
            return CfHttpAppGuard { state: arc };
        }
        let app_ctx = ctx.with_empty_map();
        let mut app = builder(app_ctx.clone());
        app.app_ctx = app_ctx.clone();
        let app = Arc::new(app);
        let state = Arc::new(AppState { app });
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
    /// The closure can store additional bindings before building the app. The
    /// app uses the shared context for every request.
    /// If already initialized, returns the existing guard and ignores the closure.
    ///
    /// This is a host entry point: it uses the real host clock. A deterministic
    /// host uses [`CfHttpAppSingleton::get_or_init_with_env_and_ctx`].
    pub fn get_or_init_with_env<F: FnOnce(Ctx) -> CfHttpApp>(
        env: &JsValue,
        builder: F,
    ) -> CfHttpAppGuard {
        Self::get_or_init_with_env_and_ctx(env, Ctx::host(), builder)
    }

    /// Initialize the app from a raw CF env `JsValue` and an explicit valtron
    /// [`Ctx`].
    ///
    /// Same binding extraction as [`CfHttpAppSingleton::get_or_init_with_env`],
    /// but the context's clock and RNG come from `ctx`.
    ///
    /// If already initialized, returns the existing guard and ignores `ctx`
    /// and the closure.
    pub fn get_or_init_with_env_and_ctx<F: FnOnce(Ctx) -> CfHttpApp>(
        env: &JsValue,
        ctx: Ctx,
        builder: F,
    ) -> CfHttpAppGuard {
        let mut weak = STATE.lock().unwrap();
        if let Some(arc) = weak.upgrade() {
            return CfHttpAppGuard { state: arc };
        }
        let app_ctx = ctx.with_empty_map();
        if let Ok(db) = D1Database::from_env(env, "DB") {
            app_ctx.store(db);
        }
        if let Ok(bucket) = R2Bucket::from_env(env, "BUCKET") {
            app_ctx.store(bucket);
        }
        if let Ok(kv) = KVNamespace::from_env(env, "KV") {
            app_ctx.store(kv);
        }
        let mut app = builder(app_ctx.clone());
        app.app_ctx = app_ctx.clone();
        let app = Arc::new(app);
        let state = Arc::new(AppState { app });
        *weak = Arc::downgrade(&state);
        CfHttpAppGuard { state }
    }

    /// Check if the app has been initialized.
    pub fn is_initialized() -> bool {
        STATE.lock().unwrap().upgrade().is_some()
    }
}
