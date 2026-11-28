//! The one Durable Object (DO) named `index` that holds the Worker-wide list.
//!
//! The keys are `sbx:<id>` with the JSON `SandboxInfo`. It serves three
//! internal paths: `PUT /index/<id>`, `DELETE /index/<id>` and the `List` remote
//! procedure call (RPC) path. The truth is each sandbox DO's own record; this
//! DO is a view.

use std::str::FromStr;

use foundation_compact::SendWrapper;
use foundation_sandbox::shared::cloudflare::SandboxIndex;
use foundation_sandbox::shared::control::SandboxInfo;
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_sandbox::shared::ids::SandboxId;
use foundation_sandbox::shared::rpc::messages::ListResponse;
use foundation_sandbox::shared::BoxFuture;

use crate::{connect_error_response, limits, read_bounded_body, INDEX_PREFIX, LIST_PATH};

/// One worker error of the index call as a sandbox error.
fn index_error(error: worker::Error) -> foundation_errstacks::ErrorTrace<SandboxError> {
    SandboxError::Io {
        op: "index".to_string(),
        message: error.to_string(),
    }
    .into()
}

/// The one index Durable Object.
#[worker::durable_object]
pub struct SandboxIndexDo {
    state: worker::State,
}

impl worker::DurableObject for SandboxIndexDo {
    fn new(state: worker::State, _env: worker::Env) -> Self {
        Self { state }
    }

    async fn fetch(&self, mut request: worker::Request) -> worker::Result<worker::Response> {
        let path = request.path();
        let limit = limits().max_message_bytes;
        if let Some(tail) = path.strip_prefix(INDEX_PREFIX) {
            let id = match SandboxId::from_str(tail) {
                Ok(id) => id,
                Err(error) => return connect_error_response(&request, &[], error),
            };
            return match request.method() {
                worker::Method::Put => {
                    let body = match read_bounded_body(&mut request, limit).await {
                        Ok(body) => body,
                        Err(error) => {
                            return connect_error_response(
                                &request,
                                &[],
                                error.current_context().clone(),
                            )
                        }
                    };
                    let info = match serde_json::from_slice::<SandboxInfo>(&body) {
                        Ok(info) => info,
                        Err(error) => {
                            return connect_error_response(
                                &request,
                                &[],
                                SandboxError::InvalidSpec {
                                    field: "sandbox_info".to_string(),
                                    reason: error.to_string(),
                                },
                            )
                        }
                    };
                    if let Err(error) = self.state.storage().put(&format!("sbx:{id}"), &info).await
                    {
                        return Err(worker::Error::from(error.to_string()));
                    }
                    worker::Response::empty()
                }
                worker::Method::Delete => {
                    if let Err(error) = self.state.storage().delete(&format!("sbx:{id}")).await {
                        return Err(worker::Error::from(error.to_string()));
                    }
                    worker::Response::empty()
                }
                _ => connect_error_response(
                    &request,
                    &[],
                    SandboxError::NotFound {
                        what: "method".to_string(),
                    },
                ),
            };
        }
        if path == LIST_PATH {
            let response = self.list().await?;
            return worker::Response::from_json(&response);
        }
        connect_error_response(
            &request,
            &[],
            SandboxError::NotFound {
                what: "method".to_string(),
            },
        )
    }

    async fn alarm(&self) -> worker::Result<worker::Response> {
        worker::Response::empty()
    }
}

impl SandboxIndexDo {
    /// Every entry under the `sbx:` prefix.
    async fn list(&self) -> worker::Result<ListResponse> {
        let storage = self.state.storage();
        let map = storage
            .list_with_options(worker::ListOptions::new().prefix("sbx:"))
            .await?;
        // `list_with_options` gives a `js_sys::Map`, not a plain object. A
        // plain `Object::values` sees none of its entries, so the list was
        // always empty. Read the Map's own values (issue #1125).
        let values = map.values();
        let mut sandboxes = Vec::new();
        loop {
            let next = values.next().map_err(|error| {
                worker::Error::from(format!("the index list failed: {error:?}"))
            })?;
            if next.done() {
                break;
            }
            let info: SandboxInfo = serde_wasm_bindgen::from_value(next.value())
                .map_err(|error| worker::Error::from(error.to_string()))?;
            sandboxes.push(info);
        }
        Ok(ListResponse { sandboxes })
    }
}

/// [`SandboxIndex`] over the index Durable Object.
///
/// The `CloudflareHost` of one sandbox DO calls it when the record changes.
pub struct WorkerSandboxIndex(SendWrapper<worker::Env>);

impl WorkerSandboxIndex {
    /// An index over the Worker environment.
    #[must_use]
    pub fn new(env: worker::Env) -> Self {
        Self(SendWrapper::new(env))
    }

    /// Sends one internal request to the index DO.
    fn send(
        &self,
        method: worker::Method,
        path: String,
        body: Option<Vec<u8>>,
    ) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(SendWrapper::new(async move {
            let namespace = self
                .0
                .durable_object(crate::INDEX_BINDING)
                .map_err(index_error)?;
            let stub = namespace.get_by_name("index").map_err(index_error)?;
            let init = worker::RequestInit::new();
            let mut init = init;
            init.with_method(method);
            if let Some(body) = body {
                let array = worker::js_sys::Uint8Array::from(body.as_slice());
                init.with_body(Some(worker::wasm_bindgen::JsValue::from(array)));
            }
            let url = format!("https://index.rpc{path}");
            let request = worker::Request::new_with_init(&url, &init).map_err(index_error)?;
            let response = stub
                .fetch_with_request(request)
                .await
                .map_err(index_error)?;
            let status = response.status_code();
            if (200..300).contains(&status) {
                Ok(())
            } else {
                Err(SandboxError::Io {
                    op: "index".to_string(),
                    message: format!("the index answered status {status}"),
                }
                .into())
            }
        }))
    }
}

impl SandboxIndex for WorkerSandboxIndex {
    fn put(&self, info: SandboxInfo) -> BoxFuture<'_, SandboxResult<()>> {
        let body = match serde_json::to_vec(&info) {
            Ok(body) => body,
            Err(error) => {
                return Box::pin(async move {
                    Err(SandboxError::Io {
                        op: "index".to_string(),
                        message: error.to_string(),
                    }
                    .into())
                })
            }
        };
        let path = format!("{INDEX_PREFIX}{}", info.id);
        self.send(worker::Method::Put, path, Some(body))
    }

    fn remove(&self, id: &SandboxId) -> BoxFuture<'_, SandboxResult<()>> {
        self.send(worker::Method::Delete, format!("{INDEX_PREFIX}{id}"), None)
    }
}
