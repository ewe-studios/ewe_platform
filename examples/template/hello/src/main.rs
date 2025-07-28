use ewe_templates_macro::template;
use serde_json::{json, Value};

fn main() {
    let data: Value = json!({
        "code": 200,
        "name": "Alex",
        "country": "Nigeria",
    });

    let template = template!(tiny, {
         [hello, r"
            hello from template {}
        "],
    });

    print!("Content: {:?}", template.render("hello", &data));
}
