//! Exercise local actions and both forms of server capability delivery.
use foundation_core::valtron::valtron_test;
use foundation_ui_runtime::{
    action::{
        ActionRegistry, ActionResult, ActionTransport, Context, Interaction, TransportFuture,
        TriggerEvent,
    },
    network::NetworkApp,
};
use foundation_ui_wire::{
    read_frame,
    writer::{write_caps, ClientCaps, FrameHeader, WriterConfig},
    ActionInputT, MountSurface, ServerActionT, ServerCapsT, WireLimits, SCHEMA_VERSION,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};

#[foundation_ui_runtime::action(host)]
async fn count(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    ctx.get::<AtomicUsize>()
        .unwrap()
        .fetch_add(1, Ordering::Relaxed);
    Ok(())
}

#[derive(Default)]
struct Sender {
    requests: Mutex<Vec<Vec<u8>>>,
    next: Mutex<Option<Vec<u8>>>,
}

impl ActionTransport for Sender {
    fn send(&self, frame: Vec<u8>) -> TransportFuture<'_> {
        self.requests.lock().unwrap().push(frame);
        Box::pin(std::future::ready(Ok(self.next.lock().unwrap().take())))
    }
}

fn event() -> TriggerEvent {
    TriggerEvent {
        node_id: 40,
        input: ActionInputT::default(),
        state: Vec::new(),
    }
}

fn advertised() -> Vec<u8> {
    let mut action = ServerActionT::default();
    action.name = Some("save".into());
    let mut caps = ServerCapsT::default();
    caps.schema_version = SCHEMA_VERSION;
    caps.vocabulary_version = foundation_ui_wire::vocab::VOCABULARY_VERSION;
    caps.actions = Some(vec![action]);
    write_caps(
        &caps,
        FrameHeader {
            stream_id: 0,
            sequence_id: 1,
            ack: 1,
        },
        &WriterConfig::default(),
    )
    .unwrap()
}

#[valtron_test]
async fn local_services_and_remote_capabilities_share_one_client() {
    for immediate in [false, true] {
        let (ctx, _) = super::support::test_ctx(super::support::WALL);
        ctx.store(AtomicUsize::new(0));
        let mut actions = ActionRegistry::new();
        actions.register(count).unwrap();
        let sender = Arc::new(Sender::default());
        let app = NetworkApp::builder(
            ctx.clone(),
            actions,
            ClientCaps::web_current(),
            MountSurface::Web,
        )
        .with_server_transport(sender.clone())
        .build();
        assert!(app.trigger("server:save", event()).await.is_err());
        app.trigger("host:count", event()).await.unwrap();
        assert_eq!(ctx.get::<AtomicUsize>().unwrap().load(Ordering::Relaxed), 1);
        assert!(sender.requests.lock().unwrap().is_empty());
        if immediate {
            *sender.next.lock().unwrap() = Some(advertised());
        }
        app.connect().await.unwrap();
        {
            let sent = sender.requests.lock().unwrap();
            assert_eq!(sent.len(), 1);
            let frame = read_frame(&sent[0], &WireLimits::default()).unwrap();
            assert!(frame.kind_as_frame_request().unwrap().hello().is_some());
        }
        if !immediate {
            assert!(app.receive_capabilities(b"broken").is_err());
            assert!(app.trigger("server:save", event()).await.is_err());
            app.receive_capabilities(&advertised()).unwrap();
        }
        let bytes =
            include_bytes!("../../foundation_ui_wire/fixtures/frame_ops_counter.bin").to_vec();
        *sender.next.lock().unwrap() = Some(bytes.clone());
        assert_eq!(
            app.trigger("server:save", event()).await.unwrap(),
            Some(bytes)
        );
        assert!(app.trigger("server:save", event()).await.unwrap().is_none());
        *sender.next.lock().unwrap() = Some(vec![0]);
        assert!(app.trigger("server:save", event()).await.is_err());
        assert_eq!(sender.requests.lock().unwrap().len(), 4);
    }
}
