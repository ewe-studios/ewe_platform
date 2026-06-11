//! Init command — scaffold integration directories from embedded templates.
//!
//! WHY: Users need a starting point for writing wasm integration tests.
//! WHAT: Copies template files into the user's crate under `integrations/{type}/`.
//! HOW: Uses `EmbedDirectoryAs` macro for templates (disk reads in debug,
//!      embedded bytes in release). Replaces `PACKAGE_NAME` placeholder.

use foundation_macros::EmbedDirectoryAs;
use foundation_nostd::embeddable::DirectoryData;
use tracing::info;

use crate::cli::{InitArgs, InitType};
use crate::error::{Result, ToTrace, WasmTestbedError};

/// Embedded template directory.
///
/// In debug mode, `read_utf8_for` reads from disk on every call,
/// allowing fast template iteration without recompiling.
/// In release mode, files are embedded as static bytes.
#[derive(EmbedDirectoryAs)]
#[source = "$CURRENT_CRATE/src/templates"]
struct TemplateDirectory;

/// Read a template file by its path within the templates directory.
pub(crate) fn read_template(path: &str) -> Result<String> {
    let bytes = TemplateDirectory.read_utf8_for(path)
        .ok_or_else(|| WasmTestbedError::TemplateNotFound(path.to_string()).trace())?;
    String::from_utf8(bytes)
        .map_err(|e| WasmTestbedError::TemplateNotUtf8(e).trace())
}

/// Write a template to a target directory, replacing `PACKAGE_NAME`.
///
/// The `template_path` is relative to the templates root (e.g. "web/index.html").
/// The `target_dir` is the integration directory where the file will be written.
///
/// # Errors
/// Returns an error when the template is missing/not UTF-8 or the write fails.
pub fn write_template_to(
    template_path: &str,
    package_name: &str,
    target_dir: &std::path::Path,
) -> Result<()> {
    let content = read_template(template_path)?;
    let content = content.replace("PACKAGE_NAME", package_name);

    let file_name = std::path::Path::new(template_path)
        .file_name()
        .ok_or_else(|| WasmTestbedError::InvalidTemplatePath(template_path.to_string()).trace())?;
    let dest = target_dir.join(file_name);

    std::fs::write(&dest, content)
        .map_err(|e| WasmTestbedError::TemplateNotFound(format!("write failed: {e}")).trace())?;
    tracing::debug!("Wrote template to {}", dest.display());
    Ok(())
}

