//! Interceptor system (Decision 04 §Interceptor System).
//!
//! WHY: Cross-cutting concerns (auth, logging, metrics, tracing, timeouts) wrap
//! RPC execution over **bytes + metadata** — never the decoded message (that is
//! the facade message-middleware, Decision 11). The chain is composed **once at
//! registration** (connect-go parity), so the per-request codec name rides the
//! call values rather than the chain.
//!
//! WHAT: the [`Interceptor`] trait, the future-returning fn types
//! ([`UnaryFunc`]/[`StreamingHandlerFunc`]/[`StreamingClientFunc`]), the call
//! values ([`UnaryCall`]/[`UnaryReply`]/[`StreamCall`]), [`InterceptorChain`]
//! (reverse-order composition), the [`UnaryInterceptorFunc`] convenience, and
//! [`RecoverInterceptor`] (panic recovery).
//!
//! HOW: fn-types are `Arc<dyn Fn(...) -> BoxFuture<'static, ...>>` — the innermost
//! `UnaryFunc` is the async handler invocation, so wrappers `.await` it. Owned
//! args keep the returned future `'static`.

use std::any::Any;
use std::panic::AssertUnwindSafe;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use bytes::Bytes;
use foundation_netio::shared::http::SimpleHeaders;

use crate::context::{Ctx, Spec};
use crate::error::{ConnectError, ConnectResult};
use crate::transport::{BoxFuture, ClientConn, HandlerConn};

/// Encoded unary request handed to an interceptor. `codec_name` rides the call
/// because the chain is composed once while the codec is negotiated per request.
pub struct UnaryCall {
    /// Request headers.
    pub headers: SimpleHeaders,
    /// Negotiated codec wire name (resolves the `ProcedureCodecs` pair downstream).
    pub codec_name: String,
    /// Encoded request frame.
    pub frame: Bytes,
}

/// Encoded unary reply returned through an interceptor.
pub struct UnaryReply {
    /// Response headers.
    pub headers: SimpleHeaders,
    /// Response trailers.
    pub trailers: SimpleHeaders,
    /// Encoded response frame.
    pub frame: Bytes,
}

/// Streaming-handler call: the negotiated codec name + the byte-level conn (by
/// value, so an interceptor can wrap/embed it).
pub struct StreamCall {
    /// Negotiated codec wire name.
    pub codec_name: String,
    /// The unsplit handler conn (a wrapper's `split` wraps the inner halves).
    pub conn: Box<dyn HandlerConn>,
}

/// Async unary RPC function — the innermost is the handler invocation.
pub type UnaryFunc =
    Arc<dyn Fn(Ctx, UnaryCall) -> BoxFuture<'static, ConnectResult<UnaryReply>> + Send + Sync>;

/// Async streaming-handler RPC function.
pub type StreamingHandlerFunc =
    Arc<dyn Fn(Ctx, StreamCall) -> BoxFuture<'static, ConnectResult<()>> + Send + Sync>;

/// Async streaming-client RPC function (opening a conn does I/O via the transport).
pub type StreamingClientFunc =
    Arc<dyn Fn(Ctx, Spec) -> BoxFuture<'static, ConnectResult<Box<dyn ClientConn>>> + Send + Sync>;

/// Seam interceptor — wraps RPC execution for cross-cutting concerns (bytes +
/// metadata). Mirrors connect-go's `Interceptor`.
pub trait Interceptor: Send + Sync + 'static {
    /// Wrap a unary RPC function.
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc;
    /// Wrap a streaming-client RPC function.
    fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc;
    /// Wrap a streaming-handler RPC function.
    fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc;
}

/// A composed interceptor chain (Decision 04). Interceptors apply in **reverse
/// order** so the first in the list executes first (connect-go `newChain`).
pub struct InterceptorChain {
    interceptors: Vec<Arc<dyn Interceptor>>,
}

impl InterceptorChain {
    /// Build a chain from an ordered list.
    #[must_use]
    pub fn new(interceptors: Vec<Arc<dyn Interceptor>>) -> Self {
        Self { interceptors }
    }

    /// Compose the unary chain around `next` (composed once at registration).
    #[must_use]
    pub fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc {
        let mut func = next;
        for interceptor in self.interceptors.iter().rev() {
            func = interceptor.wrap_unary(func);
        }
        func
    }

