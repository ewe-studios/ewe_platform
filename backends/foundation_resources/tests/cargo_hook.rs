#![cfg(feature = "build")]
use std::{fs, path::Path, process::Command};

fn command(package: &Path, target: &Path) -> Command {
    let mut command = Command::new(env!("CARGO"));
    command
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
        .env_remove("EWE_RESOURCE_PAYLOAD")
        .env("EWE_RESOURCE_TARGET", "web");
    command
}

fn cargo(package: &Path, target: &Path, expect_success: bool) {
    let output = command(package, target).output().unwrap();
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
[features]
local-only = []
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
    let package =
        foundation_resources::package::BrowserPackage::new(std::path::Path::new("web/app.js"))
            .browser_enabled(false);
    if cfg!(feature = "local-only") {
        package.with_generated("revision", "native").with_dev_preview(false).build()
    } else {
        package.with_generated("revision", "generated").build()
    }
}
"#,
    )
    .unwrap();
    let manifest = "[[resource]]\nname='art'\npath='art'\n[[resource]]\nname='version'\ngenerated='revision'\n[[resource]]\nname='browser.js'\npath='browser.js'\ntargets=['web']\n";
    fs::write(app.join("resources.toml"), manifest).unwrap();
    fs::write(app.join("browser.js"), "boot").unwrap();
    fs::write(app.join("art/icon"), "first").unwrap();
    let public = target.join("package-check/public/resource_fixture");
    let build_local = || {
        let output = command(&app, &target)
            .args(["--features", "local-only"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    build_local();
    assert!(!public.exists());
    cargo(&app, &target, true);
    // Change a tracked input so Cargo reruns the local build hook.
    fs::write(app.join("art/icon"), "native edit").unwrap();
    build_local();
    assert_eq!(fs::read(public.join("version")).unwrap(), b"generated");
    assert_eq!(fs::read(public.join("art/icon")).unwrap(), b"first");
    assert_eq!(fs::read(public.join("version")).unwrap(), b"generated");
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(public.join("version")).unwrap().permissions(),
        fs::metadata(public.join("browser.js")).unwrap().permissions()
    );
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

    let stage = temp.path().join("apple");
    let output = command(&app, &target)
        .args(["--features", "local-only"])
        .env("EWE_RESOURCE_TARGET", "apple")
        .env("EWE_RESOURCE_DEST", &stage)
        .env("EWE_RESOURCE_PAYLOAD", "EweResources")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload = stage.join("EweResources");
    assert_eq!(fs::read(payload.join("version")).unwrap(), b"native");
    assert!(!payload.join("browser.js").exists());
    assert!(!payload.join(".ewe-resource-index").exists());
    assert!(stage.join(".ewe-resource-index").is_file());
}

#[cfg(feature = "server")]
#[test]
fn external_consumer_mounts_the_same_resource_set_in_both_profiles() {
    let temp = tempfile::tempdir().unwrap();
    let app = temp.path();
    fs::create_dir(app.join("src")).unwrap();
    let provider = Path::new(env!("CARGO_MANIFEST_DIR"));
    let http = provider.parent().unwrap().join("foundation_http");
    fs::write(
        app.join("Cargo.toml"),
        format!(
            r#"
[package]
name = "packaged-consumer"
version = "1.0.0"
edition = "2021"
[workspace]
[dependencies]
foundation_resources = {{path={provider:?},default-features=false,features=["server"]}}
foundation_http = {{path={http:?}}}
[build-dependencies]
foundation_resources = {{path={provider:?},default-features=false,features=["build"]}}
[profile.live]
inherits="dev"
debug=0
[profile.embedded]
inherits="dev"
debug=0
debug-assertions=false
[profile.embedded.build-override]
debug-assertions=true
"#
        ),
    )
    .unwrap();
    fs::copy(provider.join("../../Cargo.lock"), app.join("Cargo.lock")).unwrap();
    fs::write(app.join("build.rs"),"fn main() -> std::io::Result<()> { foundation_resources::build::stage_from_cargo()?; Ok(()) }").unwrap();
    fs::write(
        app.join("resources.toml"),
        "[[resource]]\nname='module.wasm'\npath='module.wasm'\n",
    )
    .unwrap();
    fs::write(app.join("module.wasm"), [0, 97, 115, 109, 255]).unwrap();
    fs::write(app.join("src/main.rs"),r#"
use foundation_http::{Serve, shared::router::Router,native::handlers::static_file::StaticFileHandler};
use foundation_resources::server::{PackagedResourceSet,PackagedFile};
foundation_resources::packaged_resources!(PackagedResources);
fn main(){
    let mut router:Router<std::sync::Arc<dyn Serve>>=Router::new();
    router.mount(StaticFileHandler::from_resources(PackagedResources)).unwrap();
    assert!(PackagedResources.lookup("missing").unwrap().is_none());
    match PackagedResources.lookup("module.wasm").unwrap().unwrap(){
        PackagedFile::Live(path)=>{ assert!(cfg!(debug_assertions));std::fs::write(&path,[7]).unwrap();assert_eq!(std::fs::read(path).unwrap(),[7]);}
        PackagedFile::Embedded(bytes)=>{assert!(!cfg!(debug_assertions));assert_eq!(bytes,[0,97,115,109,255]);}
    }
}
"#).unwrap();
    for profile in ["live", "embedded"] {
        let result = Command::new(env!("CARGO"))
            .args(["run", "--quiet", "--offline", "--profile", profile])
            .current_dir(app)
            .env("CARGO_TARGET_DIR", app.join("target"))
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{profile}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
