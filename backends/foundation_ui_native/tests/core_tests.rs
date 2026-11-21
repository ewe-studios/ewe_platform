use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use foundation_ui_native::{
    command, CoreConfig, CoreError, EventInput, FrameListener, HostTransport, NativeCore,
    NativePlatform,
};
use foundation_ui_runtime::{
    action,
    action::{ActionRegistry, ActionResult, Context, Interaction, Origin, Scope},
    state::{install_state_signer, sign_state, HmacStateSigner, StateConfig},
};
use foundation_ui_wire::{
    finish_frame, ops, read_frame,
    writer::{write_caps, FrameHeader, WriterConfig},
    BehaviourId, FrameKindT, FrameMessageT, FrameT, MobileBehaviourT, MobileOperationT,
    MountSurface, NativeCommandId, OperationT, PlaceT, Platform, ServerActionT, ServerCapsT,
    UiBehaviourT, UiOperationT, WebOperationT, WireLimits, SCHEMA_VERSION,
};
use std::sync::{Arc, Mutex};

const COUNTER: &[u8] = include_bytes!("../../foundation_ui_wire/fixtures/frame_ops_counter.bin");

#[derive(Default)]
struct Recorder {
    frames: Mutex<Vec<Vec<u8>>>,
    requests: Mutex<Vec<Vec<u8>>>,
    reply: Mutex<ResultSlot>,
}
#[derive(Default)]
enum ResultSlot {
    #[default]
    Handoff,
    Frame(Vec<u8>),
    Fail(String),
}
impl FrameListener for Recorder {
    fn on_frame(&self, frame: Vec<u8>) {
        self.frames.lock().unwrap().push(frame);
    }
}
#[async_trait::async_trait]
impl HostTransport for Recorder {
    async fn send(&self, request: Vec<u8>) -> Result<Option<Vec<u8>>, CoreError> {
        self.requests.lock().unwrap().push(request);
        match &*self.reply.lock().unwrap() {
            ResultSlot::Handoff => Ok(None),
            ResultSlot::Frame(f) => Ok(Some(f.clone())),
            ResultSlot::Fail(e) => Err(CoreError::Transport { detail: e.clone() }),
        }
    }
}
fn event() -> EventInput {
    EventInput {
        node_id: 42,
        event_name: "tap".into(),
        value: "hello".into(),
        checked: true,
        parent_node: 40,
        state: vec![],
    }
}

fn web() -> OperationT {
    let mut op = OperationT::default();
    op.web = WebOperationT::WebText(Box::default());
    op
}
#[action(host)]
async fn local(ctx: &Context, event: Interaction<u32>) -> ActionResult {
    let mut waiting = true;
    std::future::poll_fn(|task| {
        if std::mem::take(&mut waiting) {
            task.waker().wake_by_ref();
            std::task::Poll::Pending
        } else {
            std::task::Poll::Ready(())
        }
    })
    .await;
    ctx.receiver().render_op(command::haptic());
    ctx.receiver().render_op(web());
    ctx.receiver()
        .render_op(ops::set_text(event.node_id, &event.state.to_string()));
    let mut shared = UiBehaviourT::default();
    shared.node_id = 42;
    shared.behaviour = BehaviourId::SCROLL_LOCK;
    let mut behavior = OperationT::default();
    behavior.ui = UiOperationT::UiBehaviour(Box::new(shared));
    let mut mobile = MobileBehaviourT::default();
    mobile.node_id = 42;
    mobile.custom = Some("pull_to_refresh".into());
    behavior.mobile = MobileOperationT::MobileBehaviour(Box::new(mobile));
    ctx.receiver().render_op(behavior);
    Ok(())
}

#[action(host)]
async fn local_numbers(ctx: &Context, event: Interaction<(), Vec<u32>>) -> ActionResult {
    ctx.store(event.input_value);
    Ok(())
}

