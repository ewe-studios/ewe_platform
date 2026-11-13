//! `Templater` tests moved out of `src/files.rs` (spec 70 feature 59,
//! rule 7).
//!
//! The `files` items and `BoxedError` were crate-private; they are now
//! re-exported from the crate root so the same assertions run from `tests/`.

use std::{env, fs, io::Read, path, sync};

use foundation_packager::tinytemplate::TinyTemplate;
use foundation_packager::{BoxedError, FileContent, FileSystemCommand, Templater};
use rand::Rng;
use serde_json::{json, Value};

type TestResult<T> = std::result::Result<T, BoxedError>;

fn random_directory_name(prefix: &str) -> String {
    let suffix: String = rand::rng()
        .sample_iter(&rand::distr::Alphanumeric)
        .take(16)
        .map(char::from)
        .collect();
    format!("{prefix}_{suffix}")
}

fn clean_up_directory(target: path::PathBuf) {
    fs::remove_dir_all(target).expect("should have deleted directory");
}

fn create_jinja_template() -> TestResult<minijinja::Environment<'static>> {
    let mut tt = minijinja::Environment::new();

    tt.add_template("world", "{{country}} wonderworld!")?;
    tt.add_template("hello", "Welcome to hello {{name}}!")?;

    tt.add_template("index", r"{% include 'hello' %} {% include 'world' %}")?;

    Ok(tt)
}

fn create_tiny_template() -> TestResult<TinyTemplate<'static>> {
    let mut tt = TinyTemplate::new();

    tt.add_template("world", "{country} wonderworld!")?;
    tt.add_template("hello", "Welcome to hello {name}!")?;

    tt.add_template(
        "index",
        r"{{ call hello with @root }} {{ call world with @root }}",
    )?;

    Ok(tt)
}

#[test]
fn test_can_create_directory() {
    let tmp_dir = env::temp_dir();

    let mut target = tmp_dir.clone();
    target.push("temple");

    let mut tml = Templater::new(target.to_str().unwrap());
    tml.add(FileSystemCommand::Dir(String::from("weeds"), vec![]));

    let data: Value = json!({
        "code": 200,
        "name": "Alex",
        "country": "Nigeria",
    });

    assert!(matches!(tml.run(&data), Ok(())));

    let mut expected_path = target.clone();
    expected_path.push("weeds");

    assert!(expected_path.exists());
    assert!(expected_path.is_dir());

    clean_up_directory(target);
}

#[test]
fn test_can_create_directory_with_file_with_jinja() {
    let tmp_dir = env::temp_dir();

    let mut target = tmp_dir.clone();
    target.push(random_directory_name("temple"));

    let mut tml = Templater::new(target.to_str().unwrap());

    let templ = sync::Arc::new(create_jinja_template().expect("created sample template"));

    tml.add(FileSystemCommand::Dir(
        String::from("weeds"),
        vec![FileSystemCommand::File(
            String::from("index.md"),
            FileContent::Jinja(String::from("index"), templ),
        )],
    ));

    let data: Value = json!({
        "code": 200,
        "name": "Alex",
        "country": "Nigeria",
    });

    tml.run(&data).expect("finished executing template");

    let mut expected_path = target.clone();
    expected_path.push("weeds");

    assert!(expected_path.exists());
    assert!(expected_path.is_dir());

    let mut expected_file = target.clone();
    expected_file.push("weeds");
    expected_file.push("index.md");

    assert!(expected_file.exists());
    assert!(expected_file.is_file());

    let mut created_file = fs::File::open(expected_file).expect("should read created file");

    let mut content = String::new();
    let read = created_file
        .read_to_string(&mut content)
        .expect("should read to string");

    assert_ne!(read, 0);

    assert_eq!(content, "Welcome to hello Alex! Nigeria wonderworld!");

    clean_up_directory(target);
}

#[test]
fn test_can_create_directory_with_file_with_tiny() {
    let tmp_dir = env::temp_dir();

    let mut target = tmp_dir.clone();
    target.push(random_directory_name("temple"));

    let mut tml = Templater::new(target.to_str().unwrap());

    let templ = create_tiny_template().expect("created sample template");

    tml.add(FileSystemCommand::Dir(
        String::from("weeds"),
        vec![FileSystemCommand::File(
            String::from("index.md"),
            FileContent::Tiny(String::from("index"), templ),
        )],
    ));

    let data: Value = json!({
        "code": 200,
        "name": "Alex",
        "country": "Nigeria",
    });

    tml.run(&data).expect("finished executing template");

    let mut expected_path = target.clone();
    expected_path.push("weeds");

    assert!(expected_path.exists());
    assert!(expected_path.is_dir());

    let mut expected_file = target.clone();
    expected_file.push("weeds");
    expected_file.push("index.md");

    assert!(expected_file.exists());
    assert!(expected_file.is_file());

    let mut created_file = fs::File::open(expected_file).expect("should read created file");

    let mut content = String::new();
    let read = created_file
        .read_to_string(&mut content)
        .expect("should read to string");

    assert_ne!(read, 0);

    assert_eq!(content, "Welcome to hello Alex! Nigeria wonderworld!");

    clean_up_directory(target);
}
