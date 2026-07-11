//! Interceptor + facade-middleware tests (spec-41 F18 / Decisions 04 & 11):
//! reverse-order composition with per-call codec name, a wrapping interceptor
//! observing frames both directions after split, and panic recovery.

use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

use bytes::Bytes;
use foundation_netio::shared::http::SimpleHeaders;

use foundation_connectrpc::context::{CancelSignal, Ctx, Peer, Spec};
use foundation_connectrpc::interceptor::{
    Interceptor, InterceptorChain, RecoverInterceptor, UnaryCall, UnaryFunc, UnaryReply,
};
use foundation_connectrpc::transport::{
    BoxFuture, ConnReceiver, ConnSender, Frame, HandlerConn, PipeHandlerConn,
};

fn block_on<F: Future>(fut: F) -> F::Output {
    struct W(std::thread::Thread);
    impl Wake for W {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(W(std::thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut fut = std::pin::pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::park(),
        }
    }
}

fn spec() -> Spec {
    Spec::empty(false)
}

fn ctx() -> Ctx {
    Ctx::background()
}

// ── chain composition (reverse order) + per-call codec name ───────────────────

/// Interceptor that appends its tag to a shared log on the way in and out.
struct Tagging {
    tag: &'static str,
    log: Arc<Mutex<Vec<String>>>,
}

impl Interceptor for Tagging {
    fn wrap_unary(&self, next: UnaryFunc) -> UnaryFunc {
        let tag = self.tag;
        let log = Arc::clone(&self.log);
        Arc::new(move |c, call| {
            let log = Arc::clone(&log);
            let next = next.clone();
            Box::pin(async move {
                log.lock().unwrap().push(format!("{tag}:in"));
                let reply = next(c, call).await;
                log.lock().unwrap().push(format!("{tag}:out"));
                reply
            })
        })
    }
    fn wrap_streaming_client(
        &self,
        next: foundation_connectrpc::interceptor::StreamingClientFunc,
    ) -> foundation_connectrpc::interceptor::StreamingClientFunc {
        next
    }
    fn wrap_streaming_handler(
        &self,
        next: foundation_connectrpc::interceptor::StreamingHandlerFunc,
    ) -> foundation_connectrpc::interceptor::StreamingHandlerFunc {
        next
    }
}

#[test]
fn chain_runs_first_interceptor_outermost_and_carries_codec_name() {
    let log = Arc::new(Mutex::new(Vec::new()));
    let seen_codec = Arc::new(Mutex::new(String::new()));

    let seen = Arc::clone(&seen_codec);
    let handler: UnaryFunc = Arc::new(move |_ctx, call: UnaryCall| {
        let seen = Arc::clone(&seen);
        Box::pin(async move {
            *seen.lock().unwrap() = call.codec_name.clone();
            Ok(UnaryReply {
                headers: SimpleHeaders::new(),
                trailers: SimpleHeaders::new(),
                frame: call.frame,
            })
        })
    });

    let chain = InterceptorChain::new(vec![
        Arc::new(Tagging {
            tag: "A",
            log: Arc::clone(&log),
        }),
        Arc::new(Tagging {
            tag: "B",
            log: Arc::clone(&log),
        }),
    ]);
    let composed = chain.wrap_unary(handler);

    let reply = block_on(composed(
        ctx(),
        UnaryCall {
            headers: SimpleHeaders::new(),
            codec_name: "proto".to_string(),
            frame: Bytes::from_static(b"x"),
        },
    ))
    .unwrap();

    assert_eq!(&reply.frame[..], b"x");
    // First-in-list runs outermost: A:in, B:in, B:out, A:out.
    assert_eq!(
        *log.lock().unwrap(),
        vec!["A:in", "B:in", "B:out", "A:out"]
    );
    assert_eq!(*seen_codec.lock().unwrap(), "proto");
}

// ── wrapping streaming interceptor observes frames both directions after split ─

/// Counts frames passing through its wrapped halves.
struct ObservingConn {
    inner: Box<dyn HandlerConn>,
    sent: Arc<AtomicUsize>,
    received: Arc<AtomicUsize>,
}

impl HandlerConn for ObservingConn {
    fn spec(&self) -> &Spec {
        self.inner.spec()
    }
    fn peer(&self) -> &Peer {
        self.inner.peer()
    }
    fn request_headers(&self) -> &SimpleHeaders {
        self.inner.request_headers()
    }
    fn split(self: Box<Self>) -> (Box<dyn ConnReceiver>, Box<dyn ConnSender>) {
        let (rx, tx) = self.inner.split();
        (
            Box::new(ObservingReceiver {
                inner: rx,
                received: self.received,
            }),
            Box::new(ObservingSender {
                inner: tx,
                sent: self.sent,
            }),
        )
    }
}

struct ObservingReceiver {
    inner: Box<dyn ConnReceiver>,
    received: Arc<AtomicUsize>,
}
impl ConnReceiver for ObservingReceiver {
    fn receive(&mut self) -> BoxFuture<'_, foundation_connectrpc::ConnectResult<Option<Bytes>>> {
        let counter = Arc::clone(&self.received);
        Box::pin(async move {
            let out = self.inner.receive().await;
            if let Ok(Some(_)) = &out {
                counter.fetch_add(1, Ordering::SeqCst);
            }
            out
        })
    }
    fn trailers(&self) -> &SimpleHeaders {
        self.inner.trailers()
    }
}

