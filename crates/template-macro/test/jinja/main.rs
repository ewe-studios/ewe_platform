use ewe_template_macro::template;
use ewe_templates::minijinja::context;

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
