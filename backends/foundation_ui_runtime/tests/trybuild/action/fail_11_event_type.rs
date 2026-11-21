use foundation_ui_runtime::action;

#[action]
async fn submit(_ctx: &Context, _event: String) -> ActionResult {
    Ok(())
}

fn main() {}
