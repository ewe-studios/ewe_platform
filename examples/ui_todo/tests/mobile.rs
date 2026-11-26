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
async fn dialogs_use_one_host_binding_and_cancel_without_transport() {
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
        let _ = binding(&sheet, "host:submit_todo");
        let closed = trigger(&core, &sheet, "host:close_dialog", "").await;
        assert!(!shows_control(&closed, NodeKind::MODAL));
        assert_eq!(transport.0.load(Ordering::Relaxed), 0);
        let independent = create_local_todo_core(platform).unwrap();
        let other = operations(&initial_todo_frame(independent).await.unwrap());
        assert!(shows(&other, "0 open todos"));
    }
}

#[valtron_test]
async fn a_failed_addition_survives_restart_and_reuses_its_id() {
    #[derive(Default)]
    struct Offline(std::sync::Mutex<Vec<serde_json::Value>>);
    #[async_trait::async_trait]
    impl HostTransport for Offline {
        async fn send(&self, request: Vec<u8>) -> Result<Option<Vec<u8>>, CoreError> {
            let frame = read_frame(&request, &WireLimits::default()).unwrap();
            let action = frame.kind_as_frame_request().unwrap().action().unwrap();
            let payload = action.input().unwrap().json().unwrap().bytes();
            let mut requests = self.0.lock().unwrap();
            requests.push(serde_json::from_slice(payload).unwrap());
            if requests.len() == 2 {
                let mut error = foundation_ui_wire::ActionErrorT::default();
                error.code = foundation_ui_wire::ErrorCode::InvalidInput;
                error.message = Some("Choose another title".into());
                let header = foundation_ui_wire::writer::FrameHeader {
                    stream_id: frame.stream_id(),
                    sequence_id: 1,
                    ack: frame.sequence_id(),
                };
                return Ok(Some(
                    foundation_ui_wire::writer::write_error(
                        &error,
                        header,
                        &WriterConfig::default(),
                    )
                    .unwrap(),
                ));
            }
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
        use foundation_ui_wire::{writer::{write_caps, FrameHeader}, ServerActionT, ServerCapsT, SCHEMA_VERSION};
        let mut caps = ServerCapsT::default();
        caps.schema_version = SCHEMA_VERSION;
        caps.vocabulary_version = foundation_ui_wire::vocab::VOCABULARY_VERSION;
        let mut add = ServerActionT::default();
        add.name = Some("add_todo".into());
        caps.actions = Some(vec![add]);
        core.apply_caps_frame(write_caps(&caps, FrameHeader { stream_id: 1, sequence_id: 1, ack: 0 }, &WriterConfig::default()).unwrap()).unwrap();
        core.set_frame_listener(preview.clone());
        core
    };
    let first = open();
    let ops = operations(&initial_todo_frame(first.clone()).await.unwrap());
    let dialog = trigger(&first, &ops, "host:open_add_dialog", "").await;
    let (node_id, _) = binding(&dialog, "host:submit_todo");
    let event = EventInput {
        node_id,
        event_name: "submit".into(),
        value: "Keep this title".into(),
        ..EventInput::receiving_view()
    };
    let response = first.trigger("host:submit_todo".into(), event).await;
    assert!(response.is_err() || response.unwrap().is_some());
    assert!(preview
        .0
        .lock()
        .unwrap()
        .iter()
        .any(|frame| shows(&operations(frame), "Pending")));
    let payload = transport.0.lock().unwrap()[0].clone();
    let key = payload["id"].as_str().unwrap();
    assert_eq!(payload["title"], "Keep this title");
    assert!(path.with_added_extension("sqlite3").is_file());
    assert!(!path.exists());
    drop(first);
    let next = open();
    let ops = operations(&initial_todo_frame(next.clone()).await.unwrap());
    assert!(shows(&ops, "Keep this title"));
    assert!(shows(&ops, "0 open todos"));
    assert!(shows(&ops, "Try again"));
    for (query, visible) in [("Other", false), (" KeEp ", true), ("", true)] {
        let rows = trigger(&next, &ops, "host:search_todos", query).await;
        assert_eq!(shows(&rows, "Keep this title"), visible);
        assert_eq!(shows(&rows, "Try again"), visible);
    }
    let (node_id, _) = binding(&ops, "host:retry_todo");
    let retry = EventInput {
        node_id,
        event_name: "tap".into(),
        value: format!("add:{key}"),
        ..EventInput::receiving_view()
    };
    let response = next.trigger("host:retry_todo".into(), retry).await;
    assert!(response.is_err() || response.unwrap().is_some());
    assert_eq!(
        transport.0.lock().unwrap().as_slice(),
        &[payload.clone(), payload.clone()]
    );
    drop(next);
    let corrected = open();
    let first = operations(&initial_todo_frame(corrected.clone()).await.unwrap());
    assert!(shows(&first, "Choose another title"));
    assert!(shows(&first, "Edit title"));
    let sheet = trigger(
        &corrected,
        &first,
        "host:open_add_dialog",
        &format!("correct-add:{key}"),
    )
    .await;
    let (node_id, _) = binding(&sheet, "host:submit_todo");
    let event = EventInput {
        node_id,
        event_name: "submit".into(),
        value: "Corrected title".into(),
        ..EventInput::receiving_view()
    };
    let reply = corrected.trigger("host:submit_todo".into(), event).await.unwrap().unwrap();
    assert!(shows(&operations(&reply), "Try again"));
    let corrected_key = {
        let requests = transport.0.lock().unwrap();
        assert_eq!(requests[2]["title"], "Corrected title");
        assert_ne!(requests[2]["id"], payload["id"]);
        format!("pending/add:{}", requests[2]["id"].as_str().unwrap())
    };
    use foundation_db::KeyValueStore;
    let database = pending_database(&path).await;
    let keys = database
        .list_keys(Some("pending/"))
        .await
        .unwrap()
        .collect_all()
        .await
        .unwrap();
    assert_eq!(keys, [corrected_key]);
}

#[valtron_test]
async fn confirming_our_deletion_removes_only_its_frozen_search_row() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pending.json");
    let first = "00000000-0000-4000-8000-000000000001";
    let second = "00000000-0000-4000-8000-000000000002";
    let removal = ui_todo::Pending {
        change: ui_todo::PendingChange::Delete {
            id: first.parse().unwrap(),
        },
        retry: false,
        validation: None,
    };
    std::fs::write(
        &path,
        serde_json::to_vec(&std::collections::BTreeMap::from([(
            format!("delete:{first}"),
            removal,
        )]))
        .unwrap(),
    )
    .unwrap();
    let core = ui_todo::create_persistent_todo_core(
        NativePlatform::Android,
        Arc::new(FailTransport::default()),
        path.to_str().unwrap().into(),
    )
    .unwrap();
    async fn deliver(core: &NativeCore, data: serde_json::Value) -> Vec<UiOperationT> {
        let bytes = rmp_serde::to_vec_named(&("ewe.todo.v1", data)).unwrap();
        let frame = core
            .trigger_structured(
                "host:apply_todo_delivery".into(),
                EventInput::receiving_view(),
                serde_json::to_vec(&bytes).unwrap(),
            )
            .await
            .unwrap()
            .unwrap();
        operations(&frame)
    }
    let rows = serde_json::json!([
        {"id": first, "text": "Task one", "done": false},
        {"id": second, "text": "Task two", "done": false}
    ]);
    deliver(
        &core,
        serde_json::json!({"Snapshot":{"position":2,"items":rows}}),
    )
    .await;
    let mut input = EventInput::receiving_view();
    input.value = "Task".into();
    let frame = core
        .trigger("host:search_todos".into(), input)
        .await
        .unwrap()
        .unwrap();
    let search = operations(&frame);
    assert!(shows(&search, "Deleting…"));
    assert!(shows(&search, "Task two"));
    let unrelated = deliver(
        &core,
        serde_json::json!({"Committed":{"position":3,"change":{"Remove":second}}}),
    )
    .await;
    assert!(!shows(&unrelated, "Task one"));
    let confirmed = deliver(
        &core,
        serde_json::json!({"Committed":{"position":4,"change":{"Remove":first}}}),
    )
    .await;
    assert!(!shows(&confirmed, "Task one"));
    assert!(shows(&confirmed, "Task two"));
    assert!(shows(&confirmed, "0 open todos"));
    use foundation_db::KeyValueStore;
    assert!(path.is_file());
    let database = pending_database(&path).await;
    assert!(!database
        .exists(&format!("pending/delete:{first}"))
        .await
        .unwrap());
    assert!(database
        .exists(&format!("imported/v1/delete:{first}"))
        .await
        .unwrap());
    drop(core);
    let restarted = ui_todo::create_persistent_todo_core(
        NativePlatform::Android,
        Arc::new(FailTransport::default()),
        path.to_str().unwrap().into(),
    )
    .unwrap();
    let _ = initial_todo_frame(restarted).await.unwrap();
    assert!(!database
        .exists(&format!("pending/delete:{first}"))
        .await
        .unwrap());
}

