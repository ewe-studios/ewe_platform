use foundation_ui_runtime::action;

#[action]
async fn submit<T>(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

fn main() {}
