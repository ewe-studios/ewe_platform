//! A separate replica can verify and update the state on a control.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use foundation_ui_runtime::action;
use foundation_ui_runtime::action::{ActionRegistry, ActionResult, Context, Interaction, Origin};
use foundation_ui_runtime::dispatch::{Connection, Dispatcher, DispatcherBuilder, RenderTarget};
use foundation_ui_runtime::state::{
    install_state_signer, sign_state, verify_state, with_state_binding, HmacStateSigner,
    StateConfig, StateError, StateSigner,
};
use foundation_ui_wire::writer::ClientCaps;
use foundation_ui_wire::{
    read_frame, MountSurface, OperationT, SetTextT, UiOperationT, WireLimits,
};

use super::{
    dispatch_support::request,
    support::{test_ctx, WALL},
};

struct CountingSigner {
    inner: HmacStateSigner,
    writes: AtomicUsize,
}

impl StateSigner for CountingSigner {
    fn sign(&self, ctx: &Ctx, binding: Option<&[u8]>, body: &[u8]) -> Result<Vec<u8>, StateError> {
        self.writes.fetch_add(1, Ordering::Relaxed);
        self.inner.sign(ctx, binding, body)
    }

    fn verify(
        &self,
        ctx: &Ctx,
        binding: Option<&[u8]>,
        signed: &[u8],
    ) -> Result<Vec<u8>, StateError> {
        self.inner.verify(ctx, binding, signed)
    }
}

#[action(server)]
async fn increment(ctx: &Context, event: Interaction<u32>) -> ActionResult {
    let root = ctx
        .state_root()
        .expect("the host selected a component root");
    let next = event.state + 1;
    let mut text = SetTextT::default();
    text.node_id = root;
    text.text = Some(next.to_string());
    let mut op = OperationT::default();
    op.ui = UiOperationT::SetText(Box::new(text));
    ctx.receiver().render_op(op);
    ctx.receiver().state_for(&[root, event.node_id], &next)
}

fn replica() -> (Dispatcher, Ctx, Arc<CountingSigner>) {
    let (base, _) = test_ctx(WALL);
    let signer = Arc::new(CountingSigner {
        inner: HmacStateSigner::new([17; 32], StateConfig::default()).unwrap(),
        writes: AtomicUsize::new(0),
    });
    install_state_signer(&base, signer.clone());
    let ctx = with_state_binding(&base, b"alice").unwrap();
    ctx.store(RenderTarget(40));
    let mut registry = ActionRegistry::new();
    registry.register(increment).unwrap();
    (
        DispatcherBuilder::new(Origin::Server, Arc::new(registry)).build(),
        ctx,
        signer,
    )
}

#[valtron_test]
async fn independent_replicas_sign_once_per_render_and_accept_each_others_state() {
    let (first, ctx_a, signer_a) = replica();
    let (second, ctx_b, signer_b) = replica();
    let connection = Connection::stateless(ClientCaps::web_current(), MountSurface::Web, 7);
    let mut bytes = sign_state(&ctx_a, b"1").unwrap();
    for (host, ctx, seq, expected) in [(&second, &ctx_b, 1, "2"), (&first, &ctx_a, 2, "3")] {
        let input = request("server:increment", 42, Some(bytes), seq);
        let reply = host.dispatch(ctx, &connection, &input).await;
        let frame = read_frame(&reply, &WireLimits::default()).unwrap();
        assert_eq!(
            (
                frame.stream_id(),
                frame.sequence_id(),
                frame.kind_as_frame_message().unwrap().ack()
            ),
            (7, seq, seq)
        );
        let messages = frame.kind_as_frame_message().unwrap().messages().unwrap();
        assert_eq!(messages.len(), 3);
        let text = messages
            .get(0)
            .operation()
            .unwrap()
            .ui_as_set_text()
            .unwrap();
        assert_eq!((text.node_id(), text.text()), (40, Some(expected)));
        let root = messages
            .get(1)
            .operation()
            .unwrap()
            .ui_as_attach_state()
            .unwrap();
        let input = messages
            .get(2)
            .operation()
            .unwrap()
            .ui_as_attach_state()
            .unwrap();
        assert_eq!((root.root_node(), input.root_node()), (40, 42));
        bytes = input.state().unwrap().bytes().to_vec();
        assert_eq!(root.state().unwrap().bytes(), bytes);
        assert_eq!(verify_state(ctx, &bytes).unwrap(), expected.as_bytes());
    }
    assert_eq!(signer_b.writes.load(Ordering::Relaxed), 1);
    assert_eq!(signer_a.writes.load(Ordering::Relaxed), 2);
}

