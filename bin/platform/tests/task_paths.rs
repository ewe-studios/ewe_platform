use std::{
    path::Path,
    process::{Command, Output},
};

fn execute(program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .args(args)
        .output()
        .expect("start platform executable")
}

#[test]
fn both_executables_share_sorted_task_listing() {
    let names = [
        env!("CARGO_BIN_EXE_platform"),
        env!("CARGO_BIN_EXE_ewe_platform"),
    ];
    let first = execute(names[0], &["task", "ls"]);
    let second = execute(names[1], &["task", "ls"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    let listing = String::from_utf8(first.stdout).unwrap();
    let tasks = listing
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect::<Vec<_>>();
    assert!(tasks.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(tasks.contains(&"generate"));
    assert!(tasks.contains(&"codegen:normalize"));
    assert!(tasks.contains(&"native:release:publish"));
}

#[test]
fn existing_leaves_and_generic_names_share_argument_help() {
    let executable = env!("CARGO_BIN_EXE_platform");
    for (name, path) in [
        ("generate", &["generate"][..]),
        ("local", &["local"][..]),
        ("tcp-capture", &["tcp_capture"][..]),
        ("gen-model-descriptors", &["gen_model_descriptors"][..]),
        ("codegen:generate", &["gen_api", "generate"][..]),
        ("codegen:analyze", &["gen_api", "analyze"][..]),
        ("codegen:normalize", &["gen_api", "normalize"][..]),
        ("codegen:schema", &["schema"][..]),
        ("wasm-bins:list", &["wasm_bins", "list"][..]),
        ("wasm-bins:generate", &["wasm_bins", "generate"][..]),
        ("wasm-bundle:build", &["wasm-bundle", "build"][..]),
        ("wasm-bundle:runtime", &["wasm-bundle", "runtime"][..]),
        ("wasm-bundle:plan", &["wasm-bundle", "plan"][..]),
    ] {
        let mut direct_args = path.to_vec();
        direct_args.push("--help");
        let direct = execute(executable, &direct_args);
        let generic = execute(executable, &["task", "run", name, "--help"]);
        assert!(
            direct.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&direct.stderr)
        );
        assert!(
            generic.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&generic.stderr)
        );
        let direct_help = String::from_utf8(direct.stdout).unwrap();
        let generic_help = String::from_utf8(generic.stdout).unwrap();
        assert_eq!(
            direct_help.replace(
                &format!("Usage: platform {}", path.join(" ")),
                "Usage: command"
            ),
            generic_help.replace(
                &format!("Usage: platform task run {name}"),
                "Usage: command"
            ),
            "{name}",
        );
    }
}

#[test]
fn both_executables_run_the_exported_browser_bundle_task() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("runtime-entry.js"),
        "globalThis.ready = true;",
    )
    .unwrap();
    for (exe, args) in [
        (
            env!("CARGO_BIN_EXE_platform"),
            &["wasm-bundle", "runtime"][..],
        ),
        (
            env!("CARGO_BIN_EXE_ewe_platform"),
            &["task", "run", "wasm-bundle:runtime", "--check"][..],
        ),
    ] {
        let result = Command::new(exe)
            .args(args)
            .arg("--runtime-dir")
            .arg(dir.path())
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}
