//! The Cloudflare Worker of the foundation sandbox platform (spec 77 feature 08).
//!
//! The entry checks the bearer token, reads the request body under the message
//! cap, routes the request, and forwards it to the Durable Object (DO) of the
//! sandbox. It returns the DO response unchanged, so an `Exec` frame reaches
//! the caller before the command ends.
//!
//! The crate holds no sandbox logic: the state checks, the retries and the
//! container calls are in `foundation_sandbox`.

mod index_do;
mod model_client;
mod model_slave_do;
mod sandbox_do;

use std::sync::OnceLock;

use foundation_connectrpc::ErrorWriter;
use foundation_context::env::{Timer, TimerFuture};
use foundation_context::Ctx;
use foundation_core::valtron::{initialize_pool, PoolGuard};
use foundation_errstacks::ErrorTrace;
use foundation_netio::shared::http::{
    SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
    SimpleOutgoingResponse,
};
use foundation_sandbox::shared::cloudflare::HttpResponseParts;
#[cfg(feature = "entry")]
use foundation_sandbox::shared::cloudflare::{route, RouteTarget};
#[cfg(feature = "entry")]
use foundation_sandbox::shared::control::authorize;
#[cfg(feature = "entry")]
use foundation_sandbox::shared::driver::{Capabilities, HostCheck, HostReport};
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
#[cfg(feature = "entry")]
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::limits::SandboxLimits;
#[cfg(feature = "entry")]
use foundation_sandbox::shared::rpc::messages::ProbeResponse;
#[cfg(feature = "entry")]
use foundation_sandbox::shared::spec::RootMode;
#[cfg(feature = "entry")]
use foundation_sandbox::shared::Secret;
use futures::StreamExt;
use worker::wasm_bindgen::{JsCast, JsValue};
#[cfg(feature = "entry")]
use worker::Env;
use worker::{Request, Response};

pub use index_do::{SandboxIndexDo, WorkerSandboxIndex};
pub use model_slave_do::ModelSlaveDo;
pub use sandbox_do::{SandboxDo, WorkerServiceBinding};

/// The binding of the sandbox Durable Object.
pub const SANDBOX_BINDING: &str = "SANDBOX";
/// The binding of the index Durable Object.
pub const INDEX_BINDING: &str = "SANDBOX_INDEX";
/// The binding of the model slave Durable Object (decision D16).
pub const SLAVE_BINDING: &str = "SLAVE";
/// The Worker secret that holds the bearer token.
pub const SANDBOX_TOKEN_SECRET: &str = "SANDBOX_TOKEN";
/// The header that the entry sends with the sandbox identifier.
pub const SANDBOX_ID_HEADER: &str = "x-ewe-sandbox-id";
/// The internal path prefix of the index entries.
pub const INDEX_PREFIX: &str = "/index/";
/// The `List` path that the index Durable Object answers.
pub const LIST_PATH: &str = "/ewe.sandbox.control.v1.SandboxService/List";

static CTX: OnceLock<Ctx> = OnceLock::new();
static POOL: OnceLock<PoolGuard> = OnceLock::new();

/// The `Ctx` of this isolate.
///
/// The entry builds the effect bundle. The control plane waits on its timer for
/// a boot deadline, a stop grace and a stream limit, so a Worker with no timer
/// cannot create or stop a sandbox. Without these waits a create fails with
/// `HostNotConfigured`.
///
/// # Panics
/// When the timer or the entropy source of the bundle does not build. A Worker
/// cannot work without the bundle, so the entry stops on the first request.
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
///
/// Why: a wait on the executor's wasm timer drives the valtron engine from
/// inside the poll. A task that waits inside another engine task then re-enters
/// it (issue #1125). A host timer wakes the task from the JavaScript event loop,
/// so the engine is never driven from inside a poll.
struct JsTimer;

impl Timer for JsTimer {
    fn sleep_until(&self, ctx: Ctx, deadline: foundation_compact::Instant) -> TimerFuture {
        let wait = deadline.saturating_duration_since(ctx.clock().now());
        Box::pin(foundation_compact::SendWrapper::new(async move {
            let global = worker::js_sys::global();
            let promise = worker::js_sys::Promise::new(&mut |resolve, _reject| {
                let Some(set_timeout) =
                    worker::js_sys::Reflect::get(&global, &JsValue::from_str("setTimeout"))
                        .ok()
                        .and_then(|value| value.dyn_into::<worker::js_sys::Function>().ok())
                else {
                    // No host timer: complete at once rather than hang.
                    let _ = resolve.call0(&JsValue::UNDEFINED);
                    return;
                };
                let millis = u32::try_from(wait.as_millis())
                    .unwrap_or(u32::MAX)
                    .min(u32::try_from(i32::MAX).unwrap_or(u32::MAX));
                let _ = set_timeout.call2(
                    &global,
                    &resolve,
                    &JsValue::from_f64(f64::from(millis)),
                );
            });
            let _ = worker::js_sys::futures::JsFuture::from(promise).await;
        }))
    }
}