#[action(server)]
async fn remember(ctx: &Context, event: Interaction<u32>) -> ActionResult {
    let root = ctx.state_root().unwrap();
    ctx.receiver().state(root, &event.state)?;
    // Keep the first receiver alive while the second user finishes.
    let mut paused = false;
    std::future::poll_fn(|task| {
        if paused {
            return std::task::Poll::Ready(());
        }
        paused = true;
        task.waker().wake_by_ref();
        std::task::Poll::Pending
    })
    .await;
    Ok(())
}

#[valtron_test]
async fn concurrent_requests_keep_their_bindings_and_render_targets() {
    use std::future::Future;

    let (_, alice, _) = replica();
    let bob = with_state_binding(&alice, b"bob").unwrap();
    bob.store(RenderTarget(80));
    let mut registry = ActionRegistry::new();
    registry.register(remember).unwrap();
    let host = DispatcherBuilder::new(Origin::Server, Arc::new(registry)).build();
    let connection = Connection::stateless(ClientCaps::web_current(), MountSurface::Web, 7);
    let a = request(
        "server:remember",
        42,
        Some(sign_state(&alice, b"1").unwrap()),
        1,
    );
    let b = request(
        "server:remember",
        82,
        Some(sign_state(&bob, b"2").unwrap()),
        2,
    );
    let mut first = std::pin::pin!(host.dispatch(&alice, &connection, &a));
    {
        let mut task = std::task::Context::from_waker(std::task::Waker::noop());
        assert!(first.as_mut().poll(&mut task).is_pending());
    }
    let second = host.dispatch(&bob, &connection, &b).await;
    for (reply, ctx, other, target, body) in [
        (second, &bob, &alice, 80, b"2"),
        (first.await, &alice, &bob, 40, b"1"),
    ] {
        let reply = read_frame(&reply, &WireLimits::default()).unwrap();
        let messages = reply.kind_as_frame_message().unwrap().messages().unwrap();
        assert_eq!(messages.len(), 1);
        let attachment = messages
            .get(0)
            .operation()
            .unwrap()
            .ui_as_attach_state()
            .unwrap();
        assert_eq!(attachment.root_node(), target);
        let signed = attachment.state().unwrap().bytes();
        assert_eq!(verify_state(ctx, signed).unwrap(), body);
        assert!(verify_state(other, signed).is_err());
    }
}

#[valtron_test]
async fn persistent_host_selects_each_root_from_its_own_control_bindings() {
    let (_, ctx, _) = replica();
    let mut registry = ActionRegistry::new();
    registry.register(remember).unwrap();
    let host = DispatcherBuilder::new(Origin::Server, Arc::new(registry))
        .render_target(|_, name, node| match (name, node) {
            ("remember", 42) => Some(40),
            ("remember", 82) => Some(80),
            _ => None,
        })
        .build();
    let connection = Connection::stateless(ClientCaps::web_current(), MountSurface::Web, 7);
    for (trigger, root) in [(42, 40), (82, 80)] {
        let input = request(
            "server:remember",
            trigger,
            Some(sign_state(&ctx, b"1").unwrap()),
            1,
        );
        let reply = host.dispatch(&ctx, &connection, &input).await;
        let frame = read_frame(&reply, &WireLimits::default()).unwrap();
        let messages = frame.kind_as_frame_message().unwrap().messages().unwrap();
        let attachment = messages
            .get(0)
            .operation()
            .unwrap()
            .ui_as_attach_state()
            .unwrap();
        assert_eq!(attachment.root_node(), root);
    }
}
