//! Every project template generates a project that compiles.
//!
//! The templates ship with registry-style platform dependencies (the generated
//! project is meant to build outside this repository). This test points those
//! dependencies at the local crates with a `[patch.crates-io]` table, then runs
//! `cargo check` on the generated project. It proves the template produces
//! valtron/`foundation_http` code that compiles, without publishing anything.

use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use foundation_macros::EmbedDirectoryAs;
use foundation_packager::{Directorate, PackageConfig, PackageGenerator};

/// The staged templates from `build.rs` (with `Cargo.toml` restored).
#[derive(EmbedDirectoryAs, Default)]
#[source = "$OUT_DIR/templates/"]
struct ProjectTemplates;

/// `(template name, generated directory name)`.
const TEMPLATES: &[(&str, &str)] = &[
    ("api-app", "http_app"),
    ("static-html-app", "static_html_app"),
    ("web-app", "web_app"),
];

/// Platform crates the templates reference by version, mapped to local paths.
const PLATFORM_CRATES: &[&str] = &[
    "foundation_core",
    "foundation_http",
    "foundation_toolings",
    "foundation_config",
];

#[test]
fn templates_generate_projects_that_build() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let backends = manifest_dir.join("../../backends");

    let work = std::env::temp_dir().join("ewe-templates-build");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("create work dir");

    // One shared target dir so shared dependencies compile once across templates.
    let target_dir = work.join("target");
    let projects_dir = work.join("projects");
    std::fs::create_dir_all(&projects_dir).expect("create projects dir");

    for (template_name, package_name) in TEMPLATES {
        let mut params = serde_json::Map::new();
        params.insert(
            "GITHUB_NAMESPACE".into(),
            "https://github.com/ewe-studios/ewe_platform".into(),
        );
        params.insert("PROJECT_NAME".into(), (*package_name).into());
        params.insert("TEMPLATE_NAME".into(), (*template_name).into());
        params.insert(
            "ROOT_PROJECT_DIRECTORY".into(),
            projects_dir.to_string_lossy().to_string().into(),
        );
        params.insert(
            "PROJECT_DIRECTORY".into(),
            projects_dir
                .join(package_name)
                .to_string_lossy()
                .to_string()
                .into(),
        );

        let config =
            PackageConfig::new(projects_dir.clone(), params, *template_name, *package_name);
        let generator = PackageGenerator::new(Directorate::<ProjectTemplates>::default());
        generator.create(config).expect("generate project");

        let generated_dir = projects_dir.join(package_name);

        // The templates that serve HTML must write responses through the async
        // writer: the synchronous `respond_to_buffer::*` helpers truncate on a
        // non-blocking socket. The API-only template has no handler of its own.
        let source = project_sources(&generated_dir);
        if matches!(*template_name, "web-app" | "static-html-app") {
            assert!(
                source.contains("send_html"),
                "template {template_name} must answer with `send_html`"
            );
            assert!(
                source.contains("send_not_found"),
                "template {template_name} must answer a miss with `send_not_found`"
            );
        }
        assert!(
            !source.contains("respond::") && !source.contains("respond_to_buffer::"),
            "template {template_name} still uses the synchronous buffer helper"
        );

        // Map the registry-style platform dependencies to the local crates.
        let mut patch = String::from("[patch.crates-io]\n");
        for name in PLATFORM_CRATES {
            writeln!(
                patch,
                "{name} = {{ path = \"{}\" }}",
                backends.join(name).display()
            )
            .expect("write patch line");
        }
        std::fs::create_dir_all(generated_dir.join(".cargo")).expect("create .cargo dir");
        std::fs::write(generated_dir.join(".cargo/config.toml"), patch)
            .expect("write patch config");

        let output = Command::new("cargo")
            .arg("check")
            // No `--quiet`: it hides the "patch was not used" warning checked below.
            .arg("--message-format=json")
            .env("CARGO_TARGET_DIR", &target_dir)
            .current_dir(&generated_dir)
            .output()
            .expect("run cargo check");

        let errors = rendered_errors(&String::from_utf8_lossy(&output.stdout));
        let stderr = String::from_utf8_lossy(&output.stderr);
        let manifest = std::fs::read_to_string(generated_dir.join("Cargo.toml"))
            .expect("read the generated Cargo.toml");
        let stale = stale_pins(&stderr, &manifest);
        assert!(
            stale.is_empty(),
            "template {template_name} pins versions that the local crates do not have \
             ({stale:?}); update them in its Cargo.toml.template"
        );
        assert!(
            output.status.success(),
            "cargo check failed for template {template_name}:\n{errors}\n{stderr}"
        );

        // A generated project must build with no warning of its own. Warnings
        // from a patched local dependency are not the template's business.
        let warnings = project_warnings(&output.stdout, &generated_dir);
        assert!(
            warnings.is_empty(),
            "template {template_name} builds with warnings:\n{}",
            warnings.join("\n")
        );
    }
}

/// Every `.rs` file under the generated project's `src/`, joined.
fn project_sources(root: &Path) -> String {
    let mut out = String::new();
    let mut stack = vec![root.join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                out.push_str(&std::fs::read_to_string(&path).unwrap_or_default());
                out.push('\n');
            }
        }
    }
    out
}

/// Warnings from `cargo check --message-format=json` whose source lies inside
/// the generated project.
fn project_warnings(stdout: &[u8], root: &Path) -> Vec<String> {
    let root = root.to_string_lossy();
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|value| value["reason"] == "compiler-message")
        .filter(|value| value["message"]["level"] == "warning")
        .filter(|value| {
            value["target"]["src_path"]
                .as_str()
                .is_some_and(|src| src.starts_with(root.as_ref()))
        })
        .map(|value| {
            value["message"]["rendered"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

/// The rendered text of each compiler error in `cargo --message-format=json`
/// output. The errors are on stdout, one JSON object per line, so without
/// this a failure shows only "could not compile".
fn rendered_errors(stdout: &str) -> String {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|msg| msg["message"]["level"] == "error")
        .filter_map(|msg| msg["message"]["rendered"].as_str().map(str::to_owned))
        .collect()
}

/// The crates of `manifest` whose `[patch]` entry Cargo did not use.
///
/// An unused patch means the template asks for another version than the local
/// crate has (for 0.0.x, "0.0.11" means exactly 0.0.11). Cargo then builds the
/// old crate from crates.io, and the test checks the wrong code. Cargo also
/// warns for patched crates that the template does not use; those are fine.
fn stale_pins(stderr: &str, manifest: &str) -> Vec<String> {
    stderr
        .lines()
        .filter(|line| line.contains("was not used in the crate graph"))
        .filter_map(|line| line.split('`').nth(1)?.split(' ').next())
        .filter(|name| {
            manifest
                .lines()
                .any(|dep| dep.trim_start().starts_with(&format!("{name} ")))
        })
        .map(str::to_owned)
        .collect()
}
