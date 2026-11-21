use foundation_ui_runtime::action;

#[action]
async fn submit(_ctx: &mut Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

fn main() {}
