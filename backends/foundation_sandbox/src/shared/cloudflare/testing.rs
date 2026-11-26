//! Test doubles for the Cloudflare part.
//!
//! Each double implements one trait of the part with no platform call. A test
//! builds a [`crate::shared::cloudflare::CloudflareHost`] from them.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Waker};

use foundation_connectrpc::{in_memory_body, CallHooks, ConnectRpcHandler, ResponseSink};
use foundation_context::Ctx;
use foundation_core::valtron::{Pipe, PipeReceiver, PipeSender};
use foundation_netio::shared::http::SimpleOutgoingResponse;
use foundation_netio::shared::http::{
    Proto, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleIncomingRequest, SimpleMethod,
};

use crate::shared::cloudflare::model_slave::ModelRequest;
use crate::shared::cloudflare::{
    CloudflareHost, ContainerApi, ContainerStart, DoAlarm, DoStorage, PortError, PortRequest,
    PortResponse, SandboxIndex, ServiceBinding,
};
use crate::shared::control::SandboxInfo;
use crate::shared::error::{SandboxError, SandboxResult};
use crate::shared::host::SlaveConnector;
use crate::shared::ids::SandboxId;
use crate::shared::rpc::SlaveService;
use crate::shared::{BoxByteStream, BoxFuture};

/// The address that an in-memory request uses in the request line.
const TEST_AUTHORITY: &str = "http://container.local";

/// One completion flag that a task can wait on.
#[derive(Debug, Default)]
struct Flag {
    done: AtomicBool,
    wakers: Mutex<Vec<Waker>>,
}

impl Flag {
    fn complete(&self) {
        self.done.store(true, Ordering::SeqCst);
        let wakers = std::mem::take(
            &mut *self
                .wakers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
        for waker in wakers {
            waker.wake();
        }
    }

    fn is_done(&self) -> bool {
        self.done.load(Ordering::SeqCst)
    }

    async fn wait(&self) {
        futures::future::poll_fn(|context: &mut Context<'_>| {
            if self.is_done() {
                return Poll::Ready(());
            }
            self.wakers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(context.waker().clone());
            Poll::Pending
        })
        .await;
    }
}

/// A fake container.
pub struct FakeContainer {
    running: AtomicBool,
    start: Mutex<Option<ContainerStart>>,
    start_count: AtomicUsize,
    start_error: Mutex<Option<String>>,
    destroy_count: AtomicUsize,
    signal: Mutex<Option<i32>>,
    exit: Flag,
    fetches: Mutex<Vec<u16>>,
    failures: Mutex<VecDeque<PortError>>,
    raw: Mutex<Option<PortResponse>>,
    handler: Mutex<Option<Arc<ConnectRpcHandler>>>,
}

impl FakeContainer {
    /// A container that does not run.
    #[must_use]
    pub fn new() -> Self {
        Self {
            running: AtomicBool::new(false),
            start: Mutex::new(None),
            start_count: AtomicUsize::new(0),
            start_error: Mutex::new(None),
            destroy_count: AtomicUsize::new(0),
            signal: Mutex::new(None),
            exit: Flag::default(),
            fetches: Mutex::new(Vec::new()),
            failures: Mutex::new(VecDeque::new()),
            raw: Mutex::new(None),
            handler: Mutex::new(None),
        }
    }

    /// The same container with this many failures before it answers.
    #[must_use]
    pub fn with_failures(self, failures: Vec<PortError>) -> Self {
        *self
            .failures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = failures.into();
        self
    }

    /// The same container with one raw response for every fetch.
    #[must_use]
    pub fn with_raw_response(self, response: PortResponse) -> Self {
        *self
            .raw
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(response);
        self
    }

    /// The same container with the slave service behind its port.
    #[must_use]
    pub fn with_slave(self, handler: Arc<ConnectRpcHandler>) -> Self {
        *self
            .handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(handler);
        self
    }

    /// Replaces the pending fetch failures.
    pub fn set_failures(&self, failures: Vec<PortError>) {
        *self
            .failures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = failures.into();
    }

    /// Replaces the raw response of every later fetch.
    pub fn set_raw_response(&self, response: PortResponse) {
        *self
            .raw
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(response);
    }

