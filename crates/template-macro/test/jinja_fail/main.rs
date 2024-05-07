use serde_json::{json, Value};
use template_macro::template;

fn main() {
    let data: Value = json!({
        "code": 200,
        "name": "Alex",
        "country": "Nigeria",
    });

    let template = template!(jinja, {
         [hello, 20],
    });

    print!("Content: {:?}", template.render("hello", &data));
}
