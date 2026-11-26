use foundation_ui_runtime::action;

#[action(host, web, mobile)]
async fn submit(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

fn main() {}
