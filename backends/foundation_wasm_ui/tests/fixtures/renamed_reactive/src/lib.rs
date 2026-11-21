//! Compile pure and reactive templates with renamed dependencies.
use renderer::{html, App, ProtocolError};

pub fn pure() -> template::Html {
    html! { <div>"static"</div> }
}

pub fn update(app: &App) -> Result<(), ProtocolError> {
    let (ctx, receiver) = app.context();
    let (count, set_count) = ctx.signal(1u32);
    let _tree = html! {ctx, receiver, <div>{count.get()}</div>};
    set_count.set(2);
    app.stabilize()
}
