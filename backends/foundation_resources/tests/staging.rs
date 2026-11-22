#![cfg(feature = "build")]
use foundation_resources::build::{stage, Target};
use std::fs;

#[test]
fn stage_selects_inputs_and_preserves_the_previous_complete_set_on_failure() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let output = root.join("public");
    let generated = root.join("generated");
    fs::create_dir_all(root.join("source")).unwrap();
    fs::create_dir(&generated).unwrap();
    fs::write(root.join("source/a.txt"), "first").unwrap();
    fs::write(generated.join("boot.js"), "generated").unwrap();
    let manifest = root.join("resources.toml");
    let policy="[[resource]]\nname='art'\npath='source'\n[[resource]]\nname='boot.js'\ngenerated='boot.js'\ntargets=['web']\n";
    fs::write(&manifest, policy).unwrap();
    let result = stage(&manifest, &generated, Target::Web, &output).unwrap();
    assert_eq!(result.names, ["art/a.txt", "boot.js"]);
    assert!(result
        .inputs
        .contains(&root.join("source").canonicalize().unwrap()));
    fs::write(output.join("user"), "keep").unwrap();
    fs::write(
        &manifest,
        format!("{policy}\n[[resource]]\nname='ART/A.TXT'\npath='source/a.txt'"),
    )
    .unwrap();
    assert!(stage(&manifest, &generated, Target::Web, &output).is_err());
    assert_eq!(fs::read(output.join("boot.js")).unwrap(), b"generated");
    fs::write(&manifest, policy).unwrap();
    fs::rename(root.join("source/a.txt"), root.join("source/b.txt")).unwrap();
    stage(&manifest, &generated, Target::Android, &output).unwrap();
    assert!(output.join("art/b.txt").is_file());
    assert!(!output.join("art/a.txt").exists());
    assert!(!output.join("boot.js").exists());
    assert_eq!(fs::read(output.join("user")).unwrap(), b"keep");
}

#[test]
fn shared_destinations_serialize_and_invalid_names_fail() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let manifest = root.join("resources.toml");
    fs::write(root.join("icon"), b"shared").unwrap();
    fs::write(&manifest, "[[resource]]\nname='art/icon'\npath='icon'").unwrap();
    let output = root.join("mobile");
    std::thread::scope(|threads| {
        for _ in 0..4 {
            threads.spawn(|| stage(&manifest, root, Target::Apple, &output).unwrap());
        }
    });
    assert_eq!(fs::read(output.join("art/icon")).unwrap(), b"shared");
    for name in ["../secret", "/absolute", "a//b", "a%2fb", "nul.txt"] {
        assert!(foundation_resources::ResourceName::new(name).is_err());
    }
}

#[test]
fn empty_managed_directories_disappear_and_nested_user_files_survive() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let manifest = root.join("resources.toml");
    let output = root.join("staged");
    fs::create_dir_all(root.join("icons/empty")).unwrap();
    fs::write(&manifest, "[[resource]]\nname='art'\npath='icons'").unwrap();
    stage(&manifest, root, Target::Web, &output).unwrap();
    assert!(output.join("art/empty").is_dir());
    fs::write(output.join("art/user.txt"), "keep").unwrap();
    fs::write(&manifest, "").unwrap();
    stage(&manifest, root, Target::Web, &output).unwrap();
    assert!(!output.join("art/empty").exists());
    assert_eq!(fs::read(output.join("art/user.txt")).unwrap(), b"keep");
}

#[test]
#[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
fn a_reader_sees_complete_versions_during_directory_replacement() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Barrier,
    };
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let manifest = root.join("resources.toml");
    fs::write(&manifest, "[[resource]]\nname='version'\npath='version'").unwrap();
    fs::write(root.join("version"), "first").unwrap();
    let dest = root.join("staged");
    stage(&manifest, root, Target::Apple, &dest).unwrap();
    let running = AtomicBool::new(true);
    let barrier = Barrier::new(2);
    std::thread::scope(|threads| {
        let reader = threads.spawn(|| {
            barrier.wait();
            while running.load(Ordering::Acquire) {
                let bytes = fs::read(dest.join("version")).unwrap();
                assert!(bytes == b"first" || bytes == b"second");
            }
        });
        barrier.wait();
        for _ in 0..12 {
            fs::write(root.join("version"), "second").unwrap();
            stage(&manifest, root, Target::Apple, &dest).unwrap();
        }
        running.store(false, Ordering::Release);
        reader.join().unwrap();
    });
}

#[test]
fn a_stage_cannot_copy_its_own_output() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let manifest = root.join("resources.toml");
    fs::create_dir(root.join("art")).unwrap();
    fs::write(root.join("art/icon"), "icon").unwrap();
    fs::write(&manifest, "[[resource]]\nname='art'\npath='art'").unwrap();
    for destination in [
        root.to_path_buf(),
        root.join("art"),
        root.join("art/nested/staged"),
    ] {
        assert!(stage(&manifest, root, Target::Web, &destination).is_err());
    }
    assert!(!root.join("art/nested").exists());
}

#[test]
fn unchanged_publication_keeps_existing_file_times_and_restores_missing_files() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let manifest = root.join("resources.toml");
    fs::write(&manifest, "[[resource]]\nname='icon'\npath='original'").unwrap();
    fs::write(root.join("original"), "app").unwrap();
    let dest = root.join("output");
    stage(&manifest, root, Target::Android, &dest).unwrap();
    let modified = fs::metadata(dest.join("icon")).unwrap().modified().unwrap();
    stage(&manifest, root, Target::Android, &dest).unwrap();
    assert_eq!(
        fs::metadata(dest.join("icon")).unwrap().modified().unwrap(),
        modified
    );
    fs::remove_file(dest.join("icon")).unwrap();
    stage(&manifest, root, Target::Android, &dest).unwrap();
    assert_eq!(fs::read(dest.join("icon")).unwrap(), b"app");
}
