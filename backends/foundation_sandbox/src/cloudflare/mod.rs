//! WebAssembly adapters for a Cloudflare Worker.
//!
//! This is the only module of feature 07 that names a `worker` type. Every
//! JavaScript value and every `worker` future crosses into the `Send + Sync`
//! traits inside [`foundation_compact::SendWrapper`]. The assertion is sound on
//! single-threaded wasm.

use std::sync::Arc;

use foundation_compact::SendWrapper;
use foundation_context::Ctx;
use futures::{Stream, StreamExt};
use worker::js_sys::futures::JsFuture;
use worker::wasm_bindgen::JsValue;

use crate::shared::cloudflare::{
    port_error_from_text, CloudflareConfig, CloudflareHost, ContainerApi, ContainerStart, DoAlarm,
    DoStorage, ModelBackend, ModelClient, ModelRequest, SandboxIndex, ServiceBinding,
};
use crate::shared::cloudflare::{PortError, PortRequest, PortResponse};
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::{BoxByteStream, BoxFuture};

/// A futures stream whose `worker` value asserts `Send` on wasm.
struct WasmSendStream<S>(SendWrapper<S>);

impl<S> Stream for WasmSendStream<S>
where
    S: Stream,
{
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

/// One worker error as a sandbox error.
fn worker_error(error: worker::Error) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::Io {
        op: "cloudflare".to_string(),
        message: error.to_string(),
    }
    .into()
}

/// One JavaScript error as a sandbox error.
fn js_error(error: JsValue) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::Io {
        op: "cloudflare".to_string(),
        message: format!("{error:?}"),
    }
    .into()
}

/// `ContainerApi` over the Durable Object's `worker::Container`.
pub struct WorkerContainer {
    container: SendWrapper<worker::Container>,
    max_error_message: usize,
}

impl WorkerContainer {
    /// The adapter over one worker container.
    #[must_use]
    pub fn new(container: worker::Container) -> Self {
        Self::new_with_max_error_message(
            container,
            crate::shared::limits::SandboxLimits::default().max_error_message,
        )
    }

    /// The adapter with the cap on one port error text.
    #[must_use]
    pub fn new_with_max_error_message(container: worker::Container, max: usize) -> Self {
        Self {
            container: SendWrapper::new(container),
            max_error_message: max,
        }
    }
}

impl ContainerApi for WorkerContainer {
    fn running(&self) -> bool {
        self.container.running()
    }

    fn start(&self, options: ContainerStart) -> SandboxResult<()> {
        let mut startup = worker::ContainerStartupOptions::new();
        let entrypoint: Vec<&str> = options.entrypoint.iter().map(String::as_str).collect();
        startup.set_entrypoint(&entrypoint);
        startup.enable_internet(options.enable_internet);
        for (key, value) in &options.env {
            startup.add_env(key, value);
        }
        self.container.start(Some(startup)).map_err(|error| {
            SandboxError::SubstrateApi {
                status: 0,
                message: error.to_string(),
            }
            .into()
        })
    }

    fn destroy(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(SendWrapper::new(async move {
            self.container.destroy(None).await.map_err(worker_error)
        }))
    }

    fn signal(&self, signo: i32) -> SandboxResult<()> {
        self.container.signal(signo).map_err(worker_error)
    }

    fn wait_for_exit(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(SendWrapper::new(async move {
            self.container.wait_for_exit().await.map_err(worker_error)
        }))
    }