fn core(host: Option<Arc<dyn HostTransport>>) -> (Ctx, Arc<NativeCore>) {
    let ctx = Ctx::host();
    install_state_signer(
        &ctx,
        Arc::new(HmacStateSigner::new([4; 32], StateConfig::default()).unwrap()),
    );
    let mut registry = ActionRegistry::new();
    registry.register(local).unwrap();
    registry.register(local_numbers).unwrap();
    registry.declare(None, "advertised", Scope::All).unwrap();
    registry
        .declare(Some(Origin::Host), "picker", Scope::Web)
        .unwrap();
    registry
        .declare(Some(Origin::Server), "save", Scope::All)
        .unwrap();
    let core = NativeCore::new(
        CoreConfig {
            ctx: ctx.clone(),
            registry: Arc::new(registry),
            platform: NativePlatform::Ios,
            writer: WriterConfig::default(),
        },
        host,
    );
    (ctx, core)
}

#[test]
fn hello_listener_and_command_helpers() {
    let (_, core) = core(None);
    let hello = core.hello();
    assert_eq!(hello, core.hello());
    let frame = read_frame(&hello, &WireLimits::default()).unwrap();
    let info = frame.kind_as_frame_request().unwrap().hello().unwrap();
    assert_eq!(
        (
            frame.stream_id(),
            frame.schema_version(),
            info.platform(),
            info.mount()
        ),
        (0, SCHEMA_VERSION, Platform::Ios, MountSurface::Native)
    );
    assert_eq!(core.push_ops(vec![]), Err(CoreError::NoListener));
    let host = Arc::new(Recorder::default());
    core.set_frame_listener(host.clone());
    assert!(core.push_ops(vec![OperationT::default()]).is_err());
    assert_eq!(
        core.push_ops(vec![command::open_url("https://example.org"), web()])
            .unwrap(),
        1
    );
    assert_eq!(core.push_ops(vec![command::share("milk")]).unwrap(), 2);
    core.clear_frame_listener();
    assert_eq!(core.push_ops(vec![]), Err(CoreError::NoListener));
    let frames = host.frames.lock().unwrap();
    for ((index, bytes), (command, name, text)) in frames.iter().enumerate().zip([
        (NativeCommandId::OPEN_URL, "url", "https://example.org"),
        (NativeCommandId::SHARE, "text", "milk"),
    ]) {
        let f = read_frame(bytes, &WireLimits::default()).unwrap();
        assert_eq!(f.sequence_id(), u64::try_from(index).unwrap() + 1);
        let body = f.kind_as_frame_message().unwrap();
        assert_eq!(body.ack(), 0);
        let messages = body.messages().unwrap();
        assert_eq!(messages.len(), 1);
        let op = messages
            .get(0)
            .operation()
            .unwrap()
            .mobile_as_native_command()
            .unwrap();
        let arg = op.args().unwrap().get(0);
        assert_eq!(
            (op.command(), arg.name(), arg.value_as_text().unwrap().v()),
            (command, Some(name), Some(text))
        );
    }
}

#[valtron_test]
async fn local_actions_wait_and_use_native_gate() {
    let host = Arc::new(Recorder::default());
    let (ctx, core) = core(Some(host.clone()));
    let mut input = event();
    input.state = sign_state(&ctx, b"7").unwrap();
    let bytes = core
        .trigger("host:local".into(), input)
        .await
        .unwrap()
        .unwrap();
    let frame = read_frame(&bytes, &WireLimits::default())
        .unwrap()
        .kind_as_frame_message()
        .unwrap();
    assert_eq!(frame.ack(), 1);
    let m = frame.messages().unwrap();
    assert_eq!(m.len(), 3);
    assert_eq!(
        m.get(0)
            .operation()
            .unwrap()
            .mobile_as_native_command()
            .unwrap()
            .command(),
        NativeCommandId::HAPTIC
    );
    assert_eq!(
        m.get(1)
            .operation()
            .unwrap()
            .ui_as_set_text()
            .unwrap()
            .text(),
        Some("7")
    );
    let behaviors = m.get(2).operation().unwrap();
    assert_eq!(
        behaviors.ui_as_ui_behaviour().unwrap().behaviour(),
        BehaviourId::SCROLL_LOCK
    );
    assert_eq!(
        behaviors.mobile_as_mobile_behaviour().unwrap().custom(),
        Some("pull_to_refresh")
    );
    assert_eq!(host.requests.lock().unwrap().len(), 0);
}

