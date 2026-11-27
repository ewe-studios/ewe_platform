//! Connect actions through an injected JavaScript Fetch function.
use foundation_connectrpc::{ClientOptions, TransportError};
use foundation_netio::shared::http::{RequestDescriptor, SimpleHeaders, Status};
use foundation_ui_runtime::action::ActionTransport;
use foundation_ui_wire::{writer::ClientCaps, WireLimits};
use futures_channel::oneshot;
use futures_util::future::BoxFuture;
use js_sys::{Function, Promise, Uint8Array};
use std::{future::Future, sync::Arc};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use crate::{
    v1::ActionsClient, ConnectActionTransport, PlatformExchange, PlatformHttpClient,
    PlatformHttpTransport,
};

#[wasm_bindgen(module = "/src/fetch.js")]
extern "C" {
    #[wasm_bindgen(catch, js_name = registerSession)]
    fn register(fetcher: Function) -> Result<u32, JsValue>;
    #[wasm_bindgen(js_name = releaseSession)]
    fn release(id: u32);
    #[wasm_bindgen(catch, js_name = openExchange)]
    fn open_exchange(id: u32, method: &str, url: &str, headers: &str) -> Result<u32, JsValue>;
    #[wasm_bindgen(js_name = startExchange)]
    fn start_exchange(id: u32, body: &[u8]) -> Promise;
    #[wasm_bindgen(js_name = readExchange)]
    fn read_exchange(id: u32, limit: usize) -> Promise;
    #[wasm_bindgen(js_name = closeExchange)]
    fn close_exchange(id: u32);
}

fn error(err: JsValue) -> TransportError {
    TransportError::Protocol(js_sys::Error::from(err).message().into())
}

fn task<T: Send + 'static>(
    work: impl Future<Output = Result<T, TransportError>> + 'static,
) -> BoxFuture<'static, Result<T, TransportError>> {
    let (send, done) = oneshot::channel();
    spawn_local(async move {
        drop(send.send(work.await));
    });
    Box::pin(async move { done.await.map_err(|_| TransportError::Canceled)? })
}

/// Holds a shared Fetch session on the browser event loop.
pub struct FetchClient(u32);
impl FetchClient {
    /// Register a Fetch function for this browser session.
    ///
    /// # Errors
    ///
    /// Return a JavaScript error if no session identifiers remain.
    pub fn new(fetcher: Function) -> Result<Self, JsValue> {
        register(fetcher).map(Self)
    }
}
impl Drop for FetchClient {
    fn drop(&mut self) {
        release(self.0);
    }
}

impl PlatformHttpClient for FetchClient {
    fn exchange(
        &self,
        request: RequestDescriptor,
    ) -> Result<Arc<dyn PlatformExchange>, TransportError> {
        let headers: Vec<_> = request
            .headers
            .into_iter()
            .flat_map(|(key, values)| {
                values
                    .into_iter()
                    .map(move |value| (key.to_string(), value))
            })
            .collect();
        let json =
            serde_json::to_string(&headers).map_err(|e| TransportError::Protocol(e.to_string()))?;
        let exchange = open_exchange(
            self.0,
            &request.method.to_string(),
            &request.request_uri.to_string(),
            &json,
        )
        .map_err(error)?;
        Ok(Arc::new(FetchExchange(exchange)))
    }
}

struct FetchExchange(u32);
impl PlatformExchange for FetchExchange {
    fn start(
        &self,
        body: Vec<u8>,
    ) -> BoxFuture<'_, Result<(Status, SimpleHeaders), TransportError>> {
        let id = self.0;
        task(async move {
            let value = JsFuture::from(start_exchange(id, &body))
                .await
                .map_err(error)?;
            let json = value
                .as_string()
                .ok_or_else(|| TransportError::Protocol("Missing HTTP headers".into()))?;
            let (status, pairs): (u16, Vec<(String, String)>) =
                serde_json::from_str(&json).map_err(|e| TransportError::Protocol(e.to_string()))?;
            let mut headers = SimpleHeaders::new();
            for (key, value) in pairs {
                headers.entry(key.into()).or_default().push(value);
            }
            Ok((Status::from(status.to_string()), headers))
        })
    }
    fn read(&self, max_bytes: usize) -> BoxFuture<'_, Result<Option<Vec<u8>>, TransportError>> {
        let id = self.0;
        task(async move {
            let value = JsFuture::from(read_exchange(id, max_bytes))
                .await
                .map_err(error)?;
            Ok((!value.is_undefined()).then(|| Uint8Array::new(&value).to_vec()))
        })
    }
    fn close(&self) {
        close_exchange(self.0);
    }
}

/// JavaScript adapter for the generated Foundation action client.
#[wasm_bindgen]
pub struct BrowserConnectSender(ConnectActionTransport);

#[wasm_bindgen]
impl BrowserConnectSender {
    /// Create an action sender for the base address and Fetch function.
    ///
    /// # Errors
    ///
    /// Return an error if context creation, Fetch registration, or client setup fails.
    #[wasm_bindgen(constructor)]
    pub fn new(base_url: &str, fetcher: Function) -> Result<Self, JsValue> {
        let ctx =
            foundation_context::Ctx::try_host().map_err(|e| JsValue::from_str(&e.to_string()))?;
        let transport = PlatformHttpTransport::new(Arc::new(FetchClient::new(fetcher)?));
        let client = ActionsClient::new(Arc::new(transport), base_url, ClientOptions::new())
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        Ok(Self(ConnectActionTransport::new(
            client,
            ctx,
            ClientCaps::web_current(),
            WireLimits::default(),
        )))
    }

    /// Send an action frame and return the optional response frame.
    ///
    /// # Errors
    ///
    /// Return a JavaScript error if the action transport fails.
    pub async fn exchange(&self, bytes: Vec<u8>) -> Result<Option<Vec<u8>>, JsValue> {
        self.0
            .send(bytes)
            .await
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }
}
