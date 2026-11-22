//! Exercise the single-user store through the public server registry.
use foundation_context::Ctx;
use foundation_ui_runtime::action::{invoke_action, Origin};
use foundation_ui_wire::{ActionInputT, ActionRequestT, MountSurface};
use ui_todo::{install_view_services, server_registry, TodoStore};

#[test]
fn server_owns_mutations_and_replays_confirmed_deletions() {
    let ctx = Ctx::host();
    install_view_services(&ctx, Origin::Server).unwrap();
    ctx.store(TodoStore::default());
    let registry = server_registry().unwrap();
    let submit = |name: &str, value: &str| {
        let entry = registry
            .get(Origin::Server, name, MountSurface::Web)
            .unwrap();
        let mut input = ActionInputT::default();
        input.value = Some(value.to_owned());
        let mut req = ActionRequestT::default();
        req.input = Some(Box::new(input));
        futures_lite::future::block_on(invoke_action(&ctx, entry, MountSurface::Web, None, req))
    };
    assert!(submit("add_todo", " ").is_err());
    assert!(!submit("add_todo", "buy milk").unwrap().is_empty());
    let store = ctx.get::<TodoStore>().unwrap();
    let item = &store.snapshot().items[0];
    assert_eq!(item.text, "buy milk");
    assert!(!item.done);
    let id = item.id.to_string();
    submit("toggle_todo", &id).unwrap();
    assert!(store.snapshot().items[0].done);
    let deletion = submit("confirm_delete", &id).unwrap();
    assert_eq!(store.snapshot().items, Vec::new());
    let retry = submit("confirm_delete", &id).unwrap();
    let messages = |items| foundation_ui_runtime::action::prepare_messages(&ctx, items).unwrap();
    assert_eq!(messages(deletion), messages(retry));
    assert!(submit("confirm_delete", "999").is_err());
}

#[test]
fn receiving_cache_orders_and_deduplicates_confirmed_changes() {
    use foundation_ui_runtime::action::{prepare_messages, ActionRegistry};
    use foundation_ui_wire::{MessageT, UiOperationT};
    use ui_todo::{host_registry, install_cache};

    fn invoke(
        ctx: &Ctx,
        registry: &ActionRegistry,
        origin: Origin,
        surface: MountSurface,
        name: &str,
        input: ActionInputT,
    ) -> Vec<MessageT> {
        let mut req = ActionRequestT::default();
        req.input = Some(Box::new(input));
        let updates = futures_lite::future::block_on(invoke_action(
            ctx,
            registry.get(origin, name, surface).unwrap(),
            surface,
            None,
            req,
        ))
        .unwrap();
        prepare_messages(ctx, updates).unwrap()
    }

    let server = Ctx::host();
    install_cache(&server);
    let registry = server_registry().unwrap();
    let packets: Vec<_> = ["Alpha", "Bravo"]
        .into_iter()
        .map(|title| {
            let mut input = ActionInputT::default();
            input.value = Some(title.into());
            let messages = invoke(
                &server,
                &registry,
                Origin::Server,
                MountSurface::Web,
                "add_todo",
                input,
            );
            assert!(messages.iter().all(|message| message.operation.is_none()));
            let data = messages[0].data.as_ref().unwrap();
            assert_eq!(data.target_node, 0);
            data.msgpack.as_ref().unwrap().bytes.clone().unwrap()
        })
        .collect();

    for surface in [MountSurface::Web, MountSurface::Native] {
        let client = Ctx::host();
        install_cache(&client);
        install_view_services(&client, Origin::Host).unwrap();
        let registry = host_registry().unwrap();
        let deliver = |bytes: &[u8]| {
            let mut input = ActionInputT::default();
            input.json = Some(serde_json::to_vec(bytes).unwrap());
            invoke(
                &client,
                &registry,
                Origin::Host,
                surface,
                "apply_todo_delivery",
                input,
            )
        };
        assert!(deliver(&packets[1]).is_empty());
        let changes = deliver(&packets[0]);
        let targets: Vec<_> = changes
            .into_iter()
            .map(|message| {
                let UiOperationT::Place(place) = message.operation.unwrap().ui else {
                    panic!("expected a named placement");
                };
                place.target.unwrap().text.unwrap()
            })
            .collect();
        assert_eq!(targets, ["todo-list", "open-count"]);
        assert!(deliver(&packets[0]).is_empty());
        assert!(deliver(&packets[1]).is_empty());
        let state = client.get::<TodoStore>().unwrap().snapshot();
        assert_eq!(
            state
                .items
                .iter()
                .map(|row| row.text.as_str())
                .collect::<Vec<_>>(),
            ["Alpha", "Bravo"]
        );
    }
}

#[test]
fn creation_retries_match_the_original_payload_and_respect_deletion() {
    let ctx = Ctx::host();
    ctx.store(TodoStore::default());
    let registry = server_registry().unwrap();
    let call = |name: &str, input: ActionInputT| {
        let mut request = ActionRequestT::default();
        request.input = Some(Box::new(input));
        let action = registry
            .get(Origin::Server, name, MountSurface::Web)
            .unwrap();
        futures_lite::future::block_on(invoke_action(
            &ctx,
            action,
            MountSurface::Web,
            None,
            request,
        ))
    };
    let add = |key: &str, title: &str| {
        let mut input = ActionInputT::default();
        input.json = Some(
            serde_json::to_vec(&serde_json::json!({"creation_id": key, "title": title})).unwrap(),
        );
        call("add_todo", input)
    };
    let first = "00000000-0000-4000-8000-000000000001";
    let second = "00000000-0000-4000-8000-000000000002";
    assert!(add("bad key", "same").is_err());
    add(first, "same").unwrap();
    add(first, "same").unwrap();
    assert!(add(first, "different").is_err());
    add(second, "same").unwrap();
    let store = ctx.get::<TodoStore>().unwrap();
    let items = store.snapshot().items;
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].creation_id.as_deref(), Some(first));
    assert_eq!(items[1].creation_id.as_deref(), Some(second));
    let mut deletion = ActionInputT::default();
    deletion.value = Some(items[0].id.to_string());
    call("confirm_delete", deletion).unwrap();
    add(first, "same").unwrap();
    assert_eq!(store.snapshot().items, [items[1].clone()]);
}
