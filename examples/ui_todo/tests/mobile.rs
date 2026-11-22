use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use foundation_ui_native::{
    CoreConfig, CoreError, EventInput, HostTransport, NativeCore, NativePlatform,
};
use foundation_ui_wire::{
    read_frame, writer::WriterConfig, EventId, NodeKind, UiOperationT, WireLimits,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use ui_todo::{create_local_todo_core, initial_todo_frame};
use ui_todo::{host_registry, install_cache, install_view_services};

#[derive(Default)]
struct FailTransport(AtomicUsize);

#[async_trait::async_trait]
impl HostTransport for FailTransport {
    async fn send(&self, _request: Vec<u8>) -> Result<Option<Vec<u8>>, CoreError> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Err(CoreError::Transport {
            detail: "Unexpected request".into(),
        })
    }
}

fn operations(frame: &[u8]) -> Vec<UiOperationT> {
    fn collect(op: UiOperationT, found: &mut Vec<UiOperationT>) {
        match op {
            UiOperationT::ReplaceSubtree(tree) => {
                for child in tree.operations.unwrap_or_default() {
                    collect(child.ui, found);
                }
            }
            UiOperationT::Place(tree) => {
                for child in tree.operations.unwrap_or_default() {
                    collect(child.ui, found);
                }
            }
            UiOperationT::LiveMount(_) => panic!("Unexpected live subscription"),
            _ => found.push(op),
        }
    }
    let frame = read_frame(frame, &WireLimits::default()).unwrap();
    let body = frame.kind_as_frame_message().unwrap().unpack();
    let mut found = Vec::new();
    for message in body.messages.unwrap() {
        assert!(message.control.is_none(), "{:?}", message.control);
        if let Some(op) = message.operation {
            collect(op.ui, &mut found);
        }
    }
    found
}

fn shows(ops: &[UiOperationT], text: &str) -> bool {
    ops.iter()
        .any(|op| matches!(op, UiOperationT::SetText(value) if value.text.as_deref() == Some(text)))
}

fn shows_control(ops: &[UiOperationT], kind: NodeKind) -> bool {
    ops.iter()
        .any(|op| matches!(op, UiOperationT::CreateNode(node) if node.kind == kind))
}

fn binding(ops: &[UiOperationT], action: &str) -> (u32, EventId) {
    let found = ops.iter().find_map(|op| match op {
        UiOperationT::BindAction(binding) if binding.action.as_deref() == Some(action) => {
            Some((binding.node_id, binding.event))
        }
        _ => None,
    });
    found.unwrap_or_else(|| panic!("Missing action {action}"))
}

fn state(ops: &[UiOperationT], node: u32) -> Vec<u8> {
    ops.iter()
        .find_map(|op| match op {
            UiOperationT::AttachState(attach) if attach.root_node == node => attach.state.clone(),
            _ => None,
        })
        .unwrap_or_default()
}

async fn trigger(
    core: &NativeCore,
    ops: &[UiOperationT],
    action: &str,
    value: &str,
) -> Vec<UiOperationT> {
    let (node_id, event) = binding(ops, action);
    let update = core
        .trigger(
            action.into(),
            EventInput {
                node_id,
                event_name: event.variant_name().unwrap().to_ascii_lowercase(),
                value: value.into(),
                checked: false,
                parent_node: 0,
                state: state(ops, node_id),
            },
        )
        .await
        .unwrap()
        .unwrap();
    operations(&update)
}