/// The limits of this Worker.
pub(crate) fn limits() -> SandboxLimits {
    SandboxLimits::default()
}

/// One worker error as a `worker` error.
pub(crate) fn worker_error_from_sandbox(error: impl std::fmt::Display) -> worker::Error {
    worker::Error::RustError(error.to_string())
}

#[cfg(feature = "entry")]
/// The entry of the Worker.
///
/// Steps 1 to 5 of feature 08, section 6.2.
///
/// # Errors
/// A `worker` error when the platform cannot build a response.
#[worker::event(fetch)]
pub async fn fetch(
    mut request: Request,
    env: Env,
    _context: worker::Context,
) -> worker::Result<Response> {
    console_error_panic_hook::set_once();
    let _ = ctx();

    // Step 2: read the body under the message cap.
    let limit = limits().max_message_bytes;
    let body = match read_bounded_body(&mut request, limit).await {
        Ok(body) => body,
        Err(error) => {
            return connect_error_response(&request, &[], error.current_context().clone())
        }
    };

    // Step 1: the bearer token, before any Durable Object is reached.
    let token = match env.secret(SANDBOX_TOKEN_SECRET) {
        Ok(token) => Secret::new(token.to_string()),
        Err(error) => {
            return connect_error_response(
                &request,
                &body,
                SandboxError::Io {
                    op: "worker secret".to_string(),
                    message: error.to_string(),
                },
            )
        }
    };
    let header = request.headers().get("authorization")?;
    if let Err(error) = authorize(header.as_deref(), &token) {
        return connect_error_response(&request, &body, error.current_context().clone());
    }

    // Step 3: route.
    let target = match route(&request.path(), &body) {
        Ok(target) => target,
        Err(error) => {
            return connect_error_response(&request, &body, error.current_context().clone())
        }
    };

    // Step 4: the target.
    match target {
        RouteTarget::Probe => probe_response(),
        RouteTarget::List => forward(&env, INDEX_BINDING, "index", &request, &body, None).await,
        RouteTarget::NewSandbox => {
            let id = SandboxId::generate(&ctx());
            forward(
                &env,
                SANDBOX_BINDING,
                id.as_str(),
                &request,
                &body,
                Some(&id),
            )
            .await
        }
        RouteTarget::Sandbox(id) | RouteTarget::Container(id) => {
            // Step 5: forward and return the response as it is. A container
            // call reaches the same Durable Object as a scoped sandbox call.
            let response = forward(
                &env,
                SANDBOX_BINDING,
                id.as_str(),
                &request,
                &body,
                Some(&id),
            )
            .await?;
            if response.status_code() == 404 {
                // A scoped call to a sandbox whose DO has no record removes the
                // stale index entry (invariant I6).
                remove_index_entry(&env, &id).await;
                return Ok(response);
            }
            // A stream error after the head also means that the sandbox is
            // gone, so the entry is removed then too.
            stream_response(response, env, id)
        }
    }
}

#[cfg(feature = "entry")]
/// Removes one index entry and logs a failure. A failure does not fail the call.
async fn remove_index_entry(env: &Env, id: &SandboxId) {
    let index = WorkerSandboxIndex::new(env.clone());
    if let Err(error) =
        foundation_sandbox::shared::cloudflare::SandboxIndex::remove(&index, id).await
    {
        worker::console_warn!("the stale index entry was not removed: {error}");
    }
}

#[cfg(feature = "entry")]
/// Returns the Durable Object response with a body that removes the index
/// entry when the stream fails after the head.
fn stream_response(mut response: Response, env: Env, id: SandboxId) -> worker::Result<Response> {
    let status = response.status_code();
    let headers = response.headers().clone();
    let body = response.stream()?;
    let stream = body.then(move |item| {
        let env = env.clone();
        let id = id.clone();
        async move {
            if item.is_err() {
                remove_index_entry(&env, &id).await;
            }
            item
        }
    });
    Ok(Response::from_stream(stream)?
        .with_status(status)
        .with_headers(headers))
}

