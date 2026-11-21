use foundation_ui_runtime::action;
use foundation_ui_runtime::action::{Context, Interaction};

#[action]
async fn submit(_ctx: &Context, _event: Interaction<()>) -> Result<(), String> {
    Ok(())
}

fn main() {}