    /// Makes the next start fail with `message`.
    pub fn fail_next_start(&self, message: impl Into<String>) {
        *self
            .start_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(message.into());
    }

    /// The start options that the container received.
    #[must_use]
    pub fn start_options(&self) -> Option<ContainerStart> {
        self.start
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// How many times the container was started.
    #[must_use]
    pub fn start_count(&self) -> usize {
        self.start_count.load(Ordering::SeqCst)
    }

    /// How many times the container was destroyed.
    #[must_use]
    pub fn destroy_count(&self) -> usize {
        self.destroy_count.load(Ordering::SeqCst)
    }

    /// The last signal, when one arrived.
    #[must_use]
    pub fn last_signal(&self) -> Option<i32> {
        *self
            .signal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// The ports of the fetches that arrived.
    #[must_use]
    pub fn fetches(&self) -> Vec<u16> {
        self.fetches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Completes the exit wait.
    pub fn exit(&self) {
        self.exit.complete();
    }

    /// Whether the container runs.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

impl Default for FakeContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl ContainerApi for FakeContainer {
    fn running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn start(&self, options: ContainerStart) -> SandboxResult<()> {
        if let Some(message) = self
            .start_error
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
        {
            return Err(SandboxError::SubstrateApi { status: 0, message }.into());
        }
        *self
            .start
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(options);
        self.start_count.fetch_add(1, Ordering::SeqCst);
        self.running.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn destroy(&self) -> BoxFuture<'_, SandboxResult<()>> {
        self.running.store(false, Ordering::SeqCst);
        self.destroy_count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }

    fn signal(&self, signo: i32) -> SandboxResult<()> {
        *self
            .signal
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(signo);
        Ok(())
    }

    fn wait_for_exit(&self) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async move {
            self.exit.wait().await;
            Ok(())
        })
    }

    fn fetch(
        &self,
        port: u16,
        request: PortRequest,
    ) -> BoxFuture<'_, Result<PortResponse, PortError>> {
        self.fetches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(port);
        let failure = self
            .failures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front();
        if let Some(failure) = failure {
            return Box::pin(async move { Err(failure) });
        }
        let raw = self
            .raw
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(response) = raw {
            return Box::pin(async move { Ok(response) });
        }
        let handler = self
            .handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        Box::pin(async move {
            let Some(handler) = handler else {
                return Err(PortError::NotListening);
            };
            dispatch_port(handler.as_ref(), request).await
        })
    }
}

/// A byte stream from one in-memory body.
fn one_chunk(body: Vec<u8>) -> BoxByteStream {
    Box::pin(futures::stream::once(async move { Ok(body) }))
}

/// One event of a fake port response.
enum PortEvent {
    Whole(SimpleOutgoingResponse),
    Head(SimpleOutgoingResponse),
    Chunk(bytes::Bytes),
    End,
}

/// The sink that turns router writes into fake port events.
struct PortSink {
    sender: PipeSender<PortEvent>,
}

impl ResponseSink for PortSink {
    fn whole(&mut self, response: SimpleOutgoingResponse) -> BoxFuture<'_, std::io::Result<()>> {
        let sender = self.sender.clone();
        Box::pin(async move {
            sender
                .send(PortEvent::Whole(response))
                .await
                .map_err(|_error| broken_pipe())
        })
    }

    fn open(&mut self, mut head: SimpleOutgoingResponse) -> BoxFuture<'_, std::io::Result<()>> {
        head.body = None;
        let sender = self.sender.clone();
        Box::pin(async move {
            sender
                .send(PortEvent::Head(head))
                .await
                .map_err(|_error| broken_pipe())
        })
    }

    fn chunk(&mut self, bytes: bytes::Bytes) -> BoxFuture<'_, std::io::Result<()>> {
        let sender = self.sender.clone();
        Box::pin(async move {
            sender
                .send(PortEvent::Chunk(bytes))
                .await
                .map_err(|_error| broken_pipe())
        })
    }

    fn finish(&mut self) -> BoxFuture<'_, std::io::Result<()>> {
        let sender = self.sender.clone();
        Box::pin(async move {
            sender
                .send(PortEvent::End)
                .await
                .map_err(|_error| broken_pipe())
        })
    }
}

