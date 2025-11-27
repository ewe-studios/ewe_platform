use ewe_templates::minijinja::context;
use ewe_templates_macro::template;

fn main() {
    let data = context!(code => 200, name => "Alex", country => "Nigeria");

    let template = template!(jinja, {
         [hello, r#"
            hello from template {{name}}
        "#],
    });

    print!(
        "Content: {:?}",
        template
            .get_template("hello")
            .unwrap()
            .render(data)
            .unwrap()
    );
}
