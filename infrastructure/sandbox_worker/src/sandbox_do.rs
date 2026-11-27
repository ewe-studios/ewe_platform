//! The Durable Object (DO) of one sandbox.
//!
//! The DO holds a `DoControlPlane` with both drivers of feature 07. `fetch`
//! gives each request to the control plane and maps the streamed response back
//! to the Worker. `alarm` stops an idle sandbox.

use std::str::FromStr;
use std::sync::Arc;

use foundation_compact::SendWrapper;
use foundation_sandbox::shared::cloudflare::{
    CloudflareConfig, CloudflareHost, DoControlPlane, PortError, PortRequest, PortResponse,
    SandboxIndex, ServiceBinding,
};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::ids::SandboxId;
use foundation_sandbox::shared::BoxByteStream;
use foundation_sandbox::shared::BoxFuture;
use futures::lock::Mutex;
use futures::{Stream, StreamExt};

use crate::index_do::WorkerSandboxIndex;
use crate::{
    connect_error_response, ctx, limits, read_bounded_body, simple_request,
    worker_error_from_sandbox, worker_response, SANDBOX_ID_HEADER,
};

/// The Durable Object of one sandbox.
#[worker::durable_object]
pub struct SandboxDo {
    state: worker::State,
    env: worker::Env,
    control: Mutex<Option<Arc<DoControlPlane>>>,
}

impl worker::DurableObject for SandboxDo {
    fn new(state: worker::State, env: worker::Env) -> Self {
        Self {
            state,
            env,
            control: Mutex::new(None),
        }
    }

    async fn fetch(&self, mut request: worker::Request) -> worker::Result<worker::Response> {
        let header = request.headers().get(SANDBOX_ID_HEADER)?;
        let id = match header.as_deref().map(SandboxId::from_str) {
            Some(Ok(id)) => id,
            Some(Err(error)) => return connect_error_response(&request, &[], error),
            None => {
                return connect_error_response(
                    &request,
                    &[],
                    SandboxError::InvalidSpec {
                        field: "sandbox_id".to_string(),
                        reason: format!("the {SANDBOX_ID_HEADER} header is absent"),
                    },
                )
            }
        };
        let limit = limits().max_message_bytes;
        let body = match read_bounded_body(&mut request, limit).await {
            Ok(body) => body,
            Err(error) => {
                return connect_error_response(&request, &[], error.current_context().clone())
            }
        };
        let parts = simple_request(&request, &body)?;
        let Some(control) = self.control(Some(id)).await? else {
            return connect_error_response(
                &request,
                &[],
                SandboxError::NotFound {
                    what: "sandbox".to_string(),
                },
            );
        };
        worker_response(control.handle(parts).await)
    }

    async fn alarm(&self) -> worker::Result<worker::Response> {
        let Some(control) = self.control(None).await? else {
            return worker::Response::empty();
        };
        control.on_alarm().await;
        worker::Response::empty()
    }
}

impl SandboxDo {
    /// The one control plane of this object, built on the first request.
    ///
    /// `id` is `Some` on a request and `None` on an alarm. On an alarm with no
    /// stored identifier there is nothing to do, so the result is `None`.
    async fn control(&self, id: Option<SandboxId>) -> worker::Result<Option<Arc<DoControlPlane>>> {
        let mut cell = self.control.lock().await;
        if let Some(control) = cell.as_ref() {
            return Ok(Some(Arc::clone(control)));
        }
        let index: Arc<dyn SandboxIndex> = Arc::new(WorkerSandboxIndex::new(self.env.clone()));
        let name = id
            .as_ref()
            .map_or_else(|| "sandbox".to_string(), ToString::to_string);
        // The model variables are read here, so a model spec reaches the model
        // driver (issue #1142).
        let model = crate::model_client::model_config_from_env(&self.env);
        let host = CloudflareHost::from_state(
            &self.state,
            index,
            CloudflareConfig::for_sandbox(name, model),
        )
        .map_err(worker_error_from_sandbox)?
        .with_service(
            Arc::new(WorkerServiceBinding::new(self.env.clone())) as Arc<dyn ServiceBinding>
        );
        let opened = DoControlPlane::open(ctx(), host, id, limits())
            .await
            .map_err(worker_error_from_sandbox)?;
        let Some(control) = opened else {
            return Ok(None);
        };
        let control = Arc::new(control);
        *cell = Some(Arc::clone(&control));
        Ok(Some(control))
    }
}

/// [`ServiceBinding`] over the `SLAVE` Durable Object binding.
///
/// The model slave of decision D16 lives in a second Durable Object with the
/// sandbox identifier as its name.
pub struct WorkerServiceBinding(SendWrapper<worker::Env>);

impl WorkerServiceBinding {
    /// A binding over the Worker environment.
    #[must_use]
    pub fn new(env: worker::Env) -> Self {
        Self(SendWrapper::new(env))
    }
}

impl ServiceBinding for WorkerServiceBinding {
    fn fetch(
        &self,
        binding: &str,
        name: &SandboxId,
        request: PortRequest,
    ) -> BoxFuture<'_, Result<PortResponse, PortError>> {
        let binding = binding.to_string();
        let name = name.clone();
        Box::pin(SendWrapper::new(async move {
            let namespace = self
                .0
                .durable_object(&binding)
                .map_err(|error| PortError::Other(error.to_string()))?;
            let stub = namespace
                .get_by_name(name.as_str())
                .map_err(|error| PortError::Other(error.to_string()))?;
            let headers = worker::Headers::new();
            for (header, value) in &request.headers {
                headers
                    .append(header, value)
                    .map_err(|error| PortError::Other(error.to_string()))?;
            }
            let init = worker::RequestInit::new();
            let mut init = init;
            init.with_method(worker::Method::from(request.method.clone()));
            init.with_headers(headers);
            if !request.body.is_empty() {
                let body = worker::js_sys::Uint8Array::from(request.body.as_slice());
                init.with_body(Some(worker::wasm_bindgen::JsValue::from(body)));
            }
            let url = format!("https://slave.rpc{}", request.path);
            let forwarded = worker::Request::new_with_init(&url, &init)
                .map_err(|error| PortError::Other(error.to_string()))?;
            let mut response = stub
                .fetch_with_request(forwarded)
                .await
                .map_err(|error| PortError::Other(error.to_string()))?;
            let status = response.status_code();
            let headers = response.headers().entries().collect::<Vec<_>>();
            let stream = response
                .stream()
                .map_err(|error| PortError::Other(error.to_string()))?;
            let body = stream.map(|item| {
                item.map_err(|error| {
                    SandboxError::Io {
                        op: "slave binding".to_string(),
                        message: error.to_string(),
                    }
                    .into()
                })
            });
            let body: BoxByteStream = Box::pin(SendStream(SendWrapper::new(body)));
            Ok(PortResponse {
                status,
                headers,
                body,
            })
        }))
    }
}

/// A futures stream whose JavaScript value asserts `Send` on wasm.
struct SendStream<S>(SendWrapper<S>);

impl<S: Stream> Stream for SendStream<S> {
    type Item = S::Item;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        // SAFETY: the wrapper is boxed and pinned, so the inner stream never
        // moves after this point.
        let this = unsafe { self.get_unchecked_mut() };
        let inner = unsafe { std::pin::Pin::new_unchecked(&mut *this.0) };
        inner.poll_next(context)
    }
}