fn broken_pipe() -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::BrokenPipe, "the fake port sink closed")
}

/// The dispatch future of one fake port response, or its end.
enum PortDispatch {
    Running(BoxFuture<'static, std::io::Result<()>>),
    Done,
}

/// The next fake port event, driving the router while the pipe is empty.
async fn next_port_event(
    state: &mut PortDispatch,
    receiver: &PipeReceiver<PortEvent>,
) -> Option<PortEvent> {
    loop {
        match state {
            PortDispatch::Done => return receiver.receive().await,
            PortDispatch::Running(_) => {
                let PortDispatch::Running(dispatch) = std::mem::replace(state, PortDispatch::Done)
                else {
                    return None;
                };
                let waiting = receiver.clone();
                match futures::future::select(dispatch, Box::pin(waiting.receive())).await {
                    futures::future::Either::Left((_result, _waiting)) => {
                        *state = PortDispatch::Done;
                    }
                    futures::future::Either::Right((event, dispatch)) => {
                        *state = PortDispatch::Running(dispatch);
                        return event;
                    }
                }
            }
        }
    }
}

/// A port response body that forwards the router frames.
fn port_body_stream(state: PortDispatch, receiver: PipeReceiver<PortEvent>) -> BoxByteStream {
    let inner = futures::stream::unfold((state, receiver), |(mut state, receiver)| async move {
        loop {
            match next_port_event(&mut state, &receiver).await {
                Some(PortEvent::Chunk(bytes)) => {
                    return Some((Ok(bytes.to_vec()), (state, receiver)));
                }
                Some(PortEvent::Whole(response)) => {
                    let (_, _, body) = response_parts(response);
                    if !body.is_empty() {
                        return Some((Ok(body), (state, receiver)));
                    }
                }
                Some(PortEvent::End) | None => return None,
                Some(PortEvent::Head(_)) => {}
            }
        }
    });
    Box::pin(crate::shared::cloudflare::SendStream::new(inner))
}

/// Dispatches one port request to an in-process Connect router and turns the
/// answer into a streamed port response.
///
/// # Errors
/// [`PortError::Other`] when the request URL is invalid.
pub async fn dispatch_port(
    handler: &ConnectRpcHandler,
    mut request: PortRequest,
) -> Result<PortResponse, PortError> {
    let path = if request.path.starts_with('/') {
        request.path.clone()
    } else {
        format!("/{}", request.path)
    };
    let full = format!("{TEST_AUTHORITY}{path}");
    let uri = foundation_core::url::Uri::parse(&full)
        .map_err(|error| PortError::Other(error.to_string()))?;
    let mut incoming = SimpleIncomingRequest::builder()
        .with_plain_url(path)
        .with_uri(uri)
        .with_method(SimpleMethod::from(request.method))
        .with_headers(headers_to_simple(request.headers))
        .with_body_bytes(std::mem::take(&mut request.body))
        .with_proto(Proto::HTTP11)
        .build()
        .map_err(|error| PortError::Other(error.to_string()))?;
    let body = in_memory_body(incoming.body.take());
    let (sender, receiver) = Pipe::<PortEvent>::with_depth(Pipe::<()>::DEFAULT_DEPTH);
    let sink: Box<dyn ResponseSink> = Box::new(PortSink { sender });
    let dispatch = handler.dispatch_to(Ctx::host(), incoming, body, sink, CallHooks::default());
    let mut state = PortDispatch::Running(dispatch);
    match next_port_event(&mut state, &receiver).await {
        Some(PortEvent::Whole(response)) => Ok(response_to_port(response)),
        Some(PortEvent::Head(head)) => {
            let status = u16::try_from(head.status.clone().into_usize()).unwrap_or(0);
            let headers = flatten_headers(head.headers);
            Ok(PortResponse {
                status,
                headers,
                body: port_body_stream(state, receiver),
            })
        }
        Some(PortEvent::Chunk(_) | PortEvent::End) | None => Err(PortError::Other(
            "the fake router gave no response head".to_string(),
        )),
    }
}

/// The status, headers and body bytes of one whole response.
fn response_parts(response: SimpleOutgoingResponse) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let status = u16::try_from(response.status.into_usize()).unwrap_or(0);
    let headers = flatten_headers(response.headers);
    let body = match response.body {
        Some(SendSafeBody::Bytes(bytes)) => bytes,
        Some(SendSafeBody::Text(text)) => text.into_bytes(),
        _ => Vec::new(),
    };
    (status, headers, body)
}

