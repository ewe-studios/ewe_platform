//! Native HTTP/1.1 `Transport` implementation (Decision 11 §Transport, Feature 44
//! walking skeleton).
//!
//! WHY: The `Transport` trait defines the byte-level client-seam contract. The
//! HTTP/1.1 implementation is the first concrete transport — it proves the seam (the
//! caller pushes request bytes into `send_body` and receives response bytes from
//! `recv_body`) and is the client connection-owner: `open()` spawns the byte pump on
//! the valtron pool and returns the three caller-facing pipe halves synchronously
//! (Decision 11 §Connection ownership).
//!
//! WHAT: [`H1Transport`] — wraps a netio `SimpleHttpClient` and implements
//! `Transport`. `open()` creates a `TransportPump` that wraps a native
//! `HttpExchangeTask` and forwards responses to the output pipes.
//!
//! HOW: `HttpExchangeTask` spawns `SendRequestTask` via `inlined_task`, which in turn
//! spawns `GetHttpRequestRedirectTask` — the full DNS → TCP → TLS → HTTP round-trip.
//! `TransportPump` polls `HttpExchangeTask`, forwards `HttpExchange::Head` →
//! `head_tx`, `HttpExchange::BodyChunk` → `recv_tx`. The pump is spawned via
//! `valtron::send()`. No `BoxFuture`, no async fn, no `block_on`.

use std::sync::Arc;

use foundation_core::valtron::{self, NoAction, TaskIterator, TaskStatus};
use foundation_netio::simple_http::client::shared::request_task::{
    HttpExchange, HttpExchangePending,
};
use foundation_netio::simple_http::client::shared::{ClientConfig, PreparedRequest, SystemDnsResolver};
use foundation_netio::simple_http::client::HttpExchangeTask;
use foundation_netio::simple_http::client::{ClientRequestBuilder, HttpConnectionPool, SimpleHttpClient};
use foundation_netio::simple_http::shared::{
    pushable_request_body_with_depth, HttpClientError, Proto, RequestDescriptor,
    SimpleHeaders, Status, DEFAULT_PUSHABLE_DEPTH,
};

use super::{
    ByteSink, ByteSource, HeadSource, Transport, TransportCapabilities, TransportError,
    TransportStream,
};

/// Native HTTP/1.1 transport — the client connection-owner.
#[derive(Clone)]
pub struct H1Transport {
    client: Arc<SimpleHttpClient>,
}

impl H1Transport {
    /// Create from a pre-configured `SimpleHttpClient`.
    #[must_use]
    pub fn new(client: SimpleHttpClient) -> Self {
        Self {
            client: Arc::new(client),
        }
    }
}

impl Transport for H1Transport {
    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities {
            request_streaming: true,
            full_duplex: false,
            h2_trailers: false,
            http_versions: &[Proto::HTTP11],
            multiplexed: false,
        }
    }

    fn open(
        &self,
        request: RequestDescriptor,
    ) -> Result<TransportStream, TransportError> {
        let url_str = request_url_string(&request);
        let method = request.method.clone();
        let headers = request.headers.clone();

        // 1. Pushable request body → caller's send_body.
        let (pushable, body_stream) =
            pushable_request_body_with_depth(DEFAULT_PUSHABLE_DEPTH);
        let send_body: ByteSink = pushable.into_sender();

        // 2. Build PreparedRequest.
        let prepared = ClientRequestBuilder::<SystemDnsResolver>::new(method, &url_str)
            .map_err(http_to_transport_error)?
            .body(body_stream)
            .headers(headers)
            .build()
            .map_err(http_to_transport_error)?;

        // 3. Create output pipes.
        let (head_tx, head_rx): (
            foundation_core::valtron::PipeSender<(Status, SimpleHeaders)>,
            HeadSource,
        ) = foundation_core::valtron::Pipe::with_depth(1);
        let (recv_tx, recv_body): (ByteSink, ByteSource) =
            foundation_core::valtron::Pipe::with_depth(DEFAULT_PUSHABLE_DEPTH);

        // 4. Create the pump — wraps HttpExchangeTask, forwards to pipes.
        let pump = TransportPump::new(
            prepared,
            self.client.client_pool().ok_or_else(|| {
                TransportError::Connect(Box::new(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "no connection pool configured",
                )))
            })?,
            self.client.client_config(),
            head_tx,
            recv_tx,
        );

        // 5. Spawn on the valtron pool.
        valtron::send(pump).map_err(|e| {
            TransportError::Connect(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            )))
        })?;

        // 6. Return caller-facing pipe halves.
        Ok(TransportStream {
            send_body,
            head: head_rx,
            recv_body,
        })
    }
}