#[valtron_test]
async fn dialogs_use_one_server_binding_and_cancel_without_transport() {
    for platform in [NativePlatform::Ios, NativePlatform::Android] {
        let ctx = Ctx::host();
        install_view_services(&ctx, foundation_ui_runtime::action::Origin::Host).unwrap();
        install_cache(&ctx);
        let transport = Arc::new(FailTransport::default());
        let core = NativeCore::new(
            CoreConfig {
                platform,
                registry: Arc::new(host_registry().unwrap()),
                ctx,
                writer: WriterConfig::default(),
            },
            Some(transport.clone()),
        );
        let first = operations(&initial_todo_frame(core.clone()).await.unwrap());
        assert!(shows(&first, "0 open todos"));
        assert!(shows_control(&first, NodeKind::NAVIGATION_TITLE));
        assert!(shows_control(&first, NodeKind::SEGMENTED_CONTROL));

        let sheet = trigger(&core, &first, "host:open_add_dialog", "").await;
        for expected in [NodeKind::MODAL, NodeKind::BOTTOM_SHEET] {
            assert_eq!(
                sheet
                    .iter()
                    .filter(
                        |op| matches!(op, UiOperationT::CreateNode(node) if node.kind == expected)
                    )
                    .count(),
                1,
            );
        }
        let _ = binding(&sheet, "server:add_todo");
        let closed = trigger(&core, &sheet, "host:close_dialog", "").await;
        assert!(!shows_control(&closed, NodeKind::MODAL));
        assert_eq!(transport.0.load(Ordering::Relaxed), 0);
        let independent = create_local_todo_core(platform).unwrap();
        let other = operations(&initial_todo_frame(independent).await.unwrap());
        assert!(shows(&other, "0 open todos"));
    }
}

#[valtron_test]
async fn a_failed_addition_survives_restart_and_reuses_its_creation_id() {
    #[derive(Default)]
    struct Offline(std::sync::Mutex<Vec<serde_json::Value>>);
    #[async_trait::async_trait]
    impl HostTransport for Offline {
        async fn send(&self, request: Vec<u8>) -> Result<Option<Vec<u8>>, CoreError> {
            let frame = read_frame(&request, &WireLimits::default()).unwrap();
            let action = frame.kind_as_frame_request().unwrap().action().unwrap();
            let payload = action.input().unwrap().json().unwrap().bytes();
            self.0
                .lock()
                .unwrap()
                .push(serde_json::from_slice(payload).unwrap());
            Err(CoreError::Transport {
                detail: "Offline".into(),
            })
        }
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pending.json");
    let transport = Arc::new(Offline::default());
    #[derive(Default)]
    struct Preview(std::sync::Mutex<Vec<Vec<u8>>>);
    impl foundation_ui_native::FrameListener for Preview {
        fn on_frame(&self, frame: Vec<u8>) {
            self.0.lock().unwrap().push(frame);
        }
    }
    let preview = Arc::new(Preview::default());
    let open = || {
        let core = ui_todo::create_persistent_todo_core(
            NativePlatform::Ios,
            transport.clone(),
            path.to_str().unwrap().into(),
        )
        .unwrap();
        core.set_frame_listener(preview.clone());
        core
    };
    let first = open();
    let ops = operations(&initial_todo_frame(first.clone()).await.unwrap());
    let dialog = trigger(&first, &ops, "host:open_add_dialog", "").await;
    let (node_id, _) = binding(&dialog, "server:add_todo");
    let event = EventInput {
        node_id,
        event_name: "submit".into(),
        value: "Keep this title".into(),
        ..EventInput::receiving_view()
    };
    let response = first.trigger("server:add_todo".into(), event).await;
    assert!(response.is_err() || response.unwrap().is_some());
    assert!(preview
        .0
        .lock()
        .unwrap()
        .iter()
        .any(|frame| shows(&operations(frame), "Pending")));
    let payload = transport.0.lock().unwrap()[0].clone();
    let key = payload["creation_id"].as_str().unwrap();
    assert_eq!(payload["title"], "Keep this title");
    assert!(path.is_file());
    drop(first);
    let next = open();
    let ops = operations(&initial_todo_frame(next.clone()).await.unwrap());
    assert!(shows(&ops, "Keep this title"));
    assert!(shows(&ops, "0 open todos"));
    assert!(shows(&ops, "Try again"));
    let (node_id, _) = binding(&ops, "server:add_todo");
    let retry = EventInput {
        node_id,
        event_name: "tap".into(),
        value: format!("retry-add:{key}"),
        ..EventInput::receiving_view()
    };
    let response = next.trigger("server:add_todo".into(), retry).await;
    assert!(response.is_err() || response.unwrap().is_some());
    assert_eq!(
        transport.0.lock().unwrap().as_slice(),
        &[payload.clone(), payload]
    );
}
