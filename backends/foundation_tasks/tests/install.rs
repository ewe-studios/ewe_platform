//! The build hook installer works with ordinary Cargo consumers.
use foundation_tasks::install::{install, Capability};
use std::{fs, process::Command};

#[test]
fn new_consumer_builds_checks_and_tracks_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for folder in ["src", "generator/src"] {
        fs::create_dir_all(root.join(folder)).unwrap();
    }
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='consumer'\nversion='0.1.0'\nedition='2024'\n[workspace]\n",
    )
    .unwrap();
    fs::write(
        root.join("generator/Cargo.toml"),
        "[package]\nname='generator'\nversion='1.0.0'\nedition='2024'\n",
    )
    .unwrap();
    fs::write(root.join("generator/src/lib.rs"), r#"pub fn generate() {
        println!("cargo:rerun-if-changed=input.txt");
        std::fs::copy("input.txt", std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("message")).unwrap();
    }"#).unwrap();
    fs::write(
        root.join("src/main.rs"),
        r#"fn main() { print!("{}", include_str!(concat!(env!("OUT_DIR"), "/message"))); }"#,
    )
    .unwrap();
    let manifest = root.join("Cargo.toml");
    let original = fs::read(&manifest).unwrap();
    let provider = Capability {
        name: "example",
        package: "generator",
        version: "1.0.0",
        path: Some(root.join("generator")),
        features: &[],
        function: "generate",
        fallible: false,
    };
    assert!(install(&manifest, &provider, true).is_err());
    assert_eq!(fs::read(&manifest).unwrap(), original);
    assert!(!root.join("build.rs").exists());
    install(&manifest, &provider, false).unwrap();
    let installed = fs::read(&manifest).unwrap();
    let script = fs::read(root.join("build.rs")).unwrap();
    install(&manifest, &provider, true).unwrap();
    install(&manifest, &provider, false).unwrap();
    assert_eq!(fs::read(&manifest).unwrap(), installed);
    assert_eq!(fs::read(root.join("build.rs")).unwrap(), script);
    for value in ["first", "second"] {
        fs::write(root.join("input.txt"), value).unwrap();
        let output = Command::new(env!("CARGO"))
            .args(["run", "--offline", "--quiet"])
            .current_dir(root)
            .env("CARGO_TARGET_DIR", root.join("target"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, value.as_bytes());
    }
}

fn provider(name: &'static str) -> Capability<'static> {
    Capability {
        name,
        package: "generator",
        version: "1.0.0",
        path: None,
        features: &["codegen"],
        function: "generate",
        fallible: false,
    }
}

#[test]
fn preserves_existing_workspace_alias_and_custom_fallible_main() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("member/scripts")).unwrap();
    fs::write(root.join("Cargo.toml"), "[workspace]\nmembers=['member']\n[workspace.dependencies]\nmy-gen={package='generator',version='1'}\n").unwrap();
    let path = root.join("member/Cargo.toml");
    fs::write(&path, "[package]\nname='member'\nversion='0.1.0'\nbuild='scripts/package.rs'\n[build-dependencies]\nmy-gen = { workspace = true, features = ['first'] } # owner\n").unwrap();
    let script = root.join("member/scripts/package.rs");
    let original = "// épü\nfn main() -> Result<(), std::io::Error> {\n    #![allow(unused)]\n    my_gen::generate(); // user hook\n    if true { return Ok(()) }\n    Ok(())\n}\n";
    fs::write(&script, original).unwrap();
    install(&path, &provider("second"), false).unwrap();
    assert_eq!(fs::read_to_string(&script).unwrap(), original);
    let manifest = fs::read_to_string(&path).unwrap();
    for text in ["workspace = true", "'first'", "\"codegen\"", "# owner"] {
        assert!(manifest.contains(text), "{manifest}");
    }
    let a = Capability {
        function: "alpha",
        ..provider("alpha")
    };
    let z = Capability {
        function: "zulu",
        ..provider("zulu")
    };
    for hook in [&z, &a] {
        install(&path, hook, false).unwrap();
    }
    let first = fs::read_to_string(&script).unwrap();
    for text in [
        "#![allow(unused)]",
        "my_gen::alpha();",
        "my_gen::zulu();",
        "// user hook",
        "return Ok(())",
    ] {
        assert!(first.contains(text), "{first}");
    }
    for hook in [&z, &a] {
        install(&path, hook, true).unwrap();
    }
    fs::write(&script, original).unwrap();
    for hook in [&a, &z] {
        install(&path, hook, false).unwrap();
    }
    assert_eq!(fs::read_to_string(&script).unwrap(), first);
}

#[test]
fn a_failed_second_write_restores_the_original_manifest() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Cargo.toml");
    let source = "[package]\nname='demo'\nversion='0.1.0'\nbuild='missing/custom.rs'\n";
    fs::write(&path, source).unwrap();
    assert!(install(&path, &provider("example"), false).is_err());
    assert_eq!(fs::read_to_string(&path).unwrap(), source);
    fs::create_dir(dir.path().join("missing")).unwrap();
    install(&path, &provider("example"), false).unwrap();
    install(&path, &provider("example"), true).unwrap();
}

#[test]
fn real_runner_rejects_conflicts_without_touching_either_file() {
    use foundation_context::Ctx;
    use foundation_tasks::{install::task_set, TaskRunner};
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("Cargo.toml");
    let script = dir.path().join("build.rs");
    fs::write(&script, "fn main() {}\n").unwrap();
    for (build, dep) in [
        ("build = false", ""),
        ("", "[build-dependencies]\ngenerator='2'"),
    ] {
        let source = format!("[package]\nname='demo'\nversion='1.0.0'\n{build}\n{dep}\n");
        fs::write(&manifest, &source).unwrap();
        for check in [true, false] {
            let mut argv: Vec<std::ffi::OsString> = [
                "platform",
                "task",
                "run",
                "build:install-hook",
                "example",
                "--manifest-path",
            ]
            .into_iter()
            .map(Into::into)
            .collect();
            argv.push(manifest.clone().into_os_string());
            if check {
                argv.push("--check".into());
            }
            let mut runner = TaskRunner::new(Ctx::host(), argv);
            runner
                .register(task_set(vec![provider("example")]).unwrap())
                .unwrap();
            assert!(futures_lite::future::block_on(runner.run()).is_err());
            assert_eq!(fs::read_to_string(&manifest).unwrap(), source);
            assert_eq!(fs::read_to_string(&script).unwrap(), "fn main() {}\n");
        }
    }
}

#[test]
fn concurrent_installers_preserve_both_updates() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = dir.path().join("Cargo.toml");
    fs::write(&manifest, "[package]\nname='demo'\nversion='0.1.0'\n").unwrap();
    std::thread::scope(|scope| {
        for (name, function) in [("first", "one"), ("second", "two")] {
            let path = &manifest;
            scope.spawn(move || {
                install(
                    path,
                    &Capability {
                        function,
                        ..provider(name)
                    },
                    false,
                )
                .unwrap();
            });
        }
    });
    for (name, function) in [("first", "one"), ("second", "two")] {
        install(
            &manifest,
            &Capability {
                function,
                ..provider(name)
            },
            true,
        )
        .unwrap();
    }
}