// ── TransportPump ────────────────────────────────────────────────────────────────

/// Pump state: polls `HttpExchangeTask`, forwards `Head` → `head_tx`,
/// `BodyChunk` → `recv_tx`.
enum PumpState {
    Running(HttpExchangeTask),
    Done,
}

/// Thin `TaskIterator` wrapper around `HttpExchangeTask` that feeds the pump's
/// three output pipes. Each turn polls `HttpExchangeTask` and routes results.
struct TransportPump {
    state: PumpState,
    head_tx: Option<foundation_core::valtron::PipeSender<(Status, SimpleHeaders)>>,
    recv_tx: Option<ByteSink>,
}

impl TransportPump {
    fn new(
        request: PreparedRequest,
        pool: Arc<HttpConnectionPool<SystemDnsResolver>>,
        config: ClientConfig,
        head_tx: foundation_core::valtron::PipeSender<(Status, SimpleHeaders)>,
        recv_tx: ByteSink,
    ) -> Self {
        let task = HttpExchangeTask::new(request, config.max_redirects, pool, config);
        Self {
            state: PumpState::Running(task),
            head_tx: Some(head_tx),
            recv_tx: Some(recv_tx),
        }
    }
}

impl TaskIterator for TransportPump {
    type Ready = ();
    type Pending = HttpExchangePending;
    type Spawner = foundation_core::valtron::BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        let pump = match &mut self.state {
            PumpState::Running(pump) => pump,
            PumpState::Done => return None,
        };

        loop {
            return match pump.next_status() {
                Some(TaskStatus::Ready(HttpExchange::Head { status, headers })) => {
                    if let Some(tx) = &self.head_tx {
                        let _ignore_full = tx.try_send((status, headers));
                    }
                    self.head_tx = None;
                    continue;
                }
                Some(TaskStatus::Ready(HttpExchange::BodyChunk(bytes))) => {
                    if let Some(tx) = &self.recv_tx {
                        let _ignore_full = tx.try_send(bytes);
                    }
                    continue;
                }
                Some(TaskStatus::Ready(HttpExchange::Failed(_e))) => {
                    self.head_tx = None;
                    self.recv_tx = None;
                    self.state = PumpState::Done;
                    Some(TaskStatus::Ready(()))
                }
                Some(TaskStatus::Pending(p)) => {
                    Some(TaskStatus::Pending(p))
                }
                Some(TaskStatus::Spawn(action)) => {
                    Some(TaskStatus::Spawn(action))
                }
                Some(
                    TaskStatus::Init
                    | TaskStatus::Ignore
                    | TaskStatus::Wait
                    | TaskStatus::Delayed(_)
                    | TaskStatus::Depends(_),
                ) => {
                    // Map Depends to Pending since we don't share the child's signal type.
                    Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                }
                Some(TaskStatus::Spread(_)) => continue,
                None => {
                    self.recv_tx = None;
                    self.state = PumpState::Done;
                    Some(TaskStatus::Ready(()))
                }
            };
        }
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────────

fn request_url_string(req: &RequestDescriptor) -> String {
    format!(
        "http://{}:{}{}",
        req.request_uri
            .host_str()
            .unwrap_or_else(|| "localhost".to_string()),
        req.request_uri.port_or_default(),
        req.request_uri.path()
    )
}

fn http_to_transport_error(e: HttpClientError) -> TransportError {
    TransportError::Connect(Box::new(e))
}