/// One HTTP response as a port response.
fn response_to_port(response: SimpleOutgoingResponse) -> PortResponse {
    let (status, headers, body) = response_parts(response);
    PortResponse {
        status,
        headers,
        body: one_chunk(body),
    }
}

/// The HTTP headers of one port request.
fn headers_to_simple(headers: Vec<(String, String)>) -> SimpleHeaders {
    let mut map = SimpleHeaders::new();
    for (name, value) in headers {
        map.entry(SimpleHeader::from(name)).or_default().push(value);
    }
    map
}

/// The port headers of one HTTP response.
fn flatten_headers(headers: SimpleHeaders) -> Vec<(String, String)> {
    let mut flat = Vec::new();
    for (name, values) in headers {
        for value in values {
            flat.push((name.to_string(), value));
        }
    }
    flat
}

/// Builds the router of one slave service.
pub fn slave_handler<S>(service: Arc<S>) -> Arc<ConnectRpcHandler>
where
    S: SlaveService + Send + Sync + 'static,
{
    let mut router = foundation_connectrpc::Router::new();
    crate::shared::rpc::register_slave_service(&mut router, service);
    Arc::new(router.into_handler())
}

/// An in-memory Durable Object store.
#[derive(Default)]
pub struct MemoryDoStorage {
    values: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl MemoryDoStorage {
    /// An empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A value in the store.
    #[must_use]
    pub fn value(&self, key: &str) -> Option<Vec<u8>> {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(key)
            .cloned()
    }

    /// Puts a value before the test starts.
    pub fn insert(&self, key: &str, value: Vec<u8>) {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(key.to_string(), value);
    }

    /// The keys of the store.
    #[must_use]
    pub fn keys(&self) -> Vec<String> {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .keys()
            .cloned()
            .collect()
    }
}

impl DoStorage for MemoryDoStorage {
    fn get(&self, key: &str) -> BoxFuture<'_, SandboxResult<Option<Vec<u8>>>> {
        let value = self.value(key);
        Box::pin(async move { Ok(value) })
    }

    fn put(&self, key: &str, value: Vec<u8>) -> BoxFuture<'_, SandboxResult<()>> {
        self.insert(key, value);
        Box::pin(async { Ok(()) })
    }

    fn delete(&self, key: &str) -> BoxFuture<'_, SandboxResult<()>> {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(key);
        Box::pin(async { Ok(()) })
    }
}

/// A fake Durable Object alarm.
#[derive(Default)]
pub struct FakeAlarm {
    at: Mutex<Option<u64>>,
}

impl FakeAlarm {
    /// No alarm.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The pending alarm.
    #[must_use]
    pub fn value(&self) -> Option<u64> {
        *self
            .at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl DoAlarm for FakeAlarm {
    fn set(&self, at_unix_ms: u64) -> BoxFuture<'_, SandboxResult<()>> {
        *self
            .at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(at_unix_ms);
        Box::pin(async { Ok(()) })
    }

    fn clear(&self) -> BoxFuture<'_, SandboxResult<()>> {
        *self
            .at
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        Box::pin(async { Ok(()) })
    }
}

/// An in-memory sandbox index.
#[derive(Default)]
pub struct MemoryIndex {
    entries: Mutex<HashMap<SandboxId, SandboxInfo>>,
}

impl MemoryIndex {
    /// An empty index.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The entry of one sandbox.
    #[must_use]
    pub fn get(&self, id: &SandboxId) -> Option<SandboxInfo> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .cloned()
    }

    /// The number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
    }

    /// Whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl SandboxIndex for MemoryIndex {
    fn put(&self, info: SandboxInfo) -> BoxFuture<'_, SandboxResult<()>> {
        let id = info.id.parse::<SandboxId>().ok();
        if let Some(id) = id {
            self.entries
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(id, info);
        }
        Box::pin(async { Ok(()) })
    }

    fn remove(&self, id: &SandboxId) -> BoxFuture<'_, SandboxResult<()>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(id);
        Box::pin(async { Ok(()) })
    }
}

