use foundation_ui_runtime::html;

fn main() {
    let _ = html! { <Either condition={true}><Left/></Either> };
    let _ = html! { <Either condition={true}><Left/><Right/><Right/></Either> };
    let _ = html! { <Either condition={42}><Left/><Right/></Either> };
    let _ = html! { <Either condition={true}><Right/><Left><Text primal::state/></Left></Either> };
}
