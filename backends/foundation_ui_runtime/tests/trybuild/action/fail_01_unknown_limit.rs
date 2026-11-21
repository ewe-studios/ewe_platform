use foundation_ui_runtime::action;

#[action(desktop)]
async fn submit(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

fn main() {}