#[valtron_test]
async fn remote_returns_exact_bytes_handoff_and_timeout() {
    let host = Arc::new(Recorder::default());
    let core =
        NativeCore::remote(NativePlatform::Android, vec!["save".into()], host.clone()).unwrap();
    *host.reply.lock().unwrap() = ResultSlot::Frame(COUNTER.to_vec());
    let mut input = event();
    input.state = vec![1, 2, 3];
    assert_eq!(
        core.trigger("server:save".into(), input)
            .await
            .unwrap()
            .as_deref(),
        Some(COUNTER)
    );
    {
        let requests = host.requests.lock().unwrap();
        let action = read_frame(&requests[0], &WireLimits::default())
            .unwrap()
            .kind_as_frame_request()
            .unwrap()
            .action()
            .unwrap();
        assert_eq!(action.state().unwrap().bytes(), [1, 2, 3]);
        assert_eq!(
            (
                action.action(),
                action.node_id(),
                action.input().unwrap().value()
            ),
            (Some("server:save"), 42, Some("hello"))
        );
    }
    *host.reply.lock().unwrap() = ResultSlot::Handoff;
    assert_eq!(
        core.trigger("server:save".into(), event()).await.unwrap(),
        None
    );
    *host.reply.lock().unwrap() = ResultSlot::Fail("timeout".into());
    assert_eq!(
        core.trigger("server:save".into(), event()).await,
        Err(CoreError::Transport {
            detail: "timeout".into()
        })
    );
    assert_eq!(host.requests.lock().unwrap().len(), 3);
}
#[valtron_test]
async fn routes_and_validates_server_capabilities() {
    let (_, offline) = core(None);
    assert!(matches!(
        offline.trigger("bad".into(), event()).await,
        Err(CoreError::BadReference { .. })
    ));
    assert!(matches!(
        offline.trigger("host:missing".into(), event()).await,
        Err(CoreError::UnknownAction { .. })
    ));
    assert!(matches!(
        offline.trigger("host:picker".into(), event()).await,
        Err(CoreError::UnsupportedAction { .. })
    ));
    assert!(matches!(
        offline.trigger("server:save".into(), event()).await,
        Err(CoreError::UnavailableOrigin { .. })
    ));
    let host = Arc::new(Recorder::default());
    let (_, online) = core(Some(host.clone()));
    let mut entry = ServerActionT::default();
    entry.name = Some("advertised".into());
    let mut caps = ServerCapsT::default();
    caps.schema_version = SCHEMA_VERSION;
    caps.vocabulary_version = foundation_ui_wire::vocab::VOCABULARY_VERSION;
    caps.actions = Some(vec![entry]);
    let frame = write_caps(
        &caps,
        FrameHeader {
            stream_id: 7,
            ack: 0,
            sequence_id: 1,
        },
        &WriterConfig::default(),
    )
    .unwrap();
    online.apply_caps_frame(frame).unwrap();
    assert!(online.apply_caps_frame(online.hello()).is_err());
    online
        .trigger("host:advertised".into(), event())
        .await
        .unwrap();
    assert_eq!(host.requests.lock().unwrap().len(), 1);
}
#[valtron_test]
async fn rejects_invalid_remote_replies_and_web_children() {
    let host = Arc::new(Recorder::default());
    let core = NativeCore::remote(NativePlatform::Ios, vec!["save".into()], host.clone()).unwrap();
    let mut place = PlaceT::default();
    place.operations = Some(vec![web()]);
    let mut op = OperationT::default();
    op.ui = UiOperationT::Place(Box::new(place));
    let mut body = FrameMessageT::default();
    body.messages = Some(vec![ops::message(op)]);
    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.kind = FrameKindT::FrameMessage(Box::new(body));
    for bytes in [vec![], core.hello(), finish_frame(&frame).unwrap()] {
        *host.reply.lock().unwrap() = ResultSlot::Frame(bytes);
        assert!(matches!(
            core.trigger("server:save".into(), event()).await,
            Err(CoreError::Write { .. })
        ));
    }
}

