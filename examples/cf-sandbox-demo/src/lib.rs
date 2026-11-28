//! The Cloudflare demo of spec 92 feature 06.
//!
//! One Worker holds the sandbox Durable Object (DO) classes of
//! `infrastructure/sandbox_worker`, the container binding, the agent-rpc
//! registry with the OpenRouter path, the Workers AI binding and the model
//! server path of the sandbox container. The page and the live test call one
//! small control API under `/api/`.

mod agent;
mod api;
mod config;
mod page;
mod sandbox;

use std::sync::OnceLock;

use foundation_context::env::{Timer, TimerFuture};
use foundation_context::Ctx;
use foundation_core::valtron::{initialize_pool, PoolGuard};
use worker::wasm_bindgen::JsCast;
use worker::{Env, Request, Response};

use crate::config::DemoConfig;

// The Durable Object classes of the reused sandbox Worker. They are exported
// from this dependency, and `wrangler.toml` names them. This `pub use` keeps
// the dependency linked in a way the linker cannot drop.
pub use sandbox_worker::{ModelSlaveDo, SandboxDo, SandboxIndexDo};

static CTX: OnceLock<Ctx> = OnceLock::new();
static POOL: OnceLock<PoolGuard> = OnceLock::new();

/// The `Ctx` of this isolate.
///
/// The timer wakes a wait from the JavaScript event loop, so the valtron engine
/// is never driven from inside one of its own polls (issue #1125).
///
/// # Panics
/// When the effect environment of the Worker does not build. A Worker cannot
/// work without it, so the entry stops on the first request.
pub(crate) fn ctx() -> Ctx {
    CTX.get_or_init(|| {
        POOL.get_or_init(|| initialize_pool(42, None));
        let ctx = Ctx::host();
        let env = foundation_core::env::wasm(&ctx)
            .expect("the effect environment of the Worker builds")
            .with_timer(std::sync::Arc::new(JsTimer) as std::sync::Arc<dyn Timer>);
        ctx.store(env);
        ctx
    })
    .clone()
}

/// The timer of the Worker: one JavaScript `setTimeout`.
struct JsTimer;

impl Timer for JsTimer {
    fn sleep_until(&self, ctx: Ctx, deadline: foundation_compact::Instant) -> TimerFuture {
        let wait = deadline.saturating_duration_since(ctx.clock().now());
        Box::pin(foundation_compact::SendWrapper::new(async move {
            let global = worker::js_sys::global();
            let promise = worker::js_sys::Promise::new(&mut |resolve, _reject| {
                let Some(set_timeout) = worker::js_sys::Reflect::get(
                    &global,
                    &worker::wasm_bindgen::JsValue::from_str("setTimeout"),
                )
                .ok()
                .and_then(|value| value.dyn_into::<worker::js_sys::Function>().ok()) else {
                    // No host timer: complete at once rather than hang.
                    let _ = resolve.call0(&worker::wasm_bindgen::JsValue::UNDEFINED);
                    return;
                };
                let millis = u32::try_from(wait.as_millis())
                    .unwrap_or(u32::MAX)
                    .min(u32::try_from(i32::MAX).unwrap_or(u32::MAX));
                let _ = set_timeout.call2(
                    &global,
                    &resolve,
                    &worker::wasm_bindgen::JsValue::from_f64(f64::from(millis)),
                );
            });
            let _ = worker::js_sys::futures::JsFuture::from(promise).await;
        }))
    }
}

/// The entry of the Worker.
///
/// # Errors
/// A `worker` error when the platform cannot build a response.
#[worker::event(fetch)]
pub async fn fetch(
    request: Request,
    env: Env,
    _context: worker::Context,
) -> worker::Result<Response> {
    console_error_panic_hook::set_once();
    let ctx = ctx();
    let target = request.path();
    if request.method() == worker::Method::Get && (target == "/" || target == "/index.html") {
        return page::serve(request, env, ctx).await;
    }
    if target.starts_with("/api/") {
        let config = match DemoConfig::from_env(&env) {
            Ok(config) => config,
            Err(message) => return Response::error(message, 500),
        };
        return api::handle(request, env, &config).await;
    }
    Response::error("not found", 404)
}
