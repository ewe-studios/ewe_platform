use template_macro::template;

fn main() {
    let template = template!(jinja, {
         [index, r#"
            hello from template {}
        "#],
         [about, r#"
            about the template {}
        "#],
    });
    print!("Content: {:?}", template);
}