#[cfg(feature = "app")]
#[test]
fn signal_sink_delivers_after_the_arena_is_released() {
    use foundation_ui_native::CoreSink;
    use foundation_wasm::{MemoryAllocations, ProtocolHandler};
    use foundation_wasm_ui::protocol::ProtocolMethods;
    let host = Arc::new(Recorder::default());
    let (_, core) = core(None);
    let sink = CoreSink::new(core.clone());
    let mut arena = MemoryAllocations::new();
    assert!(sink
        .encode_and_write(vec![command::haptic()], &mut arena)
        .is_err());
    core.set_frame_listener(host.clone());
    let (receipt, ptr, len) = sink
        .encode_and_write(vec![command::haptic()], &mut arena)
        .unwrap();
    assert_eq!(host.frames.lock().unwrap().len(), 0);
    sink.send_to_js(receipt.memory_id, ptr, len);
    let bytes = host.frames.lock().unwrap()[0].clone();
    let decoded = sink.handle_received(&bytes).unwrap();
    assert_eq!(decoded, vec![command::haptic()]);
    sink.send_to_js(receipt.memory_id, ptr, len);
    assert_eq!(host.frames.lock().unwrap().len(), 1);
}

#[test]
fn concurrent_and_reentrant_pushes_reach_the_listener_in_sequence() {
    use std::sync::{Barrier, Weak};
    struct Listener {
        core: Weak<NativeCore>,
        entered: Barrier,
        release: Barrier,
        seq: Mutex<Vec<u64>>,
    }
    impl FrameListener for Listener {
        fn on_frame(&self, bytes: Vec<u8>) {
            let seq = read_frame(&bytes, &WireLimits::default())
                .unwrap()
                .sequence_id();
            if seq == 1 {
                self.entered.wait();
                self.release.wait();
                assert_eq!(
                    self.core
                        .upgrade()
                        .unwrap()
                        .push_ops(vec![command::haptic()])
                        .unwrap(),
                    3
                );
            }
            self.seq.lock().unwrap().push(seq);
        }
    }
    let (_, core) = core(None);
    let host = Arc::new(Listener {
        core: Arc::downgrade(&core),
        entered: Barrier::new(2),
        release: Barrier::new(2),
        seq: Mutex::default(),
    });
    core.set_frame_listener(host.clone());
    std::thread::scope(|scope| {
        let one = scope.spawn(|| core.push_ops(vec![command::haptic()]).unwrap());
        host.entered.wait();
        let two = core.push_ops(vec![command::haptic()]).unwrap();
        host.release.wait();
        assert_eq!((one.join().unwrap(), two), (1, 2));
    });
    assert_eq!(*host.seq.lock().unwrap(), [1, 2, 3]);
}

#[cfg(feature = "app")]
#[test]
fn reversed_staged_sends_and_cancellation_have_no_sequence_gap() {
    use foundation_wasm::{MemoryAllocations, ProtocolHandler};
    use foundation_wasm_ui::protocol::ProtocolMethods;
    let (_, core) = core(None);
    let host = Arc::new(Recorder::default());
    core.set_frame_listener(host.clone());
    let sink = foundation_ui_native::CoreSink::new(core);
    let mut arena = MemoryAllocations::new();
    let (discard, _, _) = sink
        .encode_and_write(vec![command::haptic()], &mut arena)
        .unwrap();
    let (first, a, alen) = sink
        .encode_and_write(vec![command::open_url("https://example.org")], &mut arena)
        .unwrap();
    let (second, b, blen) = sink
        .encode_and_write(vec![command::share("News")], &mut arena)
        .unwrap();
    sink.ack(discard.memory_id, &mut arena);
    sink.send_to_js(second.memory_id, b, blen);
    sink.send_to_js(first.memory_id, a, alen);
    let seen: Vec<_> = host
        .frames
        .lock()
        .unwrap()
        .iter()
        .map(|bytes| {
            let f = read_frame(bytes, &WireLimits::default()).unwrap();
            let cmd = f
                .kind_as_frame_message()
                .unwrap()
                .messages()
                .unwrap()
                .get(0)
                .operation()
                .unwrap()
                .mobile_as_native_command()
                .unwrap();
            (f.sequence_id(), cmd.command())
        })
        .collect();
    assert_eq!(
        seen,
        [(1, NativeCommandId::SHARE), (2, NativeCommandId::OPEN_URL)]
    );
}

