use foundation_context::{Ctx, MonotonicClock};
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll, Waker};
use ui::action::{ActionRegistry, Context, LookupError, Origin, RawInput, RawInteraction, Surface};

#[test]
fn wrong_surface_is_rejected_before_the_server_handler_runs() {
    let mut registry = ActionRegistry::new();
    renamed_action_consumer::register(&mut registry).unwrap();
    assert!(matches!(
        registry.get(Origin::Server, "add_todo", Surface::Native),
        Err(LookupError::UnsupportedAction)
    ));
    assert!(registry
        .lookup(Origin::Server, "add_todo", Surface::Web)
        .is_ok());
    for (surface, available) in [
        (Surface::Web, cfg!(feature = "browser")),
        (Surface::Native, cfg!(feature = "mobile")),
    ] {
        assert_eq!(
            registry
                .lookup(Origin::Host, "focus", surface)
                .unwrap()
                .local,
            available
        );
    }
    let handler = registry.get(Origin::Server, "add_todo", Surface::Web);
    assert_eq!(handler.is_ok(), cfg!(feature = "server"));
    let Ok(handler) = handler else { return };
    let context = Context::for_action(
        Ctx::fixed(Arc::new(MonotonicClock::new())),
        Surface::Web,
        None,
    );
    let event = RawInteraction {
        node_id: 0,
        event_name: "submit".into(),
        input_value: RawInput::Missing,
        state_body: None,
    };
    let mut future = handler.call_raw(&context, event);
    assert!(matches!(
        future
            .as_mut()
            .poll(&mut TaskContext::from_waker(Waker::noop())),
        Poll::Ready(Ok(()))
    ));
    drop(future);
    assert_eq!(context.into_receiver().drain().len(), 1);
}

#[test]
fn browser_routes_the_server_declaration() {
    assert!(renamed_action_consumer::submit_browser_todo());
}
