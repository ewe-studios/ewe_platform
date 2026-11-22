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
