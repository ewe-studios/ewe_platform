//! Exercise the installed command paths in a small independent workspace.
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn write(root: &Path, name: &str, body: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}
fn command(root: &Path, program: &str, args: &[&str]) -> Output {
    Command::new(program)
        .current_dir(root)
        .args(args)
        .env_remove("CI_PACKAGES")
        .env_remove("CI_BASE_SHA")
        .env_remove("CI_HEAD_SHA")
        .env_remove("GITHUB_ACTIONS")
        .env_remove("GITHUB_BASE_REF")
        .env_remove("GITHUB_STEP_SUMMARY")
        .env("GITHUB_OUTPUT", root.join("output"))
        .output()
        .unwrap()
}
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    write_fixture(dir.path());
    dir
}

fn write_fixture(root: &Path) {
    write(
        root,
        "Cargo.toml",
        "[workspace]\nresolver='2'\nmembers=['core','app']\n",
    );
    write(
        root,
        "core/Cargo.toml",
        "[package]\nname='tiny-core'\nversion='0.1.0'\nedition='2021'\n[features]\nlinux=[]\n",
    );
    write(root, "core/src/lib.rs", "");
    write(
        root,
        "app/Cargo.toml",
        "[package]\nname='tiny-app'\nversion='0.1.0'\nedition='2021'\n[dependencies]\ntiny-core={path='../core'}\n",
    );
    write(root, "app/src/lib.rs", "");
    write(
        root,
        "ci/packages.toml",
        "[defaults]\nlinux-only-features=['linux']\nmacos-shards=1\n[packages.tiny-core]\nruns=['','linux']\n[packages.tiny-app]\nwindows-tests='none'\nwindows-reason='missing test tool'\n",
    );
}

