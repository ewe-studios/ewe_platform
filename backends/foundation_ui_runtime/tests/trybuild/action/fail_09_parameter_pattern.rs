use foundation_ui_runtime::action;

#[action]
async fn submit(_: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

fn main() {}
