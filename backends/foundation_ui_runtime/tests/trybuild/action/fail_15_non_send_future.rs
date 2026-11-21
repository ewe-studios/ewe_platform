use foundation_ui_runtime::action;
use foundation_ui_runtime::action::{ActionResult, Context, Interaction};
use std::rc::Rc;

#[action]
async fn submit(_ctx: &Context, _event: Interaction<()>) -> ActionResult {
    let local = Rc::new(42);
    std::future::ready(()).await;
    drop(local);
    Ok(())
}

fn main() {}
