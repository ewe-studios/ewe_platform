use ewe_templates_macro::template;
use serde_json::{json, Value};

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
