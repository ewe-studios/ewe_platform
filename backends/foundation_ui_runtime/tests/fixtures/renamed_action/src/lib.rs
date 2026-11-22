use ui::action::{ActionRegistry, ActionResult, Context, Interaction, RegistrationError};

#[ui::action(host, web)]
async fn focus(ctx: &Context, _event: ui::action::Interaction<()>) -> ActionResult {
    ctx.receiver().data_in_view(browser_probe::MESSAGE.to_vec());
    Ok(())
}

#[ui::action(host, web)]
async fn increment(ctx: &Context, event: Interaction<u32>) -> ActionResult {
    let next = browser_probe::next_count(event.state).await;
    ctx.receiver().state(event.node_id, &next)
}

#[ui::action]
async fn clear(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

pub fn register(registry: &mut ActionRegistry) -> Result<(), RegistrationError> {
    registry.register(focus)?;
    registry.register(mobile::focus)?;
    registry.register(add_todo)?;
    registry.register(increment)?;
    registry.register(clear)
}

pub fn render_form(ctx: &Context) -> ActionResult {
    ui::html! { ctx, ctx.receiver(), <Stack><Button on:tap={focus}>Focus</Button><Button on:tap={add_todo}>Add</Button></Stack> };
    ui::html_add! { "main", ctx, ctx.receiver(), <Text>Message</Text> }?;
    Ok(())
}

mod mobile {
    use ui::action::{ActionResult, Context, Interaction};

    #[ui::action(host, mobile)]
    pub(super) async fn focus(ctx: &Context, _event: Interaction<()>) -> ActionResult {
        ctx.receiver().data_in_view(mobile_probe::MESSAGE.to_vec());
        Ok(())
    }
}

#[ui::action(server, web)]
async fn add_todo(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    ctx.receiver().data_in_view(server_probe::MESSAGE.to_vec());
    Ok(())
}

/// Expose the registry to the linker in each release library.
#[no_mangle]
pub extern "C" fn available_action_count() -> usize {
    use ui::action::{Origin, Surface};
    let mut registry = ActionRegistry::new();
    register(&mut registry).expect("the declarations have disjoint scopes");
    [
        (Origin::Host, "focus", Surface::Web),
        (Origin::Host, "focus", Surface::Native),
        (Origin::Server, "add_todo", Surface::Web),
    ]
    .into_iter()
    .filter(|(origin, name, surface)| registry.get(*origin, name, *surface).is_ok())
    .count()
}

/// Route a browser event through the registered declaration and read the submitted frame.
#[no_mangle]
pub extern "C" fn submit_browser_todo() -> bool {
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll, Waker};
    use ui::action::{ActionManager, ActionTransport, Surface, TransportFuture, TriggerEvent};
    struct Server;
    impl ActionTransport for Server {
        fn send(&self, frame: Vec<u8>) -> TransportFuture<'_> {
            Box::pin(std::future::ready(Ok(Some(frame))))
        }
    }
    let mut registry = ActionRegistry::new();
    register(&mut registry).unwrap();
    let manager = ActionManager::new(Arc::new(registry), Surface::Web, 1, Default::default())
        .with_server(Arc::new(Server));
    let event = TriggerEvent {
        node_id: 7,
        input: Default::default(),
        state: vec![],
    };
    let mut future = std::pin::pin!(manager.trigger("server:add_todo", event));
    let Poll::Ready(Ok(Some(bytes))) = future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    else {
        return false;
    };
    let Ok(frame) = foundation_ui_wire::read_frame(&bytes, &Default::default()) else {
        return false;
    };
    frame
        .kind_as_frame_request()
        .and_then(|request| request.action())
        .and_then(|action| action.action())
        == Some("server:add_todo")
}