/// An index that fails every call.
pub struct FailingIndex;

impl SandboxIndex for FailingIndex {
    fn put(&self, _info: SandboxInfo) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            Err(SandboxError::Io {
                op: "index".to_string(),
                message: "the index is down".to_string(),
            }
            .into())
        })
    }

    fn remove(&self, _id: &SandboxId) -> BoxFuture<'_, SandboxResult<()>> {
        Box::pin(async {
            Err(SandboxError::Io {
                op: "index".to_string(),
                message: "the index is down".to_string(),
            }
            .into())
        })
    }
}

/// A fake model client.
pub struct FakeModelClient {
    answer: Vec<u8>,
    never: bool,
    fail: bool,
    prompts: Mutex<Vec<String>>,
}

impl FakeModelClient {
    /// A client that answers `answer`.
    #[must_use]
    pub fn answer(answer: impl Into<Vec<u8>>) -> Self {
        Self {
            answer: answer.into(),
            never: false,
            fail: false,
            prompts: Mutex::new(Vec::new()),
        }
    }

    /// A client that never answers.
    #[must_use]
    pub fn never() -> Self {
        Self {
            answer: Vec::new(),
            never: true,
            fail: false,
            prompts: Mutex::new(Vec::new()),
        }
    }

    /// A client that fails.
    #[must_use]
    pub fn failing() -> Self {
        Self {
            answer: Vec::new(),
            never: false,
            fail: true,
            prompts: Mutex::new(Vec::new()),
        }
    }