#[derive(Default)]
struct LocalFrames(std::sync::Mutex<Vec<Vec<u8>>>);

impl foundation_ui_native::FrameListener for LocalFrames {
    fn on_frame(&self, frame: Vec<u8>) {
        self.0.lock().unwrap().push(frame);
    }
}

impl LocalFrames {
    async fn apply_data(&self, core: &NativeCore) -> Vec<UiOperationT> {
        let mut updates = Vec::new();
        let frames = std::mem::take(&mut *self.0.lock().unwrap());
        for frame in frames {
            let frame = read_frame(&frame, &WireLimits::default()).unwrap();
            for message in frame.kind_as_frame_message().unwrap().messages().unwrap() {
                if let Some(data) = message.data() {
                    let bytes = data.msgpack().unwrap().bytes().unwrap().bytes();
                    let update = core
                        .trigger_structured(
                            "host:apply_todo_delivery".into(),
                            EventInput {
                                node_id: data.target_node(),
                                ..EventInput::receiving_view()
                            },
                            serde_json::to_vec(bytes).unwrap(),
                        )
                        .await
                        .unwrap();
                    if let Some(frame) = update {
                        updates.extend(operations(&frame));
                    }
                }
            }
        }
        updates
    }
}

#[valtron_test]
async fn on_device_mutations_update_the_receiving_view() {
    use foundation_ui_wire::{PropId, PropValueT};

    for platform in [NativePlatform::Ios, NativePlatform::Android] {
        let core = create_local_todo_core(platform).unwrap();
        let frames = Arc::new(LocalFrames::default());
        core.set_frame_listener(frames.clone());
        let first = operations(&initial_todo_frame(core.clone()).await.unwrap());
        let dialog = trigger(&core, &first, "host:open_add_dialog", "").await;
        let _ = trigger(&core, &dialog, "host:submit_todo", "Buy milk").await;
        let added = frames.apply_data(&core).await;
        assert!(shows(&added, "Buy milk") && shows(&added, "1 open todo"));
        assert!(!shows(&added, "Pending"));
        let (node, _) = binding(&added, "server:toggle_todo");
        let id = added
            .iter()
            .find_map(|op| match op {
                UiOperationT::SetProp(prop)
                    if prop.node_id == node && prop.prop == PropId::VALUE =>
                {
                    match &prop.value {
                        PropValueT::Text(text) => text.v.clone(),
                        _ => None,
                    }
                }
                _ => None,
            })
            .unwrap();
        let reply = core
            .trigger(
                "server:toggle_todo".into(),
                EventInput {
                    node_id: node,
                    event_name: "tap".into(),
                    value: id.clone(),
                    ..EventInput::receiving_view()
                },
            )
            .await
            .unwrap()
            .unwrap();
        foundation_ui_native::FrameListener::on_frame(&*frames, reply);
        let completed = frames.apply_data(&core).await;
        assert!(shows(&completed, "Reopen") && shows(&completed, "0 open todos"));
        let dialog = trigger(&core, &completed, "host:open_delete_dialog", &id).await;
        let _ = trigger(&core, &dialog, "host:submit_delete", &id).await;
        let removed = frames.apply_data(&core).await;
        assert!(shows(&removed, "0 open todos") && !shows(&removed, "Buy milk"));
    }
}