#[test]
fn plan_writes_counts_and_respects_platform_settings() {
    let f = fixture();
    let root = f.path();
    let result = command(
        root,
        env!("CARGO_BIN_EXE_foundation-ci"),
        &["plan", "--packages", "tiny-core,tiny-app"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let matrix: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(matrix["include"][0]["package"], "tiny-app");
    let output = fs::read_to_string(root.join("output")).unwrap();
    let keys: std::collections::BTreeMap<_, _> =
        output.lines().map(|l| l.split_once('=').unwrap()).collect();
    assert_eq!(keys["macos-count"], "2");
    assert_eq!(keys["windows-count"], "1");
    let mac: serde_json::Value = serde_json::from_str(keys["macos-matrix"]).unwrap();
    assert_eq!(mac["include"][0]["packages"], "tiny-app tiny-core");
    assert_eq!(mac["include"][0]["weight"], 2);
}
#[test]
fn affected_selects_the_consumer_and_excludes_prose() {
    let f = fixture();
    let root = f.path();
    assert!(command(root, "cargo", &["generate-lockfile"])
        .status
        .success());
    for args in [
        &["init", "-q"][..],
        &["add", "."],
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "source",
        ],
    ] {
        assert!(command(root, "git", args).status.success());
    }
    write(root, "core/src/lib.rs", "pub fn change() {}\n");
    write(root, "app/README.md", "prose");
    let output = command(
        root,
        env!("CARGO_BIN_EXE_foundation-ci"),
        &["affected", "--base", "HEAD", "--json"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        parsed["affected"]["tiny-app"],
        "depends on tiny-core (core/src/lib.rs changed)"
    );
}
#[test]
fn shared_runner_change_selects_all_packages() {
    let f = fixture();
    let root = f.path();
    assert!(command(root, "cargo", &["generate-lockfile"])
        .status
        .success());
    assert!(command(root, "git", &["init", "-q"]).status.success());
    assert!(command(root, "git", &["add", "."]).status.success());
    fixture_commit(root);
    write(
        root,
        "backends/foundation_ci/src/execution/recipes.rs",
        "// changed",
    );
    assert!(command(root, "git", &["add", "."]).status.success());
    let result = command(
        root,
        env!("CARGO_BIN_EXE_foundation-ci"),
        &["affected", "--base", "HEAD", "--json"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let affected = report["affected"].as_object().unwrap();
    assert_eq!(affected.len(), 2);
    assert_eq!(
        affected["tiny-core"],
        "backends/foundation_ci/src/execution/recipes.rs changed"
    );
}

#[test]
fn plan_rejects_an_unexplained_opt_out() {
    let f = fixture();
    let root = f.path();
    write(
        root,
        "ci/packages.toml",
        "[packages.tiny-app]\nmacos-tests='none'\n",
    );
    let output = command(
        root,
        env!("CARGO_BIN_EXE_foundation-ci"),
        &["plan", "--packages", "all"],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("reason"));
}

#[test]
fn build_groups_follow_configured_paths_and_member_references() {
    let dir = fixture();
    let root = dir.path();
    std::fs::create_dir_all(root.join("other/examples")).unwrap();
    std::fs::create_dir_all(root.join("core/integration/wasm")).unwrap();
    let policy = "[groups.demo]\ndirectory='other/examples'\nrecipe='example-applications'\nbuild-members=['tiny-app']\n[groups.suites]\ndirectory='core/integration/wasm'\nrecipe='wasm-runtime-tests'\nbuild-members=[]\n[packages.suites]\nwindows-tests='none'\nreason='browser dependency'\n";
    write(root, "ci/packages.toml", policy);
    let result = command(
        root,
        env!("CARGO_BIN_EXE_foundation-ci"),
        &["plan", "--packages", "demo,suites"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let matrix: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(matrix["include"].as_array().unwrap().len(), 2);
    let summary = fs::read_to_string(root.join("output")).unwrap();
    assert!(summary.lines().any(|l| l == "windows-count=1"));
    assert!(summary.lines().any(|l| l == "macos-count=2"));
    for args in [
        &["init", "-q"][..],
        &["add", "."],
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "group inputs",
        ],
    ] {
        assert!(command(root, "git", args).status.success());
    }
    write(root, "core/integration/wasm/change.rs", "");
    let result = command(
        root,
        env!("CARGO_BIN_EXE_foundation-ci"),
        &["affected", "--base", "HEAD", "--json"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let affected: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        affected["affected"],
        serde_json::json!({"suites":"core/integration/wasm/change.rs changed"})
    );
    for (from, to, expected) in [
        ("tiny-app", "nonexistent", "unknown Cargo member"),
        ("other/examples", "core", "already belongs"),
        ("other/examples", "../", "relative directory"),
        ("example-applications", "unknown-recipe", "unknown variant"),
    ] {
        write(root, "ci/packages.toml", &policy.replace(from, to));
        let result = command(
            root,
            env!("CARGO_BIN_EXE_foundation-ci"),
            &["plan", "--packages", "all"],
        );
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn manifest_consumer_change_selects_the_configured_contract_group() {
    let dir = fixture();
    let root = dir.path();
    assert!(command(root, "cargo", &["generate-lockfile"])
        .status
        .success());
    write(root, "contract/schema.json", "{}");
    let copy = "backends/foundation_installer/tests/fixtures/manifest/sample.json";
    write(root, copy, "{}");
    write(
        root,
        "ci/packages.toml",
        "[groups.schema-checks]\ndirectory='contract'\nrecipe='manifest-contract'\nbuild-members=[]\n",
    );
    for args in [
        &["init", "-q"][..],
        &["add", "."],
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "schema fixture",
        ],
    ] {
        assert!(command(root, "git", args).status.success());
    }
    write(root, copy, r#"{"version": 2}"#);
    let result = command(
        root,
        env!("CARGO_BIN_EXE_foundation-ci"),
        &["affected", "--base", "HEAD", "--json"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let changes: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        changes["affected"]["schema-checks"],
        format!("{copy} changed")
    );
}

#[test]
fn shard_execution_continues_and_reports_failures_at_the_end() {
    let dir = fixture();
    let root = dir.path();
    write(
        root,
        "core/src/lib.rs",
        "#[test] fn failing_example() { panic!(\"fixture failure text\"); }",
    );
    write(root, "app/src/lib.rs", "#[test] fn later_example() {}");
    let output = command(
        root,
        env!("CARGO_BIN_EXE_foundation-ci"),
        &[
            "run-many",
            "tiny-core tiny-app",
            "--step",
            "test",
            "--profile",
            "dev",
        ],
    );
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.contains("later_example ... ok"), "{stdout}");
    assert!(stdout.contains("2 package(s), 1 failed"), "{stdout}");
    assert!(
        stderr.contains("failing_example") && stderr.contains("fixture failure text"),
        "{stderr}"
    );
}

#[test]
fn package_build_uses_the_callers_checkout_path() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir
        .path()
        .join("workspace path with spaces")
        .join("build script output path")
        .join("third party fixtures")
        .join("child package runs");
    write_fixture(&root);
    write(
        &root,
        "core/build.rs",
        r#"
fn main() {
    let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    if cfg!(windows) {
        assert!(!output.to_string_lossy().starts_with(r"\\?\"), "{output:?}");
    }
    std::fs::write(output.join("binding.rs"), "const BOUND: bool = true;").unwrap();
}
"#,
    );
    write(
        &root,
        "core/src/lib.rs",
        r#"#[cfg(test)]
mod binding {
    include!(concat!(env!("OUT_DIR"), "/binding.rs"));
    #[test]
    fn generated_binding() { assert!(std::hint::black_box(BOUND)); }
}"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
        .current_dir(root.join("core"))
        .env_remove("CARGO_TARGET_DIR")
        .args(["run", "tiny-core", "--step", "test", "--profile", "dev"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("binding::generated_binding ... ok"));
}

#[test]
fn source_guard_checks_production_code_and_its_allowlist() {
    let dir = fixture();
    let root = dir.path();
    let source = "backends/widget/src/lib.rs";
    write(
        root,
        source,
        "fn sample() { let _ = std::time::SystemTime::now(); }",
    );
    let empty = root.join("allowlist");
    fs::write(&empty, "").unwrap();
    let run = || {
        command(
            root,
            env!("CARGO_BIN_EXE_foundation-ci"),
            &[
                "check-time-sources",
                "--root",
                root.to_str().unwrap(),
                "--allowlist",
                empty.to_str().unwrap(),
                "--check",
            ],
        )
    };
    let failed = run();
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains(source));
    fs::write(&empty, format!("{source} # allowed fixture\n")).unwrap();
    assert!(run().status.success());
    write(root, source, "fn sample() {}");
    let unused = run();
    assert!(!unused.status.success());
    assert!(String::from_utf8_lossy(&unused.stderr).contains("unused"));
}

#[test]
fn chromium_uses_the_configured_browser_or_a_complete_cache() {
    let root = tempfile::tempdir().unwrap();
    let cache = root.path().join("browser");
    fs::create_dir(&cache).unwrap();
    let binary = cache.join("chrome");
    let configured = root.path().join("selected");
    fs::write(&binary, "").unwrap();
    fs::write(cache.join("icudtl.dat"), "").unwrap();
    fs::write(&configured, "").unwrap();
    for selected in [Some(&configured), None] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_foundation-ci"));
        command
            .current_dir(root.path())
            .arg("prepare-chromium")
            .env("FOUNDATION_CHROMIUM_DIR", &cache)
            .env_remove("PRIMAL_TEST_CHROMIUM_BIN");
        if let Some(file) = selected {
            command.env("PRIMAL_TEST_CHROMIUM_BIN", file);
        }
        let result = command.output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout)
            .contains(selected.unwrap_or(&binary).to_str().unwrap()));
    }
}

#[test]
fn expired_package_deadline_reports_the_failing_command() {
    let fixture = fixture();
    write(
        fixture.path(),
        "ci/packages.toml",
        "[defaults]\ntimeout-minutes=0\n",
    );
    let result = command(
        fixture.path(),
        env!("CARGO_BIN_EXE_foundation-ci"),
        &["run", "tiny-app", "--step", "test", "--profile", "dev"],
    );
    assert!(!result.status.success());
    let message = String::from_utf8_lossy(&result.stderr);
    assert!(
        message.contains("tiny-app") && message.contains("timed out after 0 minutes"),
        "{message}"
    );
}

#[test]
fn lockfile_check_handles_spaces_and_reports_drift() {
    let fixture = fixture();
    let root = fixture.path();
    let nested = root.join("outside workspace");
    write(
        &nested,
        "Cargo.toml",
        "[workspace]\n[package]\nname='nested'\nversion='0.1.0'\nedition='2021'\n",
    );
    write(&nested, "src/lib.rs", "");
    assert!(command(&nested, "cargo", &["generate-lockfile"])
        .status
        .success());
    assert!(command(root, "git", &["init", "-q"]).status.success());
    assert!(command(root, "git", &["add", "."]).status.success());
    let check = || {
        command(
            root,
            env!("CARGO_BIN_EXE_foundation-ci"),
            &["check-lockfiles"],
        )
    };
    let pass = check();
    assert!(
        pass.status.success(),
        "{}",
        String::from_utf8_lossy(&pass.stderr)
    );
    assert!(String::from_utf8_lossy(&pass.stdout).contains("outside workspace"));
    let path = nested.join("Cargo.toml");
    fs::write(
        &path,
        fs::read_to_string(&path).unwrap().replace("0.1.0", "0.1.1"),
    )
    .unwrap();
    assert!(!check().status.success());
}

#[test]
fn release_package_cycles_follow_required_normal_and_build_dependencies() {
    let dir = fixture();
    let root = dir.path();
    let base = "[package]\nname='tiny-core'\nversion='0.1.0'\nedition='2021'\n";
    let normal = "[package]\nname='tiny-app'\nversion='0.1.0'\nedition='2021'\n";
    let development = "[dev-dependencies]\ntiny-app={path='../app',version='0.1'}\n";
    let back = "[dependencies]\ntiny-core={path='../core',version='0.1'}\n";
    for (metadata, dev, path, cycle) in [
        ("", development, back, true),
        (
            "",
            development,
            "[build-dependencies]\ntiny-core={path='../core',version='0.1'}",
            true,
        ),
        (
            "",
            "[dev-dependencies]\ntiny-app={path='../app'}",
            back,
            false,
        ),
        ("", "[dev-dependencies]\ntiny-app='0.1'", back, false),
        (
            "",
            development,
            "[dev-dependencies]\ntiny-core={path='../core'}",
            false,
        ),
        (
            "",
            development,
            "[dependencies]\ntiny-core={path='../core',optional=true}",
            false,
        ),
        ("publish=false\n", development, back, false),
        ("", development, "", false),
    ] {
        write(root, "core/Cargo.toml", &format!("{base}{metadata}{dev}"));
        write(root, "app/Cargo.toml", &format!("{normal}{path}"));
        let result = command(
            root,
            env!("CARGO_BIN_EXE_foundation-ci"),
            &["package-workspace", "--check-cycles"],
        );
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert_eq!(!result.status.success(), cycle, "{stderr}");
        if cycle {
            assert!(
                stderr.contains("tiny-core -(dev)-> tiny-app -> tiny-core"),
                "{stderr}"
            );
        }
    }
}

#[test]
fn release_cycle_reports_the_complete_indirect_path() {
    let dir = fixture();
    let root = dir.path();
    write(
        root,
        "Cargo.toml",
        "[workspace]\nresolver='2'\nmembers=['core','app','middle']\n",
    );
    write(root, "core/Cargo.toml", "[package]\nname='tiny-core'\nversion='0.1.0'\nedition='2021'\n[dev-dependencies]\ntiny-app={path='../app',version='0.1'}\n");
    write(root, "app/Cargo.toml", "[package]\nname='tiny-app'\nversion='0.1.0'\nedition='2021'\n[build-dependencies]\nmiddle={path='../middle'}\n");
    write(root, "middle/Cargo.toml", "[package]\nname='middle'\nversion='0.1.0'\nedition='2021'\n[dependencies]\ntiny-core={path='../core'}\n");
    write(root, "middle/src/lib.rs", "");
    let result = command(
        root,
        env!("CARGO_BIN_EXE_foundation-ci"),
        &["package-workspace", "--check-cycles"],
    );
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr)
        .contains("tiny-core -(dev)-> tiny-app -> middle -> tiny-core"));
}

#[test]
fn release_archive_builds_outside_the_checkout_and_skips_private_members() {
    let dir = fixture();
    let root = dir.path();
    write(
        root,
        "core/Cargo.toml",
        "[package]\nname='infrastructure_llama_bindings'\nversion='0.1.0'\nedition='2021'\n",
    );
    write(
        root,
        "core/build.rs",
        "fn main() { assert!(std::env::var_os(\"LLAMA_DIR\").is_none()); }",
    );
    write(
        root,
        "app/Cargo.toml",
        "[package]\nname='tiny-app'\nversion='0.1.0'\nedition='2021'\npublish=false\n",
    );
    let result = Command::new(env!("CARGO_BIN_EXE_foundation-ci"))
        .current_dir(root)
        .env("LLAMA_DIR", "workspace-only")
        .env("CARGO_NET_OFFLINE", "true")
        .env("CARGO_TARGET_DIR", root.join("build output"))
        .arg("package-workspace")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("packaged 1 publishable crates"));
    assert!(root
        .join("build output/package/infrastructure_llama_bindings-0.1.0.crate")
        .is_file());
}

fn fixture_commit(dir: &Path) {
    let result = command(
        dir,
        "git",
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "commit",
            "-qm",
            "fixture",
        ],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn pinned_submodule_installs_a_baked_copy_and_preserves_existing_files() {
    let fixture = tempfile::tempdir().unwrap();
    let root = fixture.path().join("checkout");
    let baked = fixture.path().join("baked");
    let name = "tools/source";
    let repo = baked.join(name);
    for dir in [&root, &repo] {
        fs::create_dir_all(dir).unwrap();
        assert!(command(dir, "git", &["init", "-q"]).status.success());
    }
    write(&repo, "hello.txt", "baked source");
    assert!(command(&repo, "git", &["add", "."]).status.success());
    fixture_commit(&repo);
    let id = String::from_utf8(command(&repo, "git", &["rev-parse", "HEAD"]).stdout).unwrap();
    let sha = id.trim();
    write(
        &root,
        ".gitmodules",
        &format!(
            "[submodule \"source\"]\npath = {name}\nurl = https://example.invalid/source.git\n"
        ),
    );
    assert!(command(&root, "git", &["add", ".gitmodules"])
        .status
        .success());
    assert!(command(
        &root,
        "git",
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{sha},{name}")
        ]
    )
    .status
    .success());
    fixture_commit(&root);
    let lock = baked.join("submodules.lock.json");
    fs::write(
        &lock,
        serde_json::json!({"submodules":[{"path":name,"sha":sha}]}).to_string(),
    )
    .unwrap();
    let visit = |mode: &str| {
        command(
            &root,
            env!("CARGO_BIN_EXE_foundation-ci"),
            &[
                "link-submodules",
                "--lock",
                lock.to_str().unwrap(),
                "--mode",
                mode,
            ],
        )
    };
    write(&root, &format!("{name}/user.txt"), "keep");
    assert!(!visit("symlink").status.success());
    assert_eq!(
        fs::read_to_string(root.join(name).join("user.txt")).unwrap(),
        "keep"
    );
    fs::remove_file(root.join(name).join("user.txt")).unwrap();
    let result = visit("symlink");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        fs::read_to_string(root.join(name).join("hello.txt")).unwrap(),
        "baked source"
    );
    let result = visit("submodule");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(root.join(name).read_link().is_err());
    assert!(visit("submodule").status.success());
}