/// Reads the request body, at most `max` bytes.
///
/// # Errors
/// [`SandboxError::InvalidSpec`] when the body is larger than `max`, and
/// [`SandboxError::Io`] when the stream fails.
pub(crate) async fn read_bounded_body(request: &mut Request, max: usize) -> SandboxResult<Vec<u8>> {
    let mut stream = request.stream().map_err(|error| SandboxError::Io {
        op: "request body".to_string(),
        message: error.to_string(),
    })?;
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| SandboxError::Io {
            op: "request body".to_string(),
            message: error.to_string(),
        })?;
        if body.len().saturating_add(chunk.len()) > max {
            return Err(SandboxError::InvalidSpec {
                field: "body".to_string(),
                reason: format!("the body is larger than {max} bytes"),
            }
            .into());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// The request as the shared HTTP type.
pub(crate) fn simple_request(
    request: &Request,
    body: &[u8],
) -> worker::Result<SimpleIncomingRequest> {
    let mut headers = SimpleHeaders::new();
    for (name, value) in request.headers().entries() {
        headers
            .entry(SimpleHeader::from(name))
            .or_default()
            .push(value);
    }
    // The router looks up `request_url.url` as a path, not as an absolute URL.
    // An absolute URL here makes every call miss the route table and answer an
    // empty 404 (issue #988).
    SimpleIncomingRequest::builder()
        .with_plain_url(request.path())
        .with_method(SimpleMethod::from(request.method().to_string().as_str()))
        .with_headers(headers)
        .with_body_bytes(body.to_vec())
        .build()
        .map_err(worker_error_from_sandbox)
}

/// The shared response as a Worker response whose body streams.
pub(crate) fn worker_response(parts: HttpResponseParts) -> worker::Result<Response> {
    let body = parts
        .body
        .map(|item| item.map_err(|error| worker::Error::RustError(error.to_string())));
    let headers = worker::Headers::new();
    for (name, value) in &parts.headers {
        headers.append(name, value)?;
    }
    let response = Response::from_stream(body)?;
    Ok(response.with_status(parts.status).with_headers(headers))
}

/// The shared error as a Connect error response.
pub(crate) fn connect_error_response(
    request: &Request,
    body: &[u8],
    error: SandboxError,
) -> worker::Result<Response> {
    let parts = simple_request(request, body)?;
    let mut response = SimpleOutgoingResponse::empty();
    ErrorWriter::new()
        .write(
            &mut response,
            &parts,
            &ErrorTrace::new(error.to_connect(limits().max_error_message)),
        )
        .map_err(worker_error_from_sandbox)?;
    outgoing_to_worker(response)
}

/// One whole shared response as a Worker response.
fn outgoing_to_worker(response: SimpleOutgoingResponse) -> worker::Result<Response> {
    let status = u16::try_from(response.status.clone().into_usize()).unwrap_or(500);
    let body = match response.body {
        Some(SendSafeBody::Bytes(bytes)) => bytes,
        Some(SendSafeBody::Text(text)) => text.into_bytes(),
        _ => Vec::new(),
    };
    let headers = worker::Headers::new();
    for (name, values) in response.headers {
        for value in values {
            headers.append(&name.to_string(), &value)?;
        }
    }
    let response = Response::from_bytes(body)?;
    Ok(response.with_status(status).with_headers(headers))
}

#[cfg(feature = "entry")]
/// The probe answer of the Worker entry.
fn probe_response() -> worker::Result<Response> {
    let capabilities = Capabilities {
        rootfs_container_image: true,
        root_modes: vec![RootMode::Overlay],
        host_mounts: false,
        network_none: true,
        network_host: true,
        max_vcpus: 1,
        ..Capabilities::default()
    };
    let report = HostReport {
        driver: DriverId::CloudflareContainer,
        usable: true,
        checks: vec![
            HostCheck {
                name: "container_binding".to_string(),
                ok: true,
                message: "the Durable Object has a container".to_string(),
            },
            HostCheck {
                name: "worker_version".to_string(),
                ok: true,
                message: foundation_sandbox::VERSION.to_string(),
            },
        ],
        capabilities,
    };
    Response::from_json(&ProbeResponse {
        reports: vec![report],
    })
}

#[cfg(feature = "entry")]
/// Forwards one request to a Durable Object and returns its response.
async fn forward(
    env: &Env,
    binding: &str,
    name: &str,
    request: &Request,
    body: &[u8],
    sandbox_id: Option<&SandboxId>,
) -> worker::Result<Response> {
    let stub = env.durable_object(binding)?.get_by_name(name)?;
    let headers = request.headers().clone();
    if let Some(id) = sandbox_id {
        headers.set(SANDBOX_ID_HEADER, id.as_str())?;
    }
    let init = worker::RequestInit::new();
    let mut init = init;
    init.with_method(request.method());
    init.with_headers(headers);
    if !body.is_empty() {
        let array = worker::js_sys::Uint8Array::from(body);
        init.with_body(Some(worker::wasm_bindgen::JsValue::from(array)));
    }
    let url = format!("https://sandbox.rpc{}", request.path());
    let forwarded = Request::new_with_init(&url, &init)?;
    stub.fetch_with_request(forwarded).await
}
