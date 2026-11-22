#![cfg(feature = "build")]
use std::{fs, path::Path, process::Command};

fn cargo(package: &Path, target: &Path, expect_success: bool) {
    let output = Command::new(env!("CARGO"))
        .args([
            "build",
            "--offline",
            "--profile",
            "package-check",
            "--target-dir",
        ])
        .arg(target)
        .current_dir(package)
        .env_remove("EWE_RESOURCE_DEST")
        .env("EWE_RESOURCE_TARGET", "web")
        .output()
        .unwrap();
    assert_eq!(
        output.status.success(),
        expect_success,
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cargo_tracks_directories_and_recovers_the_public_tree_after_failed_production() {
    let temp = tempfile::tempdir().unwrap();
    let app = temp.path().join("app");
    let target = temp.path().join("custom-target");
    fs::create_dir_all(app.join("src")).unwrap();
    fs::create_dir(app.join("art")).unwrap();
    let provider = env!("CARGO_MANIFEST_DIR").replace('\\', "/");
    fs::write(
        app.join("Cargo.toml"),
        format!(
            r#"
[package]
name = "resource_fixture"
version = "0.1.0"
edition = "2021"
[workspace]
[profile.package-check]
inherits = "dev"
[build-dependencies]
foundation_resources = {{ path = '{provider}', default-features = false, features = ["build"] }}
"#
        ),
    )
    .unwrap();
    fs::write(app.join("src/lib.rs"), "").unwrap();
    fs::write(
        app.join("build.rs"),
        r#"
fn main() -> std::io::Result<()> {
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("revision");
    if !output.exists() { std::fs::write(output, b"generated")?; }
    foundation_resources::build::stage_from_cargo()?;
    Ok(())
}
"#,
    )
    .unwrap();
    let manifest = "[[resource]]\nname='art'\npath='art'\n[[resource]]\nname='version'\ngenerated='revision'\n[[resource]]\nname='browser.js'\npath='browser.js'\ntargets=['web']\n";
    fs::write(app.join("resources.toml"), manifest).unwrap();
    fs::write(app.join("browser.js"), "boot").unwrap();
    fs::write(app.join("art/icon"), "first").unwrap();
    let public = target.join("package-check/public/resource_fixture");
    cargo(&app, &target, true);
    assert_eq!(fs::read(public.join("version")).unwrap(), b"generated");
    assert_eq!(fs::read(public.join("art/icon")).unwrap(), b"first");
    assert!(public.join("browser.js").is_file());
    fs::write(public.join("user"), "keep").unwrap();
    fs::rename(app.join("art/icon"), app.join("art/logo")).unwrap();
    fs::write(app.join("art/logo"), "second").unwrap();
    fs::write(
        app.join("resources.toml"),
        format!("{manifest}[[resource]]\nname='broken'\npath='absent'"),
    )
    .unwrap();
    cargo(&app, &target, false);
    assert_eq!(fs::read(public.join("art/icon")).unwrap(), b"first");
    assert!(!public.join("art/logo").exists());
    fs::write(app.join("resources.toml"), manifest).unwrap();
    cargo(&app, &target, true);
    assert_eq!(fs::read(public.join("art/logo")).unwrap(), b"second");
    assert!(!public.join("art/icon").exists());
    fs::remove_file(app.join("art/logo")).unwrap();
    cargo(&app, &target, true);
    assert!(!public.join("art/logo").exists());
    assert_eq!(fs::read(public.join("user")).unwrap(), b"keep");
}
