use foundation_context::Ctx;
use foundation_core::valtron::valtron_test;
use foundation_ui_native::{
    CoreConfig, CoreError, EventInput, HostTransport, NativeCore, NativePlatform,
};
use foundation_ui_runtime::dispatch::RenderTarget;
use foundation_ui_wire::{
    read_frame, writer::WriterConfig, EventId, PropId, PropValueT, UiOperationT, WireLimits,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use ui_todo::{install_view_services, local_registry, PAGE_ROOT};
use ui_todo_native::{create_local_todo_core, initial_todo_frame};

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

fn shows_role(ops: &[UiOperationT], role: &str) -> bool {
    ops.iter().any(|op| {
        matches!(op, UiOperationT::SetProp(prop)
            if prop.prop == PropId::PRESENTATION
            && matches!(&prop.value, PropValueT::Text(value) if value.v.as_deref() == Some(role)))
    })
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
                parent_node: PAGE_ROOT,
                state: state(ops, node_id),
            },
        )
        .await
        .unwrap()
        .unwrap();
    operations(&update)
}

#[valtron_test]
async fn todo_actions_work_without_transport_and_keep_separate_stores() {
    for platform in [NativePlatform::Ios, NativePlatform::Android] {
        let ctx = Ctx::host();
        install_view_services(&ctx, 1 << 31, u32::MAX).unwrap();
        ctx.store(RenderTarget(PAGE_ROOT));
        let transport = Arc::new(FailTransport::default());
        let core = NativeCore::new(
            CoreConfig {
                platform,
                registry: Arc::new(local_registry(&ctx).unwrap()),
                ctx,
                writer: WriterConfig::default(),
            },
            Some(transport.clone()),
        );
        let first = operations(&initial_todo_frame(core.clone()).await.unwrap());
        assert!(shows(&first, "0 open todos"));
        assert!(shows_role(&first, "navigation-title"));
        assert!(shows_role(&first, "segmented-control"));

        let sheet = trigger(&core, &first, "host:open_add_dialog", "").await;
        assert!(shows_role(&sheet, "form-sheet"));
        let added = trigger(&core, &sheet, "host:add_local_todo", "Plan the week").await;
        assert!(shows(&added, "1 open todo"));
        assert!(shows(&added, "Plan the week"));
        assert!(shows_role(&added, "checklist-row"));

        let independent = create_local_todo_core(platform).unwrap();
        let other = operations(&initial_todo_frame(independent).await.unwrap());
        assert!(shows(&other, "0 open todos"));
        assert!(!shows(&other, "Plan the week"));

        let done = trigger(&core, &added, "host:toggle_local_todo", "1").await;
        assert!(shows(&done, "0 open todos"));
        assert!(shows(&done, "Reopen"));
        let filtered = trigger(&core, &done, "host:filter_todos", "done").await;
        assert!(shows(&filtered, "Plan the week"));
        let reopened = trigger(&core, &filtered, "host:toggle_local_todo", "1").await;
        assert!(shows(&reopened, "1 open todo"));
        assert!(!shows(&reopened, "Plan the week"));

        let open = trigger(&core, &reopened, "host:filter_todos", "open").await;
        let confirmation = trigger(&core, &open, "host:open_delete_dialog", "1").await;
        assert!(shows_role(&confirmation, "confirmation-dialog"));
        let deleted = trigger(&core, &confirmation, "host:confirm_local_delete", "1").await;
        assert!(shows(&deleted, "0 open todos"));
        assert!(!shows(&deleted, "Plan the week"));
        assert_eq!(transport.0.load(Ordering::Relaxed), 0);
    }
}
