//! Exercise the single-user store through the public server registry.
use foundation_context::Ctx;
use foundation_ui_runtime::action::{invoke_action, Origin};
use foundation_ui_wire::{ActionInputT, ActionRequestT, MountSurface};
use ui_todo::{install_view_services, server_registry, TodoStore, PAGE_ROOT};

#[test]
fn server_owns_mutations_and_rejects_stale_deletes() {
    let ctx = Ctx::host();
    install_view_services(&ctx, 100, 1000).unwrap();
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
        futures_lite::future::block_on(invoke_action(
            &ctx,
            entry,
            MountSurface::Web,
            Some(PAGE_ROOT),
            req,
        ))
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
    submit("confirm_delete", &id).unwrap();
    assert_eq!(store.snapshot().items, Vec::new());
    assert!(submit("confirm_delete", &id).is_err());
}
