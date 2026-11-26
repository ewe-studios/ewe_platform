use foundation_ui_runtime::html;

fn main() {
    let _view = html! { <Button on:tap={confirm()} /> };
}
