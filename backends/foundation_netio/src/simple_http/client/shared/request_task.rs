//! Platform-agnostic HTTP exchange types for `TaskIterator`-based request tasks.
//!
//! WHY: The transport pump in connectrpc needs a single `TaskIterator` shape for HTTP
//! exchanges — native wraps `SendRequestTask<R>` (DNS + TCP + TLS + HTTP), WASM wraps
//! `fetch()` (browser Fetch API). Both produce the same `Ready`/`Pending` types so the
//! pump code is platform-agnostic.
//!
//! WHAT: [`HttpExchange`] (the `Ready` type), [`HttpExchangePending`] (the `Pending`
//! type). Each platform provides its own `HttpExchangeTask` in its own directory,
//! exported via `new_http_exchange_task(request, …)`.

use std::sync::Arc;

use crate::simple_http::shared::{SimpleHeaders, Status};
use bytes::Bytes;

/// What a platform HTTP request task yields on each poll.
///
/// Ordering guarantee: if the request succeeds, `Head` is yielded first (exactly
/// once), followed by zero or more `BodyChunk`s, then `None` (exhausted). If the
/// request fails, `Failed` is yielded (exactly once), then `None`.
pub enum HttpExchange {
    /// Response head. Exactly once per successful request.
    Head {
        status: Status,
        headers: SimpleHeaders,
    },
    /// One chunk of response body bytes. Zero or more.
    BodyChunk(Bytes),
    /// The request or response failed. Exactly once; no `Head` was produced.
    ///
    /// WHY `Arc` (F45 Resolution 7): a pre-head failure is fanned out to **both**
    /// split branches (head observer as `Err`, body continuation as `Err`), so the
    /// error payload must clone losslessly rather than being consumed once.
    Failed(Arc<dyn std::error::Error + Send + Sync + 'static>),
}

/// Marker for the pending state — the task is waiting for I/O (DNS, TCP, TLS, or
/// the response stream).
#[derive(Debug, Clone, Copy)]
pub enum HttpExchangePending {
    Waiting,
}

// ---------------------------------------------------------------------------
// HttpExchangeClientTask — platform-erased wrapper
// ---------------------------------------------------------------------------

#[cfg(all(feature = "multi", not(target_family = "wasm")))]
use crate::simple_http::client::HttpExchangeTask as NativeExchangeTask;
#[cfg(all(feature = "multi", not(target_family = "wasm")))]
use crate::simple_http::client::shared::BoxedDnsResolver;

#[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
use crate::simple_http::client::WasmHttpExchangeTask;

use foundation_core::valtron::{BoxedSendExecutionAction, TaskIterator, TaskStatus};

/// Platform-erased HTTP exchange task — one concrete `TaskIterator` type for both
/// native and wasm targets.
///
/// WHY: `HttpClient::open_exchange` needs a single return type, but the native
/// task is generic over the resolver. This wrapper erases the platform choice
/// behind identical associated types (`Ready`, `Pending`, `Spawner`).
///
/// WHAT: On native, wraps `HttpExchangeTask<BoxedDnsResolver>` (the resolver is
/// erased to a shared trait object at construction). On wasm, wraps
/// `WasmHttpExchangeTask`.
///
/// HOW: Cfg-gated inner field. The `TaskIterator` impl delegates directly — no
/// dynamic dispatch, no allocation beyond what the inner task already does.
pub struct HttpExchangeClientTask {
    #[cfg(all(feature = "multi", not(target_family = "wasm")))]
    inner_native: NativeExchangeTask<BoxedDnsResolver>,
    #[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
    inner_wasm: WasmHttpExchangeTask,
}

impl HttpExchangeClientTask {
    /// Wrap a native exchange task (erased to `BoxedDnsResolver`).
    #[cfg(all(feature = "multi", not(target_family = "wasm")))]
    #[must_use]
    pub fn native(task: NativeExchangeTask<BoxedDnsResolver>) -> Self {
        Self { inner_native: task }
    }

    /// Wrap a WASM exchange task.
    #[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
    #[must_use]
    pub fn wasm(task: WasmHttpExchangeTask) -> Self {
        Self { inner_wasm: task }
    }
}

impl TaskIterator for HttpExchangeClientTask {
    type Ready = HttpExchange;
    type Pending = HttpExchangePending;
    type Spawner = BoxedSendExecutionAction;

    fn next_status(&mut self) -> Option<TaskStatus<Self::Ready, Self::Pending, Self::Spawner>> {
        #[cfg(all(feature = "multi", not(target_family = "wasm")))]
        {
            return self.inner_native.next_status();
        }
        #[cfg(all(target_family = "wasm", feature = "wasm-fetch"))]
        {
            return self.inner_wasm.next_status();
        }
        #[cfg(not(any(
            all(feature = "multi", not(target_family = "wasm")),
            all(target_family = "wasm", feature = "wasm-fetch")
        )))]
        {
            let _ = self;
            None
        }
    }
}
