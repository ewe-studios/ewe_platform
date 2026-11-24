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