    /// The prompts that the client received.
    #[must_use]
    pub fn prompts(&self) -> Vec<String> {
        self.prompts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl crate::shared::cloudflare::model_slave::ModelClient for FakeModelClient {
    fn complete(&self, _ctx: Ctx, request: ModelRequest) -> BoxFuture<'_, SandboxResult<Vec<u8>>> {
        self.prompts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(request.prompt);
        if self.never {
            return Box::pin(futures::future::pending());
        }
        if self.fail {
            return Box::pin(async {
                Err(SandboxError::Io {
                    op: "model".to_string(),
                    message: "the model is down".to_string(),
                }
                .into())
            });
        }
        let answer = self.answer.clone();
        Box::pin(async move { Ok(answer) })
    }
}

/// A fake service binding.
pub struct MemoryServiceBinding {
    fetches: Mutex<Vec<(String, SandboxId, PortRequest)>>,
    handler: Mutex<Option<Arc<ConnectRpcHandler>>>,
    failure: Mutex<Option<PortError>>,
}

impl MemoryServiceBinding {
    /// A binding with no answer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fetches: Mutex::new(Vec::new()),
            handler: Mutex::new(None),
            failure: Mutex::new(None),
        }
    }

    /// The same binding with the slave service behind it.
    #[must_use]
    pub fn with_slave(self, handler: Arc<ConnectRpcHandler>) -> Self {
        *self
            .handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(handler);
        self
    }

    /// The same binding that fails every call.
    #[must_use]
    pub fn with_failure(self, failure: PortError) -> Self {
        *self
            .failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(failure);
        self
    }

    /// The fetches that arrived.
    #[must_use]
    pub fn fetches(&self) -> Vec<(String, SandboxId, PortRequest)> {
        self.fetches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

impl Default for MemoryServiceBinding {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceBinding for MemoryServiceBinding {
    fn fetch(
        &self,
        binding: &str,
        name: &SandboxId,
        request: PortRequest,
    ) -> BoxFuture<'_, Result<PortResponse, PortError>> {
        self.fetches
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((binding.to_string(), name.clone(), request.clone()));
        let failure = self
            .failure
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if let Some(failure) = failure {
            return Box::pin(async move { Err(failure) });
        }
        let handler = self
            .handler
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        Box::pin(async move {
            let Some(handler) = handler else {
                return Err(PortError::NotListening);
            };
            dispatch_port(handler.as_ref(), request).await
        })
    }
}

/// A host bundle over the fakes.
#[must_use]
pub fn host(
    container: Arc<FakeContainer>,
    storage: Arc<MemoryDoStorage>,
    alarm: Arc<FakeAlarm>,
    index: Arc<dyn SandboxIndex>,
    config: crate::shared::cloudflare::CloudflareConfig,
    service: Option<Arc<dyn ServiceBinding>>,
) -> CloudflareHost {
    CloudflareHost {
        container,
        storage,
        alarm,
        index,
        service,
        config: Arc::new(config),
    }
}

/// One `Get` request to a control plane, in Connect JSON.
///
/// # Panics
/// When the fixed test URL does not parse, or the fixed request does not build.
#[must_use]
pub fn control_get_request(id: &SandboxId) -> crate::shared::cloudflare::HttpRequestParts {
    use foundation_netio::shared::http::SimpleIncomingRequest;

    let path = "/ewe.sandbox.control.v1.SandboxService/Get";
    let full = format!("{TEST_AUTHORITY}{path}");
    let uri = foundation_core::url::Uri::parse(&full).expect("the test URL parses");
    let body = serde_json::to_vec(&crate::shared::rpc::messages::SandboxRef {
        sandbox_id: id.to_string(),
    })
    .unwrap_or_default();
    SimpleIncomingRequest::builder()
        .with_plain_url(path)
        .with_uri(uri)
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/json")
        .with_body_bytes(body)
        .with_proto(Proto::HTTP11)
        .build()
        .expect("the test request builds")
}

/// The status of one control-plane response.
#[must_use]
pub fn response_status(response: &crate::shared::cloudflare::HttpResponseParts) -> u16 {
    response.status
}

/// The body bytes of one control-plane response.
///
/// It collects the body, so it is for a test only.
#[must_use]
pub fn response_body(mut response: crate::shared::cloudflare::HttpResponseParts) -> Vec<u8> {
    futures::executor::block_on(async move {
        use futures::StreamExt;
        let mut body = Vec::new();
        while let Some(item) = response.body.next().await {
            body.extend(item.unwrap_or_default());
        }
        body
    })
}

/// The calls of the slave in the one container of a host.
///
/// # Errors
/// The error of the transport or the client.
pub fn slave_calls(
    ctx: &Ctx,
    host: &CloudflareHost,
    limits: &crate::shared::limits::SandboxLimits,
) -> SandboxResult<Arc<dyn crate::shared::handles::ExecFileCalls>> {
    let connector = crate::shared::cloudflare::transport::PortConnector {
        container: Arc::clone(&host.container),
        config: Arc::clone(&host.config),
        do_slave: None,
    };
    let transport = connector.connect(
        ctx,
        &crate::shared::driver::SlaveEndpoint::CloudflarePort {
            port: host.config.slave_port,
        },
        limits,
    )?;
    let client = crate::shared::control::SlaveClient::new(ctx.clone(), transport, limits.clone())?;
    Ok(Arc::new(client))
}

/// One `Exec` request to a control plane, in Connect streaming JSON.
///
/// # Panics
/// When the fixed test URL does not parse, or the fixed request does not build.
#[must_use]
pub fn control_exec_request(
    id: &SandboxId,
    exec_id: &str,
    stdin: bool,
) -> crate::shared::cloudflare::HttpRequestParts {
    use foundation_netio::shared::http::SimpleIncomingRequest;

    let path = "/ewe.sandbox.control.v1.SandboxService/Exec";
    let full = format!("{TEST_AUTHORITY}{path}");
    let uri = foundation_core::url::Uri::parse(&full).expect("the test URL parses");
    let request = crate::shared::rpc::messages::Scoped {
        sandbox_id: id.to_string(),
        inner: crate::shared::rpc::messages::ExecRequest {
            exec_id: exec_id.to_string(),
            argv: vec!["echo".to_string(), "hi".to_string()],
            env: Vec::new(),
            cwd: None,
            stdin,
            timeout_ms: None,
            user: None,
        },
    };
    let body = serde_json::to_vec(&request).unwrap_or_default();
    // A Connect streaming request carries one envelope: a flag byte, a
    // big-endian length, then the JSON message.
    let mut framed = Vec::with_capacity(body.len() + 5);
    framed.push(0);
    framed.extend_from_slice(&u32::try_from(body.len()).unwrap_or(u32::MAX).to_be_bytes());
    framed.extend_from_slice(&body);
    SimpleIncomingRequest::builder()
        .with_plain_url(path)
        .with_uri(uri)
        .with_method(SimpleMethod::POST)
        .add_header(SimpleHeader::CONTENT_TYPE, "application/connect+json")
        .add_header(SimpleHeader::custom("connect-protocol-version"), "1")
        .with_body_bytes(framed)
        .with_proto(Proto::HTTP11)
        .build()
        .expect("the test request builds")
}

/// A transport to the internal container service of one control.
///
/// # Panics
/// When the in-process transport cannot be built.
#[must_use]
#[allow(
    clippy::needless_pass_by_value,
    reason = "the test helper takes the values the caller owns"
)]
pub fn container_transport(
    ctx: Ctx,
    control: Arc<dyn crate::shared::container::ContainerControl>,
    limits: crate::shared::limits::SandboxLimits,
    calls: Arc<dyn crate::shared::handles::ExecFileCalls>,
    name: SandboxId,
) -> Arc<dyn foundation_connectrpc::shared::transport::Transport> {
    let service = Arc::new(crate::shared::cloudflare::ContainerControlService::new(
        ctx.clone(),
        control,
        limits.clone(),
        calls,
    ));
    let mut router = foundation_connectrpc::Router::new();
    crate::shared::rpc::register_container_service(&mut router, service);
    let handler = Arc::new(router.into_handler());
    let binding = Arc::new(MemoryServiceBinding::new().with_slave(handler));
    let connector = crate::shared::cloudflare::DoSlaveConnector::new(binding);
    connector
        .connect(
            &ctx,
            &crate::shared::driver::SlaveEndpoint::CloudflareDo {
                binding: "SLAVE".to_string(),
                name,
            },
            &limits,
        )
        .expect("the in-process container transport builds")
}

