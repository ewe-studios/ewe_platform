//! Check all published mobile project layouts through the command line.

use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn generate(root: &Path, platform: &str, name: &str, id: &str, host: Option<&str>) -> Output {
    let mut cli = Command::new(env!("CARGO_BIN_EXE_ewe_platform"));
    cli.current_dir(root).args([
        "generate", "-t", platform, "-p", name, "-l", "plain", "--app-id", id,
    ]);
    if let Some(host) = host {
        cli.args(["--host", host]);
    }
    cli.output().unwrap()
}

#[test]
fn mobile_projects_select_their_host_and_preserve_existing_files() {
    let root = std::env::temp_dir().join(format!("ewe-mobile-templates-{}", std::process::id()));
    fs::create_dir(&root).unwrap();
    for (platform, prefix) in [("ios-app", "Swift"), ("android-app", "Kotlin")] {
        for host in [None, Some("on-device")] {
            let local = host.is_some();
            let name = format!("{prefix}{}", if local { "Counter" } else { "Remote" });
            let id = format!("com.example.{}", name.to_lowercase());
            let out = generate(&root, platform, &name, &id, host);
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            let project = root.join(&name);
            assert_eq!(project.join("rust/Cargo.toml").is_file(), local);
            assert_eq!(project.join("build-rust.sh").is_file(), local);
            let provider = project.join(if local { "rust" } else { "resources" });
            assert!(fs::read_to_string(provider.join("Cargo.toml"))
                .unwrap()
                .contains("foundation_resources"));
            let declaration = fs::read_to_string(provider.join("resources.toml")).unwrap();
            assert!(declaration.contains("app/title.txt"));
            assert!(declaration.contains("generated = \"version.txt\""));
            assert!(fs::read_to_string(provider.join("build.rs"))
                .unwrap()
                .contains("stage_from_cargo"));
            assert_eq!(
                fs::read_to_string(provider.join("assets/title.txt"))
                    .unwrap()
                    .trim(),
                name
            );
            if platform == "android-app" {
                assert_eq!(
                    fs::read(project.join("gradle/wrapper/gradle-wrapper.jar")).unwrap(),
                    include_bytes!(
                        "../../../backends/foundation_ui_project/templates/android-app/common/gradle/wrapper/gradle-wrapper.jar"
                    )
                    .as_slice()
                );
                assert!(fs::read_to_string(project.join("app/build.gradle.kts"))
                    .unwrap()
                    .contains(&id));
                let main = fs::read_to_string(
                    project.join("app/src/main/kotlin/ewe/starter/MainActivity.kt"),
                )
                .unwrap();
                assert!(main.contains(&format!("package {id}")));
                assert_eq!(main.contains("LocalEweScreen"), local);
                assert_eq!(main.contains("RemoteEweScreen"), !local);
            } else {
                let xcode = project.join(format!("{name}.xcodeproj"));
                assert!(xcode
                    .join(format!("xcshareddata/xcschemes/{name}.xcscheme"))
                    .is_file());
                let project = fs::read_to_string(xcode.join("project.pbxproj")).unwrap();
                assert!(project.contains(&id));
                assert_eq!(project.contains("CounterFFI.xcframework"), local);
            }
            fs::write(project.join("README.md"), "keep").unwrap();
            let again = generate(&root, platform, &name, &id, host);
            assert!(!again.status.success());
            assert!(String::from_utf8_lossy(&again.stderr).contains(&name));
            assert_eq!(
                fs::read_to_string(project.join("README.md")).unwrap(),
                "keep"
            );
        }
    }
    for (name, id) in [
        ("../Escape", "com.example.test"),
        ("Invalid", "com..example"),
    ] {
        assert!(!generate(&root, "android-app", name, id, None)
            .status
            .success());
    }
    fs::remove_dir_all(root).unwrap();
}
