use foundation_context::Ctx;
use foundation_ui_native::{CoreError, EventInput, NativeCore, NativePlatform};
use foundation_ui_runtime::{
    action::{ActionError, ActionRegistry, ActionResult, Context, Interaction},
    action, html,
    view::{full_view, ViewIds},
};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

uniffi::setup_scaffolding!();

fn counter(ctx: &Context) -> Result<Arc<AtomicU64>, foundation_errstacks::ErrorTrace<ActionError>> {
    ctx.get::<AtomicU64>().ok_or_else(|| foundation_errstacks::ErrorTrace::new(
        ActionError::new(foundation_ui_wire::ErrorCode::ActionFailed, "Missing counter"),
    ))
}

fn screen(ctx: &Context, count: u64) -> ActionResult {
    let view = html! { <Stack>{% if COMPOSED %}<Text>{app_identity::TITLE.trim()}</Text>{% endif %}<Text>{count.to_string()}</Text>
        <Button on:tap={increment}>Increment</Button></Stack> };
    full_view(ctx, ctx.receiver(), &view, None::<&()>)
}

#[action(host)]
async fn show(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    screen(ctx, counter(ctx)?.load(Ordering::Relaxed))
}

#[action(host)]
async fn increment(ctx: &Context, _event: Interaction<()>) -> ActionResult {
    let count = counter(ctx)?.fetch_add(1, Ordering::Relaxed);
    screen(ctx, count.wrapping_add(1))
}

#[uniffi::export]
pub fn create_counter_core(platform: NativePlatform) -> Result<Arc<NativeCore>, CoreError> {
    let ctx = Ctx::host();
    ctx.store(AtomicU64::new(0));
    ctx.store(ViewIds::range(1, u32::MAX));
    let mut actions = ActionRegistry::new();
    actions.register(show).and_then(|()| actions.register(increment))
        .map_err(|e| CoreError::BadReference { detail: e.to_string() })?;
    Ok(NativeCore::builder(platform, ctx, actions).build())
}

#[uniffi::export]
pub async fn initial_counter_frame(core: Arc<NativeCore>) -> Result<Vec<u8>, CoreError> {
    core.trigger("host:show".into(), EventInput {
        node_id: 1, event_name: String::new(), value: String::new(), checked: false,
        parent_node: 0, state: Vec::new(),
    }).await?.ok_or_else(|| CoreError::Write { detail: "Missing counter frame".into() })
}
