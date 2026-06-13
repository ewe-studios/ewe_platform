//! WHY: Every program (and test) repeats the same five-object ritual —
//! signals runtime, root context, UI runtime with a protocol, attach,
//! receiver. Nothing varies but the protocol (spec-42 feature 03).
//!
//! WHAT: [`App`] — one struct owning all five, with protocol-named
//! constructors. THE DEFAULT is the COMPACT COLUMNAR wire (protocol byte 1,
//! VERSION 1 — our owned format, NOT Apache Arrow; the misleading
//! "arrow v1" naming was retired by review). Apache Arrow IPC is the same
//! protocol byte's VERSION 2: `arrow_ipc()`, on by default via the `arrow`
//! feature. `json`/`mock` are opt-in debugging/testing presets, never
//! defaults.
//!
//! HOW: One struct, named constructors over `with_protocol` — the protocol
//! is a VALUE choice behind the `ProtocolMethods` seam, not a type-level
//! property (distinct `ColumnarApp`/`JsonApp` types would force genericity
//! on every downstream signature for zero benefit). `App` owns the pieces,
//! so drop order is handled in one place: dropping the `App` disposes the
//! root context (tearing down effects) before the runtime goes away.

use alloc::rc::Rc;
use alloc::vec::Vec;

use foundation_signals::{Context, Runtime as SignalsRuntime};
use foundation_theme::GeneratedTheme;
use foundation_ui_traits::DomOp;
use foundation_wasm::MemoryAllocations;

use foundation_ui_traits::Html;

use crate::theme::{inject_theme_css, BODY_NODE_ID};

use crate::protocol::{ColumnarV1, FrameSink, FrameSinkV1, JsonV1, MockProtocol, ProtocolMethods};
use crate::protocol::CollectedFrames;
use foundation_ui_traits::ProtocolEncoder;
use crate::runtime::{Runtime, SharedInstructionReceiver};

/// The batches a [`MockProtocol`] captured — what `App::mock` hands back
/// for assertions.
pub type SentBatches = Rc<core::cell::RefCell<Vec<Vec<DomOp>>>>;

/// The five-object wiring as one value. See module docs.
pub struct App {
    signals: Rc<SignalsRuntime>,
    ctx: Context,
    runtime: Runtime,
    receiver: SharedInstructionReceiver,
    theme: Option<GeneratedTheme>,
}

impl App {
    /// THE DEFAULT — the compact columnar wire (protocol byte 1,
    /// VERSION 1): our owned format, zero-copy on the JS side.
    #[must_use]
    pub fn new() -> Self {
        Self::with_protocol(ColumnarV1::new())
    }

    /// Explicit alias of the default (compact columnar, wire VERSION 1).
    #[must_use]
    pub fn columnar() -> Self {
        Self::new()
    }

    /// Apache Arrow (IPC `RecordBatch`es — protocol byte 1, wire VERSION
    /// 2), read on the JS side with the embedded `apache-arrow.js`. The
    /// compact columnar default is ~7x faster/smaller on small UI deltas
    /// (see README §wire performance); pick arrow for bulk batches and
    /// ecosystem interop.
    #[cfg(feature = "arrow")]
    #[must_use]
    pub fn arrow() -> Self {
        Self::with_protocol(crate::protocol::ArrowIpcV2::new())
    }

    /// Human-readable JSON `DomOp`s — a DEBUGGING preset, never the default.
    #[must_use]
    pub fn json() -> Self {
        Self::with_protocol(JsonV1::new())
    }

    /// SERVER preset: the same compact columnar v1 wire, but every flushed
    /// batch lands as an envelope-framed `Vec<u8>` in the returned queue —
    /// ship them yourself (WebSocket binary frames, SSE events) toward a
    /// client `mount-stream`. On a native server `host_apply` is a no-op
    /// stub, so the normal presets would silently drop flushes; this is the
    /// preset that makes server-driven UIs real. First-paint HTML needs no
    /// App at all — `html! { … }.to_markup()` (see README §server).
    #[must_use]
    pub fn server() -> (Self, CollectedFrames) {
        let sink = FrameSinkV1::new();
        let frames = sink.frames();
        (Self::with_protocol(sink), frames)
    }

    /// Server preset over ANY Layer-1 encoder — whatever wire the clients
    /// negotiate (feature 04): `JsonEncoder` for debuggable streams,
    /// `foundation_arrow::ArrowIpcEncoder` for standard Arrow tooling.
    #[must_use]
    pub fn server_with<E>(encoder: E) -> (Self, CollectedFrames)
    where
        E: ProtocolEncoder<Vec<DomOp>> + 'static,
    {
        let sink = FrameSink::with_encoder(encoder);
        let frames = sink.frames();
        (Self::with_protocol(sink), frames)
    }