/// One request and its response over a transport.
///
/// It returns the status and the length of each response body chunk. The caller
/// never holds the whole body.
///
/// # Errors
/// [`SandboxError::Io`] or [`SandboxError::StreamLost`] when the exchange fails.
pub fn transport_round_trip(
    transport: &dyn foundation_connectrpc::shared::transport::Transport,
    path: &str,
    body: Vec<u8>,
) -> crate::shared::error::SandboxResult<(u16, Vec<usize>)> {
    use foundation_netio::shared::http::{
        Proto, RequestDescriptor, SimpleHeaders, SimpleMethod, SimpleUrl,
    };

    let url = format!("http://container.local{path}");
    let request = RequestDescriptor {
        proto: Proto::HTTP11,
        request_url: SimpleUrl::url_only(url.clone()),
        request_uri: foundation_core::url::Uri::parse(&url).map_err(|error| SandboxError::Io {
            op: "test transport".to_string(),
            message: error.to_string(),
        })?,
        headers: SimpleHeaders::new(),
        method: SimpleMethod::POST,
    };
    let mut stream = transport.open(request).map_err(|error| SandboxError::Io {
        op: "test transport".to_string(),
        message: error.to_string(),
    })?;
    if !body.is_empty() {
        let _ = stream.send_body.try_send(bytes::Bytes::from(body));
    }
    stream.send_body.close();
    let (status, chunks) = futures::executor::block_on(async move {
        use futures::StreamExt;
        let head = stream.head.next().await.ok_or(SandboxError::StreamLost)?;
        let (status, _headers) = head.map_err(|error| SandboxError::Io {
            op: "test transport".to_string(),
            message: error.to_string(),
        })?;
        let mut chunks = Vec::new();
        while let Some(chunk) = stream.recv_body.next().await {
            let bytes = chunk.map_err(|error| SandboxError::Io {
                op: "test transport".to_string(),
                message: error.to_string(),
            })?;
            chunks.push(bytes.len());
        }
        Ok::<_, foundation_errstacks::ErrorTrace<SandboxError>>((status.into_usize(), chunks))
    })?;
    Ok((u16::try_from(status).unwrap_or(0), chunks))
}
