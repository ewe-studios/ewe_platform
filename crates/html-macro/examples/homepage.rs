use html_macro::{html, Html}

fn app() -> Html {
    html!(
        <h1>"High-Five counter: {count}"</h1>
        <button onclick={move |_| count += 1}>"Up high!"</button>
        <button onclick={move |_| count -= 1}>"Down low!"</button>
    )
}

fn main() {
    let elem = app();
}