    /// `MockProtocol` wiring for tests, with the captured-batches handle.
    #[must_use]
    pub fn mock() -> (Self, SentBatches) {
        let mock = MockProtocol::new();
        let sent = mock.sent_batches();
        (Self::with_protocol(mock), sent)
    }

    /// Escape hatch: any `ProtocolMethods` implementation.
    #[must_use]
    pub fn with_protocol(protocol: impl ProtocolMethods<Vec<DomOp>> + 'static) -> Self {
        let signals = Rc::new(SignalsRuntime::new());
        let ctx = Context::new(Rc::clone(&signals));
        let runtime = Runtime::builder()
            .protocol(protocol)
            .memory(MemoryAllocations::new())
            .build();
        runtime.attach(&signals);
        let receiver = runtime.receiver();
        Self {
            signals,
            ctx,
            runtime,
            receiver,
            theme: None,
        }
    }

    /// Install a theme: the app takes ownership and IMMEDIATELY queues the
    /// stylesheet's head-injection ops (feature 09 §8 — theme CSS must reach
    /// `<head>` before the first content batch). Pairs with `theme!{}` or the
    /// runtime `Theme` builder, both of which yield a [`GeneratedTheme`].
    ///
    /// ```ignore
    /// let app = App::new().theme(theme! {
    ///     colors  { primary: { light: "#3b82f6", dark: "#60a5fa" } }
    ///     spacing { sm: 8px, md: 16px }
    /// });
    /// ```
    #[must_use]
    pub fn theme(mut self, theme: GeneratedTheme) -> Self {
        inject_theme_css(&self.receiver, theme.to_css());
        self.theme = Some(theme);
        self
    }

    /// The installed theme, if any.
    #[must_use]
    pub fn get_theme(&self) -> Option<&GeneratedTheme> {
        self.theme.as_ref()
    }

    /// The pair every reactive `html!` / component call needs — cheap
    /// `Rc`-backed clones.
    #[must_use]
    pub fn context(&self) -> (Context, SharedInstructionReceiver) {
        (self.ctx.clone(), self.receiver.clone())
    }

    /// The root ownership scope.
    #[must_use]
    pub fn ctx(&self) -> &Context {
        &self.ctx
    }

    /// The `DomOp` queue handle.
    #[must_use]
    pub fn receiver(&self) -> SharedInstructionReceiver {
        self.receiver.clone()
    }

    /// The reactive engine (stabilize, callback registration, …).
    #[must_use]
    pub fn signals(&self) -> &Rc<SignalsRuntime> {
        &self.signals
    }

    /// The UI runtime (protocol + memory).
    #[must_use]
    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Run dirty effects glitch-free; the attached runtime flushes queued
    /// `DomOp`s afterwards.
    pub fn stabilize(&self) {
        self.signals.stabilize();
    }

    /// A child ownership scope — component-scoped teardown without touching
    /// the root.
    #[must_use]
    pub fn scope(&self) -> Context {
        self.ctx.child()
    }

    /// Append a tree to the BOTTOM of the document `<body>` — the splice that
    /// makes a built (but detached) `html! {}` fragment part of the live
    /// document.
    ///
    /// `html! { ctx, rcv, … }` builds and registers a subtree but leaves it
    /// DETACHED (it has no parent); without this the JS runtime applies the
    /// create/append ops into a registry but nothing reaches the page. `mount`
    /// queues the single `AppendChild` onto the reserved [`BODY_NODE_ID`] the
    /// JS `NodeRegistry` seeds from `document.body`, then the next
    /// [`stabilize`](Self::stabilize) flushes it. Returns the mounted root id.
    ///
    /// `AppendChild` is LAST-CHILD semantics, so each `mount` lands at the
    /// current bottom of `<body>`, after everything mounted before it. That is
    /// exactly what you want for elements that must come after the body content
    /// is in place — e.g. a `<script>` that should run once the body is parsed
    /// and set up:
    ///
    /// ```ignore
    /// app.mount(content);                       // body content first…
    /// app.mount(html! { ctx, rcv, <script src="/boot.js"></script> }); // …then a late script
    /// ```
    ///
    /// This pairs with [`theme`](Self::theme), which injects into `<head>`
    /// (reserved [`HEAD_NODE_ID`](crate::HEAD_NODE_ID)): the startup instruction
    /// stream covers head AND the bottom of body. It is the same
    /// [`mount_fragment`](crate::mount_fragment) machinery `<Fragment>` uses — a
    /// root mount is just a fragment spliced under body.
    pub fn mount(&self, root: Html) -> u32 {
        crate::mount_fragment(&self.ctx, &self.receiver, root, BODY_NODE_ID)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl core::fmt::Debug for App {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("App { signals, ctx, runtime, receiver }")
    }
}
