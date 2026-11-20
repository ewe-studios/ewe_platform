use foundation_ui_runtime::action::{
    Action, ActionFuture, ActionRef, ActionRegistry, Context, Interaction, LookupError, Origin,
    RegistrationError, Scope, Surface,
};
use std::sync::{Arc, Mutex};

/// A nullable state still requires a verified body. Only unit state can be absent.
struct Remember(Arc<Mutex<Vec<Interaction<Option<String>>>>>);

impl Action for Remember {
    type State = Option<String>;
    const NAME: &'static str = "remember";
    fn origin() -> Option<Origin> {
        None
    }
    fn scope() -> Scope {
        Scope::All
    }
    fn call<'a>(&'a self, _: &'a Context, event: Interaction<Self::State>) -> ActionFuture<'a> {
        Box::pin(async move {
            self.0.lock().unwrap().push(event);
            Ok(())
        })
    }
}

struct Notify;
impl Action for Notify {
    type State = ();
    const NAME: &'static str = "notify";
    fn origin() -> Option<Origin> {
        Some(Origin::Host)
    }
    fn scope() -> Scope {
        Scope::Mobile
    }
    fn call<'a>(&'a self, _: &'a Context, _: Interaction<()>) -> ActionFuture<'a> {
        Box::pin(async { Ok(()) })
    }
}

#[test]
fn invalid_references_are_rejected_before_lookup() {
    for invalid in [
        "",
        "host",
        "notify",
        "server:",
        "js:notify",
        "host:a:b",
        "host: a",
    ] {
        assert_eq!(
            ActionRef::parse(invalid),
            Err(LookupError::InvalidReference)
        );
    }
    let reference = ActionRef::parse("server:add_todo").unwrap();
    assert_eq!(reference.origin, Origin::Server);
    assert_eq!(reference.name, "add_todo");
}

#[test]
fn disjoint_scopes_share_one_name_and_overlaps_leave_both_entries_intact() {
    let mut registry = ActionRegistry::new();
    registry.register(Notify).unwrap();
    registry
        .declare(Some(Origin::Host), "notify", Scope::Web)
        .unwrap();
    assert!(
        !registry
            .lookup(Origin::Host, "notify", Surface::Web)
            .unwrap()
            .local
    );
    registry.register_page("notify").unwrap();
    assert_eq!(
        registry.declare(None, "notify", Scope::All),
        Err(RegistrationError::Duplicate)
    );
    assert_eq!(
        registry.register_page("notify"),
        Err(RegistrationError::Duplicate)
    );
    assert_eq!(registry.register(Notify), Err(RegistrationError::Duplicate));
    assert!(
        registry
            .lookup(Origin::Host, "notify", Surface::Web)
            .unwrap()
            .local
    );
    assert_eq!(
        registry
            .get(Origin::Host, "notify", Surface::Native)
            .unwrap()
            .name(),
        "notify"
    );
    assert_eq!(
        registry.lookup(Origin::Server, "notify", Surface::Web),
        Err(LookupError::UnavailableOrigin)
    );
}

#[test]
fn both_origins_select_one_unqualified_entry() {
    let mut registry = ActionRegistry::new();
    registry.declare(None, "remember", Scope::All).unwrap();
    registry.register(Remember(Arc::default())).unwrap();
    for origin in [Origin::Host, Origin::Server] {
        for surface in [Surface::Web, Surface::Native] {
            let entry = registry.get(origin, "remember", surface).unwrap();
            assert_eq!(entry.origin(), None);
            assert_eq!(entry.scope(), Scope::All);
            assert!(entry.requires_state());
        }
    }
}

#[test]
fn lookup_reports_unknown_name_and_wrong_surface() {
    let mut registry = ActionRegistry::new();
    registry.register(Notify).unwrap();
    assert_eq!(
        registry.lookup(Origin::Host, "new", Surface::Web),
        Err(LookupError::UnknownAction)
    );
    assert_eq!(
        registry.lookup(Origin::Host, "notify", Surface::Web),
        Err(LookupError::UnsupportedAction)
    );
    assert_eq!(
        registry.declare(None, "bad:name", Scope::All),
        Err(RegistrationError::InvalidName)
    );
}

#[cfg(not(target_family = "wasm"))]
#[foundation_core::valtron::valtron_test]
async fn adapter_decodes_verified_state_before_it_calls_the_action() {
    use crate::support::{test_ctx, WALL};
    use foundation_ui_runtime::action::RawInteraction;
    use foundation_ui_wire::ErrorCode;
    let mut registry = ActionRegistry::new();
    let calls = Arc::default();
    registry.register(Remember(Arc::clone(&calls))).unwrap();
    registry.register(Notify).unwrap();
    let ctx = Context::for_render(test_ctx(WALL).0);
    let event = |body: Option<&[u8]>| RawInteraction {
        node_id: 42,
        event_name: "submit".into(),
        input_value: "Buy milk".into(),
        state_body: body.map(<[u8]>::to_vec),
    };
    let action = registry
        .get(Origin::Host, "remember", Surface::Web)
        .unwrap();
    for body in [None, Some(&b"5"[..]), Some(&b""[..])] {
        let error = action.call_raw(&ctx, event(body)).await.unwrap_err();
        assert_eq!(error.current_context().code, ErrorCode::StateRejected);
    }
    action
        .call_raw(&ctx, event(Some(br#""my list""#)))
        .await
        .unwrap();
    action.call_raw(&ctx, event(Some(b"null"))).await.unwrap();
    {
        let seen = calls.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].state.as_deref(), Some("my list"));
        assert_eq!(seen[0].node_id, 42);
        assert_eq!(seen[0].event_name, "submit");
        assert_eq!(seen[0].input_value, "Buy milk");
        assert_eq!(seen[1].state, None);
    }
    let unit = registry
        .get(Origin::Host, "notify", Surface::Native)
        .unwrap();
    assert!(!unit.requires_state());
    unit.call_raw(&ctx, event(None)).await.unwrap();
    assert!(unit.call_raw(&ctx, event(Some(b"42"))).await.is_err());
}
