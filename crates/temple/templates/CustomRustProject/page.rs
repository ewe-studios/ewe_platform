use tinytemplate::TinyTemplate;

fn main() {
    let mut tt = TinyTemplate::new();

    tt.add_template("world", "{country} wonderworld!")?;
    tt.add_template("hello", "Welcome to hello {name}!")?;

    tt.add_template(
        "index",
        r#"{{ call hello with @root }} {{ call world with @root }}"#,
    )?;
}
