//! Construct a local application, a capturing server receiver, or a custom frame transport.
use alloc::rc::Rc;
use alloc::vec::Vec;

use foundation_signals::{Context, Runtime as SignalsRuntime};
use foundation_theme::GeneratedTheme;
use foundation_ui_traits::Html;
use foundation_ui_wire::OperationT;

use crate::theme::{inject_theme_css, BODY_NODE_ID};

use crate::protocol::CollectedFrames;
use crate::protocol::{MockProtocol, ProtocolMethods};
use crate::runtime::{Runtime, SharedInstructionReceiver};

/// The batches a [`MockProtocol`] captured — what `App::mock` hands back
/// for assertions.
pub type SentBatches = Rc<core::cell::RefCell<Vec<Vec<OperationT>>>>;

/// The five-object wiring as one value. See module docs.
pub struct App {
    signals: Rc<SignalsRuntime>,
    ctx: Context,
    runtime: Runtime,
    receiver: SharedInstructionReceiver,
    theme: Option<GeneratedTheme>,
}

impl App {
    /// Send generated operations as one EWF1 frame per flush.
    #[must_use]
    pub fn new() -> Self {
        Self::frames()
    }

    /// Use one frame per signal update batch.
    #[must_use]
    pub fn frames() -> Self {
        Self::with_protocol(crate::protocol::FrameV1::new(0))
    }

    /// Collect complete EWF1 frames in the returned queue for server delivery.
    #[must_use]
    pub fn server() -> (Self, CollectedFrames) {
        let frames = CollectedFrames::default();
        let output = frames.clone();
        let sink =
            crate::protocol::FrameOutput::new(crate::protocol::FrameV1::new(0), move |bytes| {
                output.borrow_mut().push_back(bytes);
                Ok(())
            });
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
    pub fn with_protocol(protocol: impl ProtocolMethods<Vec<OperationT>> + 'static) -> Self {
        let signals = Rc::new(SignalsRuntime::new());
        let ctx = Context::new(Rc::clone(&signals));
        let runtime = Runtime::builder().protocol(protocol).global_arena().build();
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
    /// See the [runnable application scenarios](https://github.com/ewe-studios/ewe_platform/blob/master/backends/foundation_wasm_ui/docs/scenarios/apps-and-frame-operations.md).
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

    /// The generated operation queue.
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
    /// queued operations afterwards.
    ///
    /// # Errors
    /// Returns the first rejected batch from this signal runtime.
    pub fn stabilize(&self) -> Result<(), crate::protocol::ProtocolError> {
        self.signals.stabilize();
        self.receiver.take_error().map_or(Ok(()), Err)
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
    /// See the [runnable application scenarios](https://github.com/ewe-studios/ewe_platform/blob/master/backends/foundation_wasm_ui/docs/scenarios/apps-and-frame-operations.md).
    ///
    /// This pairs with [`theme`](Self::theme), which injects into `<head>`
    /// (reserved [`HEAD_NODE_ID`](crate::HEAD_NODE_ID)): the startup instruction
    /// stream covers head AND the bottom of body. It is the same
    /// [`mount_fragment`](crate::mount_fragment) machinery `<Fragment>` uses — a
    /// root mount is just a fragment spliced under body.
    #[expect(
        clippy::must_use_candidate,
        reason = "A caller can mount a component and discard its root identifier."
    )]
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