    fn fetch(
        &self,
        port: u16,
        request: PortRequest,
    ) -> BoxFuture<'_, Result<PortResponse, PortError>> {
        Box::pin(SendWrapper::new(async move {
            let fetcher = self
                .container
                .get_tcp_port(port)
                .map_err(|error| PortError::Other(error.to_string()))?;
            let headers = worker::Headers::new();
            for (name, value) in &request.headers {
                headers
                    .append(name, value)
                    .map_err(|error| PortError::Other(error.to_string()))?;
            }
            let init = worker::RequestInit::new();
            let mut init = init;
            init.with_method(worker::Method::from(request.method));
            init.with_headers(headers);
            if !request.body.is_empty() {
                let body = worker::js_sys::Uint8Array::from(request.body.as_slice());
                init.with_body(Some(JsValue::from(body)));
            }
            let path = if request.path.starts_with('/') {
                format!("http://container.local{}", request.path)
            } else {
                format!("http://container.local/{}", request.path)
            };
            let worker_request = worker::Request::new_with_init(&path, &init)
                .map_err(|error| PortError::Other(error.to_string()))?;
            let mut response = fetcher
                .fetch_request(worker_request)
                .await
                .map_err(|error| {
                    port_error_from_text(&error.to_string(), self.max_error_message)
                })?;
            let status = response.status_code();
            let headers = response.headers().entries().collect::<Vec<_>>();
            let stream = response.stream().map_err(|error| {
                port_error_from_text(&error.to_string(), self.max_error_message)
            })?;
            let body = stream.map(|item| {
                item.map_err(|error| {
                    SandboxError::Io {
                        op: "container fetch".to_string(),
                        message: error.to_string(),
                    }
                    .into()
                })
            });
            let body: BoxByteStream = Box::pin(WasmSendStream(SendWrapper::new(body)));
            Ok(PortResponse {
                status,
                headers,
                body,
            })
        }))
    }
}

/// `DoStorage` over the Durable Object's `worker::Storage`.
pub struct WorkerStorage(SendWrapper<worker::Storage>);

impl WorkerStorage {
    /// The adapter over one worker storage.
    #[must_use]
    pub fn new(storage: worker::Storage) -> Self {
        Self(SendWrapper::new(storage))
    }
}

impl DoStorage for WorkerStorage {
    fn get(&self, key: &str) -> BoxFuture<'_, SandboxResult<Option<Vec<u8>>>> {
        let key = key.to_string();
        Box::pin(SendWrapper::new(async move {
            self.0.get::<Vec<u8>>(&key).await.map_err(worker_error)
        }))
    }

    fn put(&self, key: &str, value: Vec<u8>) -> BoxFuture<'_, SandboxResult<()>> {
        let key = key.to_string();
        Box::pin(SendWrapper::new(async move {
            self.0.put(&key, value).await.map_err(worker_error)
        }))
    }

    fn delete(&self, key: &str) -> BoxFuture<'_, SandboxResult<()>> {
        let key = key.to_string();
        Box::pin(SendWrapper::new(async move {
            self.0
                .delete(&key)
                .await
                .map(|_removed| ())
                .map_err(worker_error)
        }))
    }
}

/// `DoAlarm` over the Durable Object's one alarm.
pub struct WorkerAlarm(SendWrapper<worker::worker_sys::DurableObjectStorage>);

impl WorkerAlarm {
    /// The adapter over one Durable Object storage.
    #[must_use]
    pub fn new(storage: worker::worker_sys::DurableObjectStorage) -> Self {
        Self(SendWrapper::new(storage))
    }
}

impl DoAlarm for WorkerAlarm {
    fn set(&self, at_unix_ms: u64) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(SendWrapper::new(async move {
            let date = worker::js_sys::Date::new(&JsValue::from_f64(at_unix_ms as f64));
            let promise = self
                .0
                .set_alarm(date, worker::js_sys::Object::new())
                .map_err(js_error)?;
            JsFuture::from(promise)
                .await
                .map(|_value| ())
                .map_err(js_error)
        }))
    }

    fn clear(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(SendWrapper::new(async move {
            let promise = self
                .0
                .delete_alarm(worker::js_sys::Object::new())
                .map_err(js_error)?;
            JsFuture::from(promise)
                .await
                .map(|_value| ())
                .map_err(js_error)
        }))
    }
}

