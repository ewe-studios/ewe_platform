use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use foundation_context::Ctx;
use foundation_ui_runtime::action::{encode_updates, Context};
use foundation_ui_runtime::state::{
    install_state_signer, verify_state, with_state_binding, HmacStateSigner, StateConfig,
    StateError, StateSigner,
};
use foundation_ui_wire::writer::{ClientCaps, FrameHeader, WriterConfig};
use foundation_ui_wire::{
    read_frame, MountSurface, OperationT, Platform, SetTextT, UiOperationT, WireLimits,
};

use super::support::{test_ctx, WALL};

const HEADER: FrameHeader = FrameHeader {
    stream_id: 7,
    sequence_id: 11,
    ack: 10,
};

struct CountingSigner {
    inner: HmacStateSigner,
    calls: AtomicUsize,
}

impl StateSigner for CountingSigner {
    fn sign(&self, ctx: &Ctx, binding: Option<&[u8]>, body: &[u8]) -> Result<Vec<u8>, StateError> {
        self.calls.fetch_add(1, Ordering::Relaxed);
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

#[test]
fn render_keeps_order_and_signs_each_state_once_for_all_its_nodes() {
    let (base, _) = test_ctx(WALL);
    let signer = Arc::new(CountingSigner {
        inner: HmacStateSigner::new([3; 32], StateConfig::default()).unwrap(),
        calls: AtomicUsize::new(0),
    });
    install_state_signer(&base, signer.clone());
    let ctx = with_state_binding(&base, b"Alice").unwrap();
    let other = with_state_binding(&base, b"Bob").unwrap();
    let page = Context::for_render_on(ctx.clone(), MountSurface::Native);
    assert_eq!(page.surface(), MountSurface::Native);
    assert_eq!(page.state_root(), None);
    let mut text = SetTextT::default();
    text.node_id = 40;
    text.text = Some("Buy milk".into());
    let mut operation = OperationT::default();
    operation.ui = UiOperationT::SetText(Box::new(text));
    page.receiver().render_op(operation);
    page.receiver()
        .state_for(&[40, 42, 40], &["Buy milk"])
        .unwrap();
    page.receiver().data(43, vec![0x2a]);
    assert_eq!(signer.calls.load(Ordering::Relaxed), 0);
    let bytes = encode_updates(
        &ctx,
        page.into_receiver().drain(),
        &ClientCaps::native_current(Platform::Ios),
        HEADER,
        &WriterConfig::default(),
    )
    .unwrap();
    let frame = read_frame(&bytes, &WireLimits::default()).unwrap();
    assert_eq!(frame.stream_id(), HEADER.stream_id);
    assert_eq!(frame.sequence_id(), HEADER.sequence_id);
    let body = frame.kind_as_frame_message().unwrap();
    assert_eq!(body.ack(), HEADER.ack);
    let messages = body.messages().unwrap();
    assert_eq!(messages.len(), 4);
    let view = messages
        .get(0)
        .operation()
        .unwrap()
        .ui_as_set_text()
        .unwrap();
    assert_eq!((view.node_id(), view.text()), (40, Some("Buy milk")));
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
    let attachment = root.state().unwrap().bytes();
    assert_eq!(input.state().unwrap().bytes(), attachment);
    assert_eq!(signer.calls.load(Ordering::Relaxed), 1);
    assert_eq!(verify_state(&ctx, attachment).unwrap(), br#"["Buy milk"]"#);
    assert!(verify_state(&other, attachment).is_err());
    let data = messages.get(3).data().unwrap();
    assert_eq!(data.target_node(), 43);
    let blob = data.msgpack().unwrap();
    assert_eq!(blob.target_node(), 43);
    assert_eq!(blob.bytes().unwrap().bytes(), &[0x2a]);
}

#[test]
fn overlapping_calls_share_services_and_discard_only_their_own_output() {
    let (ctx, _) = test_ctx(WALL);
    let failed = Context::for_action(ctx.clone(), MountSurface::Web, Some(40));
    let live = Context::for_action(ctx.clone(), MountSurface::Web, Some(50));
    failed.store(123_u32);
    assert_eq!(*ctx.get::<u32>().unwrap(), 123);
    assert_eq!(*live.get::<u32>().unwrap(), 123);
    assert_eq!(live.state_root(), Some(50));
    failed.receiver().data(40, vec![1]);
    live.receiver().data(50, vec![2]);
    drop(failed);
    let bytes = encode_updates(
        &ctx,
        live.into_receiver().drain(),
        &ClientCaps::web_current(),
        HEADER,
        &WriterConfig::default(),
    )
    .unwrap();
    let messages = read_frame(&bytes, &WireLimits::default())
        .unwrap()
        .kind_as_frame_message()
        .unwrap()
        .messages()
        .unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages.get(0).data().unwrap().target_node(), 50);
}

#[test]
fn signing_or_writer_failure_returns_no_partial_frame() {
    let (ctx, _) = test_ctx(WALL);
    let page = Context::for_render(ctx.clone());
    page.receiver().data(40, vec![1]);
    page.receiver().state(40, &2).unwrap();
    assert!(encode_updates(
        &ctx,
        page.into_receiver().drain(),
        &ClientCaps::web_current(),
        HEADER,
        &WriterConfig::default()
    )
    .is_err());
    let page = Context::for_render(ctx.clone());
    page.receiver().data(40, vec![1]);
    page.receiver().render_op(OperationT::default());
    assert!(encode_updates(
        &ctx,
        page.into_receiver().drain(),
        &ClientCaps::web_current(),
        HEADER,
        &WriterConfig::default()
    )
    .is_err());
}

#[test]
fn direct_host_access_shares_entries_and_preserves_derived_contexts() {
    let (host, _) = test_ctx(WALL);
    let ctx = Context::for_render(host.clone());
    assert!(ctx.get::<String>().is_none());
    let first = Arc::new(String::from("first"));
    assert!(ctx.insert(first.clone()).is_none());
    assert!(Arc::ptr_eq(&host.get::<String>().unwrap(), &first));
    assert!(Arc::ptr_eq(&ctx.get_or_insert(String::new()), &first));
    assert!(Arc::ptr_eq(
        &ctx.get_and_store(String::from("next")).unwrap(),
        &first
    ));
    ctx.set(2u64);
    host.store(3u64);
    assert_eq!(ctx.get_cloned::<u64>(), Some(3));
    ctx.store(4u64);
    let fork: Ctx = ctx.fork();
    assert_eq!(fork.get_cloned::<u64>(), Some(4));
    assert_eq!(ctx.remove::<u64>().as_deref(), Some(&4));
    assert!(!host.contains::<u64>());
    assert!(fork.contains::<u64>());
    let empty: Ctx = ctx.with_empty_map();
    assert!(!empty.contains::<String>());
    let (other, _) = test_ctx(WALL);
    let rebased: Ctx = ctx.rebase(&other);
    assert!(rebased.contains::<String>());
    assert!(Arc::ptr_eq(&rebased.clock(), &other.clock()));
    let clocked: Ctx = ctx.with_clock(other.clock());
    assert!(Arc::ptr_eq(&clocked.clock(), &other.clock()));
    assert!(Arc::ptr_eq(&ctx.try_clock().unwrap(), &host.clock()));
    assert!(Arc::ptr_eq(&ctx.clock(), &host.clock()));
    assert!(Arc::ptr_eq(&ctx.config(), &host.config()));
    assert!(Arc::ptr_eq(&ctx.try_rng().unwrap(), &host.rng()));
    assert!(Arc::ptr_eq(&ctx.rng(), &host.rng()));
    ctx.assert_minimum();
}