    /// Compose the streaming-handler chain around `next`.
    #[must_use]
    pub fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc {
        let mut func = next;
        for interceptor in self.interceptors.iter().rev() {
            func = interceptor.wrap_streaming_handler(func);
        }
        func
    }

    /// Compose the streaming-client chain around `next`.
    #[must_use]
    pub fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc {
        let mut func = next;
        for interceptor in self.interceptors.iter().rev() {
            func = interceptor.wrap_streaming_client(func);
        }
        func
    }
}

/// Convenience interceptor that only wraps unary RPCs.
pub struct UnaryInterceptorFunc<F>(pub F);

impl<F> Interceptor for UnaryInterceptorFunc<F>
where
    F: Fn(UnaryFunc) -> UnaryFunc + Send + Sync + 'static,
{
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc {
        (self.0)(next)
    }
    fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc {
        next
    }
    fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc {
        next
    }
}

/// The panic payload passed to a [`RecoverInterceptor`] handler.
pub type PanicPayload = Box<dyn Any + Send>;

/// Panic-recovery interceptor (Decision 04 RS4 / H18/H19). Wraps handler
/// execution in `catch_unwind`; on panic it calls the user's handler (which can
/// log / emit metrics) and returns its [`ConnectError`] (typically
/// [`Code::Internal`](crate::Code::Internal)). Wraps **unary + streaming-handler
/// only** — never streaming-client. The connection is treated as poisoned after
/// a caught panic (the panicked future is dropped, so the conn is not reused).
pub struct RecoverInterceptor<F> {
    handler: F,
}

impl<F> RecoverInterceptor<F>
where
    F: Fn(&Ctx, &Spec, &SimpleHeaders, PanicPayload) -> ConnectError + Clone + Send + Sync + 'static,
{
    /// Create a recover interceptor from a panic handler.
    pub fn new(handler: F) -> Self {
        Self { handler }
    }
}

impl<F> Interceptor for RecoverInterceptor<F>
where
    F: Fn(&Ctx, &Spec, &SimpleHeaders, PanicPayload) -> ConnectError + Clone + Send + Sync + 'static,
{
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc {
        let handler = self.handler.clone();
        Arc::new(move |ctx: Ctx, call: UnaryCall| {
            let handler = handler.clone();
            let next = next.clone();
            let inner = next(ctx.clone(), call);
            Box::pin(async move {
                match (CatchUnwind { inner }).await {
                    Ok(result) => result,
                    Err(payload) => {
                        let spec = ctx.spec().clone();
                        let headers = (*ctx.request.headers).clone();
                        Err(handler(&ctx, &spec, &headers, payload).into())
                    }
                }
            })
        })
    }

    fn wrap_streaming_client(&self, next: StreamingClientFunc) -> StreamingClientFunc {
        // H18/H19: never wraps streaming-client.
        next
    }

    fn wrap_streaming_handler(&self, next: StreamingHandlerFunc) -> StreamingHandlerFunc {
        let handler = self.handler.clone();
        Arc::new(move |ctx: Ctx, call: StreamCall| {
            let handler = handler.clone();
            let next = next.clone();
            let inner = next(ctx.clone(), call);
            Box::pin(async move {
                match (CatchUnwind { inner }).await {
                    Ok(result) => result,
                    Err(payload) => {
                        let spec = ctx.spec().clone();
                        let headers = (*ctx.request.headers).clone();
                        Err(handler(&ctx, &spec, &headers, payload).into())
                    }
                }
            })
        })
    }
}

/// A future adapter that catches a panic during `poll` (Decision 04 RS4). The
/// inner future is asserted unwind-safe: on a caught panic it is dropped, so any
/// half-built connection state goes with it (poisoned, not reused).
struct CatchUnwind<F> {
    inner: F,
}

impl<F: std::future::Future> std::future::Future for CatchUnwind<F> {
    type Output = std::thread::Result<F::Output>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: we only project a pinned reference to `inner`, never move it.
        let inner = unsafe { self.map_unchecked_mut(|s| &mut s.inner) };
        match std::panic::catch_unwind(AssertUnwindSafe(|| inner.poll(cx))) {
            Ok(Poll::Ready(v)) => Poll::Ready(Ok(v)),
            Ok(Poll::Pending) => Poll::Pending,
            Err(payload) => Poll::Ready(Err(payload)),
        }
    }
}