impl CloudflareHost {
    /// Builds the host bundle of one Durable Object.
    ///
    /// # Errors
    /// [`SandboxError::HostNotConfigured`] when the Durable Object has no
    /// container, or the error of the worker storage call.
    pub fn from_state(
        state: &worker::State,
        index: Arc<dyn SandboxIndex>,
        config: CloudflareConfig,
    ) -> SandboxResult<Self> {
        let container = state.container().ok_or_else(|| {
            foundation_errstacks::ErrorTrace::new(SandboxError::HostNotConfigured)
        })?;
        let storage = state.storage();
        let raw_storage = state.as_raw().storage().map_err(js_error)?;
        Ok(Self {
            container: Arc::new(WorkerContainer::new_with_max_error_message(
                container,
                config.max_error_message,
            )),
            storage: Arc::new(WorkerStorage::new(storage)),
            alarm: Arc::new(WorkerAlarm::new(raw_storage)),
            index,
            service: None,
            config: Arc::new(config),
        })
    }

    /// The same host with the service binding of the model slave.
    #[must_use]
    pub fn with_service(mut self, service: Arc<dyn ServiceBinding>) -> Self {
        self.service = Some(service);
        self
    }
}

/// The model client over the Worker's `OpenRouter` fetch or the `AI` binding.
pub struct WorkerModelClient {
    env: SendWrapper<worker::Env>,
    ai_binding: String,
}

impl WorkerModelClient {
    /// A client over one Worker environment.
    #[must_use]
    pub fn new(env: worker::Env) -> Self {
        Self {
            env: SendWrapper::new(env),
            ai_binding: "AI".to_string(),
        }
    }

    /// The same client with another `AI` binding name.
    #[must_use]
    pub fn with_ai_binding(mut self, binding: impl Into<String>) -> Self {
        self.ai_binding = binding.into();
        self
    }
}

impl ModelClient for WorkerModelClient {
    fn complete(&self, _ctx: Ctx, request: ModelRequest) -> BoxFuture<'_, SandboxResult<Vec<u8>>> {
        Box::pin(SendWrapper::new(async move {
            match request.backend {
                ModelBackend::CloudflareAi { model } => {
                    let ai = self.env.ai(&self.ai_binding).map_err(worker_error)?;
                    let input = serde_json::json!({ "prompt": request.prompt });
                    let output: serde_json::Value =
                        ai.run(&model, input).await.map_err(worker_error)?;
                    let text = output
                        .get("response")
                        .and_then(serde_json::Value::as_str)
                        .map_or_else(|| output.to_string(), str::to_string);
                    Ok(text.into_bytes())
                }
                ModelBackend::OpenRouter { base_url, model } => {
                    let Some(key) = request.api_key else {
                        return Err(foundation_errstacks::ErrorTrace::new(
                            SandboxError::HostNotConfigured,
                        ));
                    };
                    let headers = worker::Headers::new();
                    headers
                        .set("content-type", "application/json")
                        .map_err(worker_error)?;
                    headers
                        .set("authorization", &format!("Bearer {}", key.expose()))
                        .map_err(worker_error)?;
                    let body = serde_json::json!({
                        "model": model,
                        "messages": [{ "role": "user", "content": request.prompt }],
                    });
                    let init = worker::RequestInit::new();
                    let mut init = init;
                    init.with_method(worker::Method::Post);
                    init.with_headers(headers);
                    init.with_body(Some(JsValue::from_str(&body.to_string())));
                    let url = format!("{}/chat/completions", base_url.trim_end_matches('/'));
                    let worker_request =
                        worker::Request::new_with_init(&url, &init).map_err(worker_error)?;
                    let mut response = worker::Fetch::Request(worker_request)
                        .send()
                        .await
                        .map_err(worker_error)?;
                    let value: serde_json::Value = response.json().await.map_err(worker_error)?;
                    let text = value
                        .get("choices")
                        .and_then(|choices| choices.get(0))
                        .and_then(|choice| choice.get("message"))
                        .and_then(|message| message.get("content"))
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default();
                    Ok(text.as_bytes().to_vec())
                }
            }
        }))
    }
}