#[valtron_test]
async fn native_provider_selects_the_service_once_per_screen() {
    use foundation_ui_native::{HostTransportProvider, NativeSender, ServiceError};
    use std::sync::atomic::{AtomicUsize, Ordering};
    struct Provider(Arc<Recorder>, AtomicUsize);
    #[async_trait::async_trait]
    impl HostTransportProvider for Provider {
        async fn action_transport(
            &self,
            service: String,
        ) -> Result<Arc<NativeSender>, ServiceError> {
            self.1.fetch_add(1, Ordering::Relaxed);
            if service == "todos" {
                Ok(NativeSender::new(self.0.clone()))
            } else {
                Err(ServiceError::UnknownService { name: service })
            }
        }
    }

    let recorder = Arc::new(Recorder::default());
    let provider = Provider(recorder.clone(), AtomicUsize::new(0));
    let builder = || {
        let mut registry = ActionRegistry::new();
        registry
            .declare(Some(Origin::Server), "save", Scope::All)
            .unwrap();
        NativeCore::builder(NativePlatform::Android, Ctx::host(), registry)
    };
    assert!(matches!(
        builder().with_service(&provider, "missing".into()).await,
        Err(ServiceError::UnknownService { .. })
    ));
    assert!(recorder.requests.lock().unwrap().is_empty());
    let core = builder()
        .with_service(&provider, "todos".into())
        .await
        .unwrap()
        .build();
    let direct = builder().with_host_transport(recorder.clone()).build();
    for screen in [&core, &direct] {
        for _ in 0..2 {
            assert_eq!(
                screen.trigger("server:save".into(), event()).await.unwrap(),
                None
            );
        }
    }
    let sent = recorder.requests.lock().unwrap();
    assert_eq!(sent.len(), 4);
    assert_eq!(sent[0..2], sent[2..4]);
    assert_eq!(provider.1.load(Ordering::Relaxed), 2);
}

#[valtron_test]
async fn native_structured_submission_reaches_the_local_handler_and_the_selected_transport() {
    let host = Arc::new(Recorder::default());
    let (ctx, core) = core(Some(host.clone()));
    let json = b"[3,7]".to_vec();
    core.trigger_structured("host:local_numbers".into(), event(), json.clone())
        .await
        .unwrap();
    assert_eq!(ctx.get::<Vec<u32>>().unwrap().as_slice(), [3, 7]);
    assert_eq!(
        core.trigger_structured("server:save".into(), event(), json.clone())
            .await,
        Err(CoreError::UnsupportedInput)
    );
    let mut caps = ServerCapsT::default();
    let mut action = ServerActionT::default();
    action.name = Some("save".into());
    caps.actions = Some(vec![action]);
    caps.schema_version = SCHEMA_VERSION;
    caps.vocabulary_version = foundation_ui_wire::vocab::STRUCTURED_INPUT_VERSION;
    core.apply_caps_frame(
        write_caps(
            &caps,
            FrameHeader {
                stream_id: 7,
                ack: 0,
                sequence_id: 1,
            },
            &WriterConfig::default(),
        )
        .unwrap(),
    )
    .unwrap();
    core.trigger_structured("server:save".into(), event(), json.clone())
        .await
        .unwrap();
    let requests = host.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let received = read_frame(&requests[0], &WireLimits::default())
        .unwrap()
        .kind_as_frame_request()
        .unwrap()
        .action()
        .unwrap()
        .input()
        .unwrap();
    assert_eq!(received.json().unwrap().bytes(), json);
    assert_eq!(received.value(), None);
}
