//! Every project template generates a project that compiles.
//!
//! The templates ship with registry-style platform dependencies (the generated
//! project is meant to build outside this repository). This test points those
//! dependencies at the local crates with a `[patch.crates-io]` table, then runs
//! `cargo check` on the generated project. It proves the template produces
//! valtron/`foundation_http` code that compiles, without publishing anything.

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

        let project_dir = projects_dir.join(package_name);

        // Map the registry-style platform dependencies to the local crates.
        let mut patch = String::from("[patch.crates-io]\n");
        for name in PLATFORM_CRATES {
            patch.push_str(&format!(
                "{name} = {{ path = \"{}\" }}\n",
                backends.join(name).display()
            ));
        }
        std::fs::create_dir_all(project_dir.join(".cargo")).expect("create .cargo dir");
        std::fs::write(project_dir.join(".cargo/config.toml"), patch).expect("write patch config");

        let status = Command::new("cargo")
            .arg("check")
            .arg("--quiet")
            .env("CARGO_TARGET_DIR", &target_dir)
            .current_dir(&project_dir)
            .status()
            .expect("run cargo check");

        assert!(
            status.success(),
            "cargo check failed for template {template_name}"
        );
    }
}