struct ObservingSender {
    inner: Box<dyn ConnSender>,
    sent: Arc<AtomicUsize>,
}
impl ConnSender for ObservingSender {
    fn send_headers(
        &mut self,
        headers: SimpleHeaders,
    ) -> BoxFuture<'_, foundation_connectrpc::ConnectResult<()>> {
        self.inner.send_headers(headers)
    }
    fn send(&mut self, frame: Bytes) -> BoxFuture<'_, foundation_connectrpc::ConnectResult<()>> {
        self.sent.fetch_add(1, Ordering::SeqCst);
        self.inner.send(frame)
    }
    fn close(
        self: Box<Self>,
        error: Option<foundation_errstacks::ErrorTrace<foundation_connectrpc::ConnectError>>,
        trailers: SimpleHeaders,
    ) -> BoxFuture<'static, foundation_connectrpc::ConnectResult<()>> {
        self.inner.close(error, trailers)
    }
}

#[test]
fn wrapping_interceptor_observes_frames_both_directions_after_split() {
    let (conn, ends) = PipeHandlerConn::with_defaults(
        spec(),
        Peer::empty(),
        SimpleHeaders::new(),
        CancelSignal::new(),
    );
    let sent = Arc::new(AtomicUsize::new(0));
    let received = Arc::new(AtomicUsize::new(0));
    let wrapped: Box<dyn HandlerConn> = Box::new(ObservingConn {
        inner: Box::new(conn),
        sent: Arc::clone(&sent),
        received: Arc::clone(&received),
    });

    let (mut rx, mut tx) = wrapped.split();

    block_on(async {
        // Feed one request in; drain one response out.
        ends.request_tx
            .send(Frame::Message(Bytes::from_static(b"req")))
            .await
            .unwrap();
        let got = rx.receive().await.unwrap();
        assert_eq!(got, Some(Bytes::from_static(b"req")));
        tx.send(Bytes::from_static(b"res")).await.unwrap();
        let out = ends.response_rx.receive().await.unwrap();
        assert!(matches!(out, Frame::Message(b) if &b[..] == b"res"));
    });

    assert_eq!(received.load(Ordering::SeqCst), 1, "observed inbound frame");
    assert_eq!(sent.load(Ordering::SeqCst), 1, "observed outbound frame");
}

// ── panic recovery ────────────────────────────────────────────────────────────

#[test]
fn panic_in_handler_becomes_internal_via_recover() {
    let handler: UnaryFunc = Arc::new(|_ctx, _call| {
        Box::pin(async move {
            panic!("handler boom");
        })
    });

    let recover = RecoverInterceptor::new(
        |_ctx: &Ctx, _spec: &Spec, _headers: &SimpleHeaders, payload: Box<dyn std::any::Any + Send>| {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| (*s).to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "panic".to_string());
            foundation_connectrpc::ConnectError::internal(format!("recovered: {msg}"))
        },
    );
    let composed = recover.wrap_unary(handler);

    let outcome = block_on(composed(
        ctx(),
        UnaryCall {
            headers: SimpleHeaders::new(),
            codec_name: "proto".to_string(),
            frame: Bytes::new(),
        },
    ));
    let trace = outcome.err().expect("panic must surface as an error");
    assert_eq!(
        trace.current_context().code(),
        foundation_connectrpc::Code::Internal
    );
    assert!(trace.current_context().message().contains("handler boom"));
}
