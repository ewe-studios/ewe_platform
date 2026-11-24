//! Transports for live frames and raw markup.
use std::prelude::rust_2021::*;
use std::sync::{Arc, Mutex};

/// A live client connection a [`Broadcaster`] fans frames to. The transport
/// layer implements it — for example, the event stream in `foundation_http`. Each method returns
/// `false` when the connection is gone, so the broadcaster can drop it.
pub trait FrameTransport {
    /// Deliver a binary frame. SSE transports base64 it; WebSocket sends it raw.
    fn send_binary(&mut self, frame: &[u8]) -> bool;
    /// Deliver a TEXT frame (raw markup) — the HTML protocol.
    fn send_text(&mut self, text: &str) -> bool;
}

/// One channel frame: a binary frame, or raw markup (HTML).
enum Frame {
    Binary(Vec<u8>),
    Text(String),
}

impl Frame {
    /// Write this frame to a connection the way its protocol expects.
    fn write(&self, conn: &mut (dyn FrameTransport + Send)) -> bool {
        match self {
            Frame::Binary(bytes) => conn.send_binary(bytes),
            Frame::Text(text) => conn.send_text(text),
        }
    }
}

/// Owned connections + the frame backlog replayed to late joiners.
struct Inner {
    conns: Vec<Box<dyn FrameTransport + Send>>,
    backlog: Vec<Frame>,
}

/// Shared fan-out of server-driven UI connections. Cheap to clone (`Arc`) — the
/// server keeps one handle and shares another into its request `Ctx`.
#[derive(Clone)]
pub struct Broadcaster {
    inner: Arc<Mutex<Inner>>,
}

impl Broadcaster {
    /// An empty broadcaster.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                conns: Vec::new(),
                backlog: Vec::new(),
            })),
        }
    }

    /// A handle for publishing raw data to subscribers.
    #[must_use]
    pub fn sender(&self) -> BroadcastTx {
        BroadcastTx {
            inner: self.inner.clone(),
        }
    }

    /// Register a freshly-connected client, replaying the backlog into it so a
    /// late joiner catches up (kills the connect/stabilize race).
    ///
    /// # Panics
    /// Panics if an earlier operation poisoned the broadcaster lock.
    pub fn register(&self, mut conn: impl FrameTransport + Send + 'static) {
        let mut inner = self.inner.lock().expect("broadcaster poisoned");
        if inner.backlog.iter().all(|frame| frame.write(&mut conn)) {
            inner.conns.push(Box::new(conn));
        }
    }

    /// Drop all connections (teardown closes the sockets).
    ///
    /// # Panics
    /// Panics if an earlier operation poisoned the broadcaster lock.
    pub fn close(&self) {
        self.inner
            .lock()
            .expect("broadcaster poisoned")
            .conns
            .clear();
    }

    /// Number of frames pushed so far (diagnostics).
    ///
    /// # Panics
    /// Panics if an earlier operation poisoned the broadcaster lock.
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.inner
            .lock()
            .expect("broadcaster poisoned")
            .backlog
            .len()
    }

    /// Number of currently-connected clients (diagnostics).
    ///
    /// # Panics
    /// Panics if an earlier operation poisoned the broadcaster lock.
    #[must_use]
    pub fn conn_count(&self) -> usize {
        self.inner.lock().expect("broadcaster poisoned").conns.len()
    }
}

impl Default for Broadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// A `Send` write handle: push a frame to every connected client (and the
/// backlog). Cheap to clone (`Arc`).
#[derive(Clone)]
pub struct BroadcastTx {
    inner: Arc<Mutex<Inner>>,
}

impl BroadcastTx {
    /// Deliver one binary frame. Its bytes identify its format.
    pub fn send(&self, frame: &[u8]) {
        self.push(Frame::Binary(frame.to_vec()));
    }

    /// Deliver one TEXT frame (raw markup) — the HTML protocol, routed through
    /// the runtime's `routeHtml` (materialize / island morph).
    pub fn send_text(&self, markup: &str) {
        self.push(Frame::Text(markup.to_string()));
    }

    fn push(&self, frame: Frame) {
        let mut inner = self.inner.lock().expect("broadcaster poisoned");
        inner.conns.retain_mut(|conn| frame.write(conn.as_mut()));
        inner.backlog.push(frame);
    }
}

mod live;
pub use live::{BacklogConfig, LiveChannel, LiveError, LivePublication, LiveTx};
mod live_sink;
pub use live_sink::FrameBroadcastSink;
