use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

fn check_copy(source: &Path, target: &Path, label: &str) {
    let mut copy_command = Command::new("cp");
    #[cfg(not(target_os = "macos"))]
    copy_command.arg("--no-preserve=mode,ownership");
    match copy_command.arg("-R").arg(source).arg(target).output() {
        Ok(copy) => println!(
            "{label}: exit={:?} exists={} stderr={}",
            copy.status.code(),
            target.is_file(),
            String::from_utf8_lossy(&copy.stderr).trim(),
        ),
        Err(error) => println!("{label}: launch error={error}"),
    }
}

fn main() {
    let scratch = PathBuf::from(env::args_os().nth(1).expect("scratch directory"));
    let source = scratch.join("published-bindgen.rs");
    fs::create_dir_all(&scratch).unwrap();
    fs::write(&source, b"pub const PROBE: bool = true;").unwrap();
    let source_with_slash = PathBuf::from(format!("{}/published-bindgen.rs", scratch.display()));

    for (label, directory) in [
        ("short", scratch.join("case")),
        (
            "deep",
            scratch
                .join("build artifacts with spaces")
                .join("local dependency generation")
                .join("same checkout generated binding path")
                .join("long folder for native build tools"),
        ),
    ] {
        fs::create_dir_all(&directory).unwrap();
        let verbatim = fs::canonicalize(&directory).unwrap();
        let normal_target = directory.join("ordinary-bindgen.rs");
        let verbatim_target = verbatim.join("verbatim-bindgen.rs");
        assert!(!normal_target.exists());
        assert!(!verbatim_target.exists());
        for (form, target) in [("ordinary", normal_target), ("canonical", verbatim_target)] {
            check_copy(&source, &target, &format!("{label} {form} first"));
            check_copy(
                &source_with_slash,
                &target,
                &format!("{label} {form} overwrite"),
            );
        }
    }
}
