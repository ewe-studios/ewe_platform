use foundation_ui_runtime::action;

#[action]
async unsafe fn submit(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

fn main() {}