/// Write a bindgen run.js template, replacing `PACKAGE_NAME` and `TEST_NAMES`.
///
/// Reads the template from the given path, replaces `PACKAGE_NAME` with the
/// actual package name, and replaces the `/* TEST_NAMES */` comment with
/// a comma-separated list of quoted test names.
///
/// # Errors
/// Returns an error when the template is missing/not UTF-8 or the write fails.
pub fn write_bindgen_runjs_to(
    template_path: &str,
    package_name: &str,
    tests: &[String],
    target_dir: &std::path::Path,
) -> Result<()> {
    let content = read_template(template_path)?;
    let content = content.replace("PACKAGE_NAME", package_name);

    let test_names = tests
        .iter()
        .map(|t| format!("\"{t}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let content = content.replace("/* TEST_NAMES */", &test_names);

    let file_name = std::path::Path::new(template_path)
        .file_name()
        .ok_or_else(|| WasmTestbedError::InvalidTemplatePath(template_path.to_string()).trace())?;
    let dest = target_dir.join(file_name);

    std::fs::write(&dest, content)
        .map_err(|e| WasmTestbedError::TemplateNotFound(format!("write failed: {e}")).trace())?;
    tracing::debug!("Wrote bindgen template to {}", dest.display());
    Ok(())
}

/// Read the package name from a Cargo.toml file.
fn read_package_name(cargo_toml: &std::path::Path) -> Result<String> {
    let content = std::fs::read_to_string(cargo_toml)
        .map_err(|_e| WasmTestbedError::MissingPackageName(cargo_toml.display().to_string()).trace())?;

    let mut in_package = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if trimmed.starts_with('[') && in_package {
            in_package = false;
            continue;
        }
        if in_package && trimmed.starts_with("name") {
            if let Some(value) = trimmed.split('=').nth(1) {
                let value = value.trim().trim_matches('"').trim_matches('\'');
                return Ok(value.to_string());
            }
        }
    }

    Err(WasmTestbedError::MissingPackageName(cargo_toml.display().to_string()).trace())
}

/// Print a loud warning for every tool the scaffolded mode will need that is
/// missing from PATH — scaffolding still succeeds (the user may install later),
/// but they find out NOW instead of at first run.
fn warn_missing_tools(init_type: &InitType) {
    let needed: &[(&str, &str)] = match init_type {
        InitType::Node => &[
            ("cargo", "builds #[wasm_test] crates to wasm32"),
            ("node", "runs the owned harness (https://nodejs.org)"),
        ],
        InitType::Web => &[
            ("cargo", "builds the wasm module"),
            ("node", "drives Playwright"),
            ("npm", "installs Playwright (`mise run setup:playwright`)"),
        ],
        InitType::Deno => &[
            ("cargo", "builds the wasm module"),
            ("deno", "runs the harness (https://deno.land)"),
        ],
        InitType::Wrangler => &[
            ("cargo", "builds the wasm module"),
            ("npx", "launches wrangler dev"),
        ],
    };
    for (tool, why) in needed {
        if which::which(tool).is_err() {
            tracing::warn!("missing tool `{tool}` — needed because it {why}");
        }
    }
}

/// Entry point for the `init` command.
///
/// # Errors
/// Returns an error when the crate path is missing or scaffolding writes fail.
pub fn run(args: InitArgs) -> Result<()> {
    let crate_path = args.crate_path.canonicalize().map_err(|_| {
        WasmTestbedError::CratePathNotFound(args.crate_path.display().to_string()).trace()
    })?;

    let cargo_toml = crate_path.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(WasmTestbedError::NotACargoCrate(crate_path.display().to_string()).trace());
    }

    let package_name = read_package_name(&cargo_toml)?;
    info!("Scaffolding for package: {package_name}");

    let types = match args.r#type {
        Some(t) => vec![t],
        None => vec![
            InitType::Node,
            InitType::Web,
            InitType::Deno,
            InitType::Wrangler,
        ],
    };

    for init_type in &types {
        warn_missing_tools(init_type);
        // The owned mode scaffolds a Rust cases file, not a JS harness dir — the
        // harness is generated at run time (`wasm-testbed node <crate>`).
        if matches!(init_type, InitType::Node) {
            let dest = crate_path.join("src").join("wasm_tests.rs");
            if dest.exists() {
                info!("{} already exists — skipping", dest.display());
            } else {
                let content = read_template("fwt/sample_wasm_test.rs")?;
                std::fs::write(&dest, content)
                    .map_err(|e| WasmTestbedError::Io(e).trace())?;
                info!(
                    "wrote {} — add `mod wasm_tests;` to lib.rs, set crate-type = [\"cdylib\"], \
                     and depend on foundation_wasm (feature \"web\") + foundation_macros; \
                     then run: wasm-testbed node {}",
                    dest.display(),
                    crate_path.display()
                );
            }
            continue;
        }
        let dir_name = match init_type {
            InitType::Node => unreachable!("handled above"),
            InitType::Web => "web",
            InitType::Deno => "deno",
            InitType::Wrangler => "wrangler",
        };

        let integration_dir = crate_path.join("integrations").join(dir_name);
        std::fs::create_dir_all(&integration_dir)
            .map_err(|e| WasmTestbedError::CratePathNotFound(format!("{e}")).trace())?;

        let templates = match init_type {
            InitType::Node => unreachable!("handled above"),
            InitType::Web => vec!["web/index.html", "web/index.js", "web/loader.js"],
            InitType::Deno => vec!["deno/index.js", "deno/loader.js"],
            InitType::Wrangler => {
                vec!["wrangler/worker.js", "wrangler/loader.js", "wrangler/wrangler.toml"]
            }
        };

        for template_path in &templates {
            write_template_to(template_path, &package_name, &integration_dir)?;
        }

        info!("Scaffolded integrations/{dir_name}/");
    }

    info!("Done. Edit the files in integrations/ to add your test logic.");
    Ok(())
}
