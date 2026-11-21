use ui::action::{ActionRegistry, ActionResult, Context, Interaction, RegistrationError};

#[ui::action(host, web)]
async fn focus(_ctx: &Context, _event: ui::action::Interaction<()>) -> ActionResult {
    Ok(())
}

#[cfg(target_family = "wasm")]
#[ui::action]
async fn increment(ctx: &Context, event: Interaction<u32>) -> ActionResult {
    let local_count = std::rc::Rc::new(event.state);
    std::future::ready(()).await;
    ctx.receiver()
        .state(event.node_id, &local_count.saturating_add(1))
}

#[ui::action]
async fn clear(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

pub fn register(registry: &mut ActionRegistry) -> Result<(), RegistrationError> {
    registry.register(focus)?;
    #[cfg(target_family = "wasm")]
    registry.register(increment)?;
    registry.register(clear)
}

pub fn render_form(ctx: &Context) -> ActionResult {
    ui::html! { ctx, ctx.receiver(), <Button on:tap={focus}>Focus</Button> };
    ui::html_add! { "main", ctx, ctx.receiver(), <Text>Message</Text> }?;
    Ok(())
}