async fn pending_database(path: &std::path::Path) -> foundation_db::LibsqlStore {
    foundation_db::LibsqlStore::new_kv(
        path.with_added_extension("sqlite3").to_str().unwrap(),
        None,
        Ctx::host(),
    )
    .await
    .unwrap()
}

#[valtron_test]
async fn concurrent_mobile_cores_keep_independent_pending_keys() {
    use futures_util::TryStreamExt;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pending.json");
    let open = || {
        let core = ui_todo::create_persistent_todo_core(
            NativePlatform::Android,
            Arc::new(FailTransport::default()),
            path.to_str().unwrap().into(),
        )
        .unwrap();
        core.set_frame_listener(Arc::new(LocalFrames::default()));
        core
    };
    let left = open();
    let right = open();
    let first = operations(&initial_todo_frame(left.clone()).await.unwrap());
    let second = operations(&initial_todo_frame(right.clone()).await.unwrap());
    let left_sheet = trigger(&left, &first, "host:open_add_dialog", "").await;
    let right_sheet = trigger(&right, &second, "host:open_add_dialog", "").await;
    futures_util::join!(
        trigger(&left, &left_sheet, "host:submit_todo", "Left addition"),
        trigger(&right, &right_sheet, "host:submit_todo", "Right addition"),
    );
    let db = pending_database(&path).await;
    let records: Vec<_> = db
        .records::<ui_todo::Pending>("pending/")
        .await
        .unwrap()
        .try_collect()
        .await
        .unwrap();
    assert_eq!(records.len(), 2);
    let left_id = records
        .iter()
        .find_map(|(_, entry)| match &entry.change {
            ui_todo::PendingChange::Add { id, title } if title == "Left addition" => {
                Some(id.to_string())
            }
            _ => None,
        })
        .unwrap();
    let change = serde_json::json!({"Committed":{"position":1,"change":{"Upsert":{"id":left_id,"text":"Left addition","done":false}}}});
    let bytes = rmp_serde::to_vec_named(&("ewe.todo.v1", change)).unwrap();
    left.trigger_structured(
        "host:apply_todo_delivery".into(),
        EventInput::receiving_view(),
        serde_json::to_vec(&bytes).unwrap(),
    )
    .await
    .unwrap();
    drop(left);
    drop(right);
    let restarted = operations(&initial_todo_frame(open()).await.unwrap());
    assert!(shows(&restarted, "Right addition"));
    assert!(shows(&restarted, "Try again"));
    assert!(!shows(&restarted, "Left addition"));
}

