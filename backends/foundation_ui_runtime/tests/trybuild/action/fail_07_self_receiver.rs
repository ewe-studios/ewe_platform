use foundation_ui_runtime::action;

#[action]
async fn submit(&self, _event: Interaction<()>) -> ActionResult {
    Ok(())
}

fn main() {}
