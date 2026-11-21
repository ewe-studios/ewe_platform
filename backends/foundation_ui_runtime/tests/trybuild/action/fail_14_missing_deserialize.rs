use foundation_ui_runtime::action;
use foundation_ui_runtime::action::{ActionResult, Context, Interaction};

#[derive(serde::Serialize)]
struct Form;

#[action]
async fn submit(_ctx: &Context, _event: Interaction<Form>) -> ActionResult {
    Ok(())
}

fn main() {}
