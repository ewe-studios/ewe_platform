//! Platform HTTP exchange task (WASM) — yields `HttpExchange` items from a
//! `FetchHttpClient` via a `FutureTask` polled cooperatively.
//!
//! WHY: The transport pump in connectrpc needs a platform-agnostic HTTP exchange
//! shape. On WASM this wraps a `Future` that calls `HttpClient::send_async()` and
//! translates the `SimpleResponse` into `HttpExchange` items.
//!
//! WHAT: [`WasmHttpExchangeTask`] — a `TaskIterator` that holds a `FutureTask`,
//! polls it to completion, then drains the response body into `HttpExchange` items.
//!
//! HOW: `FutureTask::new(future)` wraps the fetch. On each `next_status()` call
//! we poll the child task. When it completes we extract the head, then yield body
//! chunks. `run_until_next_state()` drives the executor between polls.

use std::future::Future;
use std::pin::Pin;

use foundation_compact::SendWrapper;
use foundation_core::extensions::result_ext::SendableBoxedError;
use foundation_core::valtron::{
    from_future, BoxedSendExecutionAction, FutureTask, Stream, TaskIterator, TaskStatus,
};

use crate::shared::client::body_reader::{SendSafeBodyBytesItem, SendSafeBodyBytesIterator};
use crate::shared::client::http_client::HttpClient;
use crate::shared::client::request::PreparedRequest;
use crate::shared::client::request_task::{HttpExchange, HttpExchangePending};
use crate::shared::http::{HttpClientError, SendSafeBody};
use crate::wasm::client::client::FetchHttpClient;

// ---------------------------------------------------------------------------
// Internal types
// ---------------------------------------------------------------------------

/// Output from the async fetch — all response data needed for the exchange.
struct FetchOutput {
    status: crate::shared::http::Status,
    headers: crate::shared::http::SimpleHeaders,
    body: SendSafeBody,
}

/// Async block that calls the HTTP client and returns structured output.
async fn fetch_via_client(req: PreparedRequest) -> Result<FetchOutput, HttpClientError> {
    let client = FetchHttpClient::new();
    let (status, headers, body) = client.send_async(req).await?.into_parts();
    Ok(FetchOutput {
        status,
        headers,
        body,
    })
}

type FetchFuture = Pin<Box<dyn Future<Output = Result<FetchOutput, HttpClientError>> + Send>>;
type FetchTask = FutureTask<FetchFuture>;

// ---------------------------------------------------------------------------
// WasmHttpExchangeTask
// ---------------------------------------------------------------------------

/// State machine for the WASM HTTP exchange.
enum WasmExchangeState {
    /// Child `FutureTask` being polled.
    Polling { task: FetchTask },
    /// Future resolved; draining body chunks.
    DrainingBody {
        chunk_iter: SendSafeBodyBytesIterator,
    },
    /// Fetch or body failed.
    Failed(Option<SendableBoxedError>),
}

/// WASM HTTP exchange task.
pub struct WasmHttpExchangeTask {
    state: WasmExchangeState,
}

impl WasmHttpExchangeTask {
    #[must_use]
    pub fn new(request: PreparedRequest) -> Self {
        let future: FetchFuture = Box::pin(SendWrapper::new(fetch_via_client(request)));
        Self {
            state: WasmExchangeState::Polling {
                task: from_future(future),
            },
        }
    }
}

impl TaskIterator for WasmHttpExchangeTask {
    type Ready = HttpExchange;
    type Pending = HttpExchangePending;
    // House law: always `BoxedSendExecutionAction`, never `NoSpawner`/`NoAction`.
    // This task never returns `TaskStatus::Spawn`, but the Spawner type is uniform
    // so it can unify with the native task behind `HttpExchangeClientTask` (F51).
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        match &mut self.state {
            WasmExchangeState::Polling { ref mut task } => match task.next_status() {
                Some(TaskStatus::Ready(Ok(FetchOutput {
                    status,
                    headers,
                    body,
                }))) => {
                    self.state = WasmExchangeState::DrainingBody {
                        chunk_iter: SendSafeBodyBytesIterator::new(body),
                    };
                    Some(TaskStatus::Ready(HttpExchange::Head { status, headers }))
                }
                Some(TaskStatus::Ready(Err(e))) => {
                    let err: SendableBoxedError = Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("{e}"),
                    ));
                    self.state = WasmExchangeState::Failed(Some(err));
                    Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                }
                Some(TaskStatus::Pending(_)) => {
                    Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                }
                Some(TaskStatus::Depends(_)) => {
                    Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                }
                Some(TaskStatus::Spawn(_))
                | Some(TaskStatus::Ignore)
                | Some(TaskStatus::Wait)
                | Some(TaskStatus::Init)
                | Some(TaskStatus::Delayed(_))
                | Some(TaskStatus::Spread(_)) => {
                    Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                }
                None => {
                    let err: SendableBoxedError = Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "fetch task completed without yielding Ready",
                    ));
                    self.state = WasmExchangeState::Failed(Some(err));
                    Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                }
            },
            WasmExchangeState::DrainingBody { ref mut chunk_iter } => match chunk_iter.next() {
                Some(Stream::Next(SendSafeBodyBytesItem::Chunk(bytes))) => {
                    Some(TaskStatus::Ready(HttpExchange::BodyChunk(bytes)))
                }
                Some(Stream::Next(SendSafeBodyBytesItem::StreamError(e))) => {
                    let se: SendableBoxedError = Box::new(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        e.to_string(),
                    ));
                    self.state = WasmExchangeState::Failed(Some(se));
                    Some(TaskStatus::Pending(HttpExchangePending::Waiting))
                }
                Some(Stream::Ignore) => Some(TaskStatus::Pending(HttpExchangePending::Waiting)),
                None => None,
                _ => Some(TaskStatus::Pending(HttpExchangePending::Waiting)),
            },
            WasmExchangeState::Failed(ref mut opt) => opt
                .take()
                // Box → Arc at the public boundary (F45 Resolution 7): the error
                // is fanned out to both split branches, so it must be clonable.
                .map(|e| TaskStatus::Ready(HttpExchange::Failed(std::sync::Arc::from(e)))),
        }
    }
}

// ---------------------------------------------------------------------------
// Constructor re-export
// ---------------------------------------------------------------------------

#[must_use]
pub fn new_http_exchange_task(request: PreparedRequest) -> WasmHttpExchangeTask {
    WasmHttpExchangeTask::new(request)
}

// ---------------------------------------------------------------------------
// Compile-time smoke test
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::http::{Extensions, SimpleHeaders, SimpleMethod};
    use foundation_core::url::Uri;

    #[test]
    fn test_wasm_http_exchange_task_constructs() {
        let request = PreparedRequest {
            method: SimpleMethod::GET,
            url: Uri::parse("http://example.com/").expect("uri"),
            headers: SimpleHeaders::new(),
            body: SendSafeBody::None,
            extensions: Extensions::new(),
        };
        let mut task = new_http_exchange_task(request);
        let _status = task.next_status();
    }
}
