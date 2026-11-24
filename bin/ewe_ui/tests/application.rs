//! Verify the application contract through the installed command surface.
use std::{
    path::Path,
    process::{Command, Output},
};
fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ewe_ui"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn compose_every_selection_and_share_native_inputs() {
    for mask in 1..8 {
        let tmp = tempfile::tempdir().unwrap();
        let mut args = vec!["new", "--name", "FieldNotes"];
        for (i, name) in ["web", "android", "ios"].into_iter().enumerate() {
            if mask & (1 << i) != 0 {
                args.extend(["--target", name]);
            }
        }
        if mask & 6 != 0 {
            args.extend(["--app-id", "dev.example.fieldnotes", "--host", "on-device"]);
        }
        let result = cli(tmp.path(), &args);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let root = tmp.path().join("FieldNotes");
        for (i, name) in ["web", "android", "ios"].into_iter().enumerate() {
            assert_eq!(root.join(name).is_dir(), mask & (1 << i) != 0);
        }
        assert!(root.join("application/src/lib.rs").is_file());
        assert!(root.join("application/resources.toml").is_file());
        assert_eq!(root.join("native-core/Cargo.toml").is_file(), mask & 6 != 0);
        if mask & 2 != 0 {
            assert!(
                std::fs::read_to_string(root.join("android/app/build.gradle.kts"))
                    .unwrap()
                    .contains("../../application")
            );
        }
        if mask & 4 != 0 {
            assert!(root.join("ios/FieldNotes.xcodeproj").is_dir());
        }
        assert!(!cli(tmp.path(), &args).status.success());
    }
}
#[test]
fn reject_invalid_inputs_before_creating_a_destination() {
    let tmp = tempfile::tempdir().unwrap();
    for (args, message) in [
        (vec!["new"], "--name"),
        (vec!["new", "--name", "App"], "--target"),
        (vec!["new", "--name", "App", "--target", "ios"], "--app-id"),
        (
            vec!["new", "--name", "App", "--target", "desktop"],
            "desktop",
        ),
        (
            vec![
                "new", "--name", "App", "--target", "ios", "--app-id", "invalid",
            ],
            "--app-id",
        ),
    ] {
        let result = cli(tmp.path(), &args);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(message));
    }
    assert_eq!(tmp.path().read_dir().unwrap().count(), 0);
}
#[test]
fn concurrent_creators_publish_one_complete_project() {
    let dir = tempfile::tempdir().unwrap();
    let launch = || {
        Command::new(env!("CARGO_BIN_EXE_ewe_ui"))
            .current_dir(dir.path())
            .args(["new", "--name", "Shared", "--target", "web"])
            .spawn()
            .unwrap()
    };
    let (mut a, mut b) = (launch(), launch());
    assert_ne!(a.wait().unwrap().success(), b.wait().unwrap().success());
    assert!(dir.path().join("Shared/web/Cargo.toml").is_file());
    assert_eq!(dir.path().read_dir().unwrap().count(), 1);
}
#[test]
fn human_and_automation_help_use_the_real_command_tree() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        &["help"][..],
        &["--help"][..],
        &["help", "--llm"][..],
        &["task", "list"][..],
    ] {
        let out = cli(dir.path(), args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(!text.contains('\x1b'));
        if args == ["help", "--llm"] {
            for required in [
                "--target",
                "--app-id",
                "wasm-bundle:npm-pack",
                "--archive",
                "--integrity",
            ] {
                assert!(text.contains(required));
            }
        }
    }
}

#[test]
fn project_task_registration_exposes_arguments_and_rejects_shared_names() {
    use std::fs;
    let tmp = tempfile::tempdir().unwrap();
    assert!(
        cli(tmp.path(), &["new", "--name", "Local", "--target", "web"])
            .status
            .success()
    );
    let root = tmp.path().join("Local");
    let task = root.join("tools/tasks");
    fs::create_dir_all(&task).unwrap();
    fs::write(task.join("Cargo.toml"), "").unwrap();
    let manifest = root.join("ewe-ui.toml");
    let mut document = fs::read_to_string(&manifest).unwrap();
    document.push_str(
        r#"
[tasks]
manifest = "tools/tasks/Cargo.toml"
namespace = "sample"
[[tasks.commands]]
name = "inspect"
description = "Inspect application data"
[[tasks.commands.arguments]]
name = "device"
description = "Device to inspect"
required = true
"#,
    );
    fs::write(&manifest, &document).unwrap();
    for args in [
        &["help", "--llm"][..],
        &["task", "run", "sample:inspect", "--help"],
    ] {
        let out = cli(&root, args);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let text = String::from_utf8(out.stdout).unwrap();
        assert!(text.contains("--device") && text.contains("Device to inspect"));
    }
    assert!(!cli(&root, &["task", "run", "sample:inspect"])
        .status
        .success());
    fs::write(
        manifest,
        document.replace("namespace = \"sample\"", "namespace = \"native\""),
    )
    .unwrap();
    let conflict = cli(&root, &["task", "list"]);
    assert!(!conflict.status.success());
    assert!(String::from_utf8_lossy(&conflict.stderr).contains("conflicts"));
}

#[test]
fn failed_ios_scheme_after_web_render_removes_the_entire_staging_tree() {
    let directory = tempfile::tempdir().unwrap();
    let name = "A".repeat(250);
    let output = cli(
        directory.path(),
        &[
            "new",
            "--name",
            &name,
            "--target",
            "web",
            "--target",
            "ios",
            "--app-id",
            "dev.example.cleanup",
        ],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("iOS scheme"));
    assert_eq!(directory.path().read_dir().unwrap().count(), 0);
}
