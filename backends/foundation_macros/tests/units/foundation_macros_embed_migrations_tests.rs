//! Build-time checks of `embed_migrations!`.

use std::{fs, path::Path, process::Command};

#[test]
fn embed_migrations_rejects_a_gap() {
    let macros = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fixture = macros.join("tests/ui");
    let package = tempfile::tempdir().unwrap();
    let migrations = package.path().join("migrations");
    fs::create_dir(&migrations).unwrap();
    for entry in fs::read_dir(fixture.join("embed_migrations_gap_data")).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), migrations.join(entry.file_name())).unwrap();
    }
    let source = fixture.join("embed_migrations_gap.rs");
    let database = macros.parent().unwrap().join("foundation_db");
    let manifest = package.path().join("Cargo.toml");
    fs::write(
        &manifest,
        format!(
            r#"[package]
name = "migration-gap"
version = "0.0.0"
edition = "2021"

[[bin]]
name = "migration-gap"
path = {source:?}

[dependencies]
foundation_db = {{ path = {database:?}, default-features = false }}

[workspace]
"#
        ),
    )
    .unwrap();
    let result = Command::new(env!("CARGO"))
        .current_dir(macros)
        .args([
            "check",
            "--offline",
            "--message-format=json",
            "--manifest-path",
        ])
        .arg(manifest)
        .output()
        .unwrap();
    assert!(
        !result.status.success(),
        "The migration gap must fail compilation"
    );
    let messages = String::from_utf8(result.stdout).unwrap();
    let errors: Vec<_> = messages
        .lines()
        .filter_map(|line| {
            let event: serde_json::Value = serde_json::from_str(line).unwrap();
            let diagnostic = event.get("message")?;
            (diagnostic["level"] == "error")
                .then(|| diagnostic["message"].as_str().unwrap().to_owned())
        })
        .collect();
    assert_eq!(
        errors,
        ["migrations: group is missing migration number 2"],
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
