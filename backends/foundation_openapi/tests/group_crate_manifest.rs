//! The manifest of a generated group crate names each sibling crate's real
//! version.
//!
//! A hard-coded version went stale. The generator wrote
//! `foundation_netio = "0.0.4"` while the crate was 0.0.6, so every build found
//! the committed manifests stale, rewrote them, and the next `cargo metadata`
//! failed: no version matched the requirement.

use std::fs;
use std::path::{Path, PathBuf};

use foundation_openapi::unified::{group_crate_manifest, GenError};

/// The four crates a Cloudflare group crate depends on by path, with versions
/// that no real crate in the workspace uses, so the assertion cannot pass by
/// accident.
const SIBLINGS: &[(&str, &str)] = &[
    ("foundation_cloudflare_types", "9.1.0"),
    ("foundation_deployment", "9.2.0"),
    ("foundation_macros", "9.3.0"),
    ("foundation_netio", "9.4.0"),
];

/// A workspace-shaped tree. The sibling crates sit under `backends/`; the
/// group crates are rendered under `scratch/`, which holds no sibling crate —
/// the same shape `Codegen::check` renders into.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "foundation_openapi_group_manifest_{name}_{}",
            std::process::id()
        ));
        // A leftover tree from a killed run must not pass the test for us.
        let _ = fs::remove_dir_all(&root);
        let backends = root.join("backends");
        for (crate_name, version) in SIBLINGS {
            let dir = backends.join(crate_name);
            fs::create_dir_all(&dir).expect("create the sibling crate");
            fs::write(
                dir.join("Cargo.toml"),
                format!("[package]\nname = \"{crate_name}\"\nversion = \"{version}\"\n"),
            )
            .expect("write the sibling manifest");
        }
        Self { root }
    }

    fn siblings(&self) -> PathBuf {
        self.root.join("backends")
    }

    fn scratch_group_crate(&self) -> PathBuf {
        self.root.join("scratch/foundation_cloudflare_access")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn each_path_dependency_requires_its_siblings_version() {
    let fixture = Fixture::new("versions");
    let manifest = group_crate_manifest(
        &fixture.siblings(),
        "foundation_cloudflare_access",
        "access",
    )
    .expect("render the manifest");

    for (crate_name, version) in SIBLINGS {
        let expected =
            format!("{crate_name} = {{ path = \"../{crate_name}\", version = \"{version}\"");
        assert!(
            manifest.contains(&expected),
            "{crate_name} must require the sibling crate's own version {version}:\n{manifest}"
        );
    }
}

/// The versions come from the shipped workspace, not from the directory being
/// rendered into.
///
/// WHY: `Codegen::check` renders the group crates into a scratch tree. That tree
/// has no sibling crates in it, and reading the versions from next to the output
/// failed the Cloudflare build script with "No such file or directory".
#[test]
fn a_scratch_render_reads_the_shipped_workspace() {
    let fixture = Fixture::new("scratch");
    let group_dir = fixture.scratch_group_crate();
    fs::create_dir_all(&group_dir).expect("create the scratch group crate");
    assert!(
        !group_dir
            .parent()
            .expect("the group crate has a parent")
            .join("foundation_netio")
            .exists(),
        "the test only means something when the output tree has no siblings"
    );

    let manifest = group_crate_manifest(
        &fixture.siblings(),
        "foundation_cloudflare_access",
        "access",
    )
    .expect("a scratch render still names the shipped versions");
    for (crate_name, version) in SIBLINGS {
        assert!(
            manifest.contains(&format!("version = \"{version}\"")),
            "{crate_name} must be required by {version}:\n{manifest}"
        );
    }
}

#[test]
fn a_sibling_without_a_version_names_its_manifest() {
    let fixture = Fixture::new("no_version");
    let path = fixture
        .siblings()
        .join("foundation_deployment")
        .join("Cargo.toml");
    fs::write(&path, "[package]\nname = \"foundation_deployment\"\n").expect("drop the version");

    let err = group_crate_manifest(
        &fixture.siblings(),
        "foundation_cloudflare_access",
        "access",
    )
    .expect_err("a path dependency needs a version");
    match &err {
        GenError::ManifestVersion { path: named, .. } => {
            assert_eq!(
                named,
                &path.display().to_string(),
                "the error names the manifest"
            );
        }
        other => panic!("expected ManifestVersion, got {other}"),
    }
}

/// Every committed Cloudflare group crate requires the version its sibling
/// crate actually has.
///
/// The generator writes these manifests, but a hand edit or an outdated
/// generator leaves a requirement behind, and then `cargo metadata` fails for
/// the whole workspace — with a message that names the requirement, not the
/// generator that wrote it.
#[test]
fn committed_group_manifests_require_current_sibling_versions() {
    let backends = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crate sits under backends/");

    let mut checked = 0_u32;
    for entry in fs::read_dir(backends).expect("read the backends directory") {
        let dir = entry.expect("a directory entry").path();
        let name = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        if !name.starts_with("foundation_cloudflare_") {
            continue;
        }
        let Ok(raw) = fs::read_to_string(dir.join("Cargo.toml")) else {
            continue;
        };
        let doc: toml_edit::DocumentMut = raw.parse().expect("a manifest parses");
        let Some(deps) = doc
            .get("dependencies")
            .and_then(toml_edit::Item::as_table_like)
        else {
            continue;
        };
        for (dependency, _) in SIBLINGS {
            let Some(requirement) = deps
                .get(dependency)
                .and_then(toml_edit::Item::as_table_like)
            else {
                continue;
            };
            let Some(required) = requirement.get("version").and_then(toml_edit::Item::as_str)
            else {
                continue;
            };
            let sibling = backends.join(dependency).join("Cargo.toml");
            let sibling_raw =
                fs::read_to_string(&sibling).expect("the sibling manifest is present");
            let sibling_doc: toml_edit::DocumentMut =
                sibling_raw.parse().expect("the sibling manifest parses");
            let actual = sibling_doc
                .get("package")
                .and_then(|package| package.get("version"))
                .and_then(toml_edit::Item::as_str)
                .expect("the sibling names its version");
            assert_eq!(
                required, actual,
                "{name} requires {dependency} {required}, but the crate is {actual}"
            );
            checked += 1;
        }
    }
    assert!(checked > 0, "no generated group crate was checked");
}