#[valtron_test]
async fn mobile_storage_uses_the_full_legacy_filename_for_each_namespace() {
    use foundation_db::KeyValueStore;
    let dir = tempfile::tempdir().unwrap();
    for (index, name) in ["records.json", "records.bak", "records.sqlite3"]
        .into_iter()
        .enumerate()
    {
        let path = dir.path().join(name);
        let id = format!("00000000-0000-4000-8000-{index:012}");
        let legacy = serde_json::to_vec(&serde_json::json!({
            (format!("add:{id}")): {"change":{"Add":{"id":id,"title":name}}, "retry": true}
        }))
        .unwrap();
        std::fs::write(&path, &legacy).unwrap();
        let core = ui_todo::create_persistent_todo_core(
            NativePlatform::Ios,
            Arc::new(FailTransport::default()),
            path.to_str().unwrap().into(),
        )
        .unwrap();
        let frame = operations(&initial_todo_frame(core).await.unwrap());
        assert!(shows(&frame, name));
        assert_eq!(std::fs::read(&path).unwrap(), legacy);
        let db = pending_database(&path).await;
        let keys = db
            .list_keys(Some("pending/"))
            .await
            .unwrap()
            .collect_all()
            .await
            .unwrap();
        assert_eq!(keys, [format!("pending/add:{id}")]);
    }
}
