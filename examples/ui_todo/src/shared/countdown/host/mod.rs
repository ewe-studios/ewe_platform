//! Declare the countdown controls and actions for both roles.
use super::TodoId;
use foundation_ui_runtime::{
    action,
    action::{ActionResult, Context, Interaction},
    html,
    view::template_types::Html,
};
use serde::Deserialize;

foundation_ui_runtime::__ewe_action_host! {
    @items
    mod implementation;
    pub(crate) use implementation::install_host;
    pub(in crate::shared) use implementation::{apply, restore_view};
}

#[derive(Deserialize)]
#[allow(
    dead_code,
    reason = "The server retains the host action input declaration."
)]
pub(crate) struct Tick {
    session: Option<TodoId>,
    #[serde(default)]
    refresh: bool,
}

pub(crate) fn button(enabled: bool) -> Html {
    html! { <Button id="todo-pause-button" enabled={enabled} on:tap={pause_todo_input}>Pause input for one minute</Button> }
}

#[action(host)]
pub(in crate::shared) async fn pause_todo_input(
    ctx: &Context,
    _event: Interaction<()>,
) -> ActionResult {
    implementation::pause(ctx).await
}

#[action(host)]
pub(in crate::shared) async fn tick_todo_countdown(
    ctx: &Context,
    event: Interaction<(), Tick>,
) -> ActionResult {
    implementation::tick(ctx, event.input_value).await
}
