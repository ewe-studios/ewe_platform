use std::fmt::Write;
use std::path::{Path, PathBuf};

pub struct AppDistribution {
    pub name: String,
    pub crate_dir: PathBuf,
    pub route_prefix: String,
}

/// Scan the project for WASM app crates and generate platform glue code.
///
/// # Panics
///
/// Panics if `CARGO_MANIFEST_DIR` is not set.
pub fn generate_platform_code() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let project_root = manifest_dir.parent().unwrap().to_path_buf();

    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=build.rs");

    let mut apps = Vec::new();
    let app_dir = project_root.join("app");
    if app_dir.join("Cargo.toml").exists() {
        apps.push(AppDistribution {
            name: "app".into(),
            crate_dir: app_dir,
            route_prefix: "/app/".into(),
        });
    }
    if let Ok(entries) = std::fs::read_dir(&project_root) {
        for e in entries.filter_map(std::result::Result::ok) {
            let p = e.path();
            let n = p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if n.starts_with("app-") && p.is_dir() && p.join("Cargo.toml").exists() {
                // Skip wasmtime shell crates (app-shell, app-shell-*) — handled separately
                if n.starts_with("app-shell") { continue; }
                apps.push(AppDistribution {
                    name: n.clone(),
                    crate_dir: p,
                    route_prefix: format!("/{n}/"),
                });
            }
        }
    }

    // Unconditionally, and before anything that might fail: projects bake the
    // public key with `include_str!("keys/ota_public.key")`, so the file has
    // to exist by the time the crate itself compiles — whether or not this
    // project happens to have any WASM apps.
    let keypair = ensure_ota_keys(&manifest_dir);

    if !apps.is_empty() {
        let public_dir = manifest_dir.join("public");
        // Each app at its crate's own version: `public/{app_id}/v{version}/`.
        // Two apps at different versions naturally land in different
        // directories — no collision, no single version for all.
        for app in &apps {
            let version = crate_version(&app.crate_dir);
            let out = app_version_dir(&public_dir, &app.name, &version);
            build_wasm_app(&app.crate_dir, &out);
            prune_stale_public_versions(&public_dir, &app.name, &version);
        }
        let generated_dir = manifest_dir.join("src").join("generated");
        std::fs::create_dir_all(&generated_dir).ok();
        generate_app_modules(&apps, &generated_dir);
        generate_app_manifests(&public_dir, &apps, &keypair);
    }

    // F33: Discover wasmtime shell apps (surface 3 — in-process WASM).
    // Only generates modules if foundation_wasmtime is in Cargo.toml deps.
    let has_wasmtime_dep = std::fs::read_to_string(manifest_dir.join("Cargo.toml"))
        .map(|c| c.contains("foundation_wasmtime"))
        .unwrap_or(false);
    let mut shells: Vec<AppDistribution> = Vec::new();
    if has_wasmtime_dep {
        let shell_dir = project_root.join("app-shell");
        if shell_dir.join("Cargo.toml").exists() {
            shells.push(AppDistribution {
                name: "app_shell".into(),
                crate_dir: shell_dir,
                route_prefix: "/shell/".into(),
            });
        }
        if let Ok(entries) = std::fs::read_dir(&project_root) {
            for e in entries.filter_map(std::result::Result::ok) {
                let p = e.path();
                let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                if n.starts_with("app-shell-") && p.is_dir() && p.join("Cargo.toml").exists() {
                    shells.push(AppDistribution {
                        name: n.replace('-', "_"),
                        crate_dir: p,
                        route_prefix: format!("/{n}/"),
                    });
                }
            }
        }
        if !shells.is_empty() {
            // Surface 3 modules under `public/` in the same versioned shape as
            // every other app, so Tauri bundles them and an OTA can replace them.
            let public_dir = manifest_dir.join("public");
            for s in &shells {
                let version = crate_version(&s.crate_dir);
                let out = app_version_dir(&public_dir, &s.name, &version);
                build_wasmtime_app(&s.crate_dir, &out);
                prune_stale_public_versions(&public_dir, &s.name, &version);
            }
            let generated_dir = manifest_dir.join("src").join("generated");
            generate_wasmtime_modules(&shells, &generated_dir);
            generate_app_manifests(&public_dir, &shells, &keypair);
        }
    }

    // Patch tauri.conf.json BEFORE tauri_build reads it.
    if let Some(first) = apps.first() {
        patch_tauri_conf_for_ewe(&manifest_dir, &first.route_prefix);
    } else {
        patch_tauri_conf_for_ewe(&manifest_dir, "/__platform__/");
    }
    // Derive resources from what the build actually wrote to public/.
    sync_bundle_resources(&manifest_dir);

    tauri_build::build();
}

/// Read the `version` field from a crate's `Cargo.toml`.
///
/// WHY: each app CRATE owns its version independently. `app/Cargo.toml` at
/// v0.2.0 and `app-hello/Cargo.toml` at v0.1.0 ship to different version
/// directories. Reading a single version from `tauri.conf.json` is one
/// version for every app — the pre-app-first design.
///
/// Falls back to `0.0.0` when the file is unreadable.
#[must_use]
pub fn crate_version(crate_dir: &Path) -> String {
    std::fs::read_to_string(crate_dir.join("Cargo.toml"))
        .ok()
        .and_then(|toml| {
            toml.lines()
                .skip_while(|l| !l.trim_start().starts_with("version"))
                .next()
                .and_then(|l| l.split('=').nth(1))
                .map(|v| v.trim().trim_matches(['"', '\''].as_slice()).to_string())
        })
        .unwrap_or_else(|| "0.0.0".to_string())
}

/// Where an app's assets live in `public/`: `public/{app_id}/v{version}/`.
///
/// Same shape the asset manager serves from at runtime and the layout Tauri
/// bundles — one layout end to end, so nothing translates between them.
#[must_use]
pub fn app_version_dir(public_dir: &Path, app_id: &str, version: &str) -> PathBuf {
    public_dir.join(app_id).join(format!("v{version}"))
}

/// Delete stale version directories for one app under `public/`.
///
/// Only `v{semver}` directories other than the current version are touched.
fn prune_stale_public_versions(public_dir: &Path, app_id: &str, current: &str) {
    let app_dir = public_dir.join(app_id);
    let Ok(entries) = std::fs::read_dir(&app_dir) else { return };
    let current_v = format!("v{current}");
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == current_v || !entry.path().is_dir() { continue }
        let is_version_dir = name.strip_prefix('v').is_some_and(|v| {
            let parts: Vec<&str> = v.split('.').collect();
            parts.len() == 3 && parts.iter().all(|p| p.parse::<u64>().is_ok())
        });
        if is_version_dir {
            std::fs::remove_dir_all(entry.path()).ok();
            println!("cargo:warning=removed stale bundle version {app_id}/{name}");
        }
    }
}

/// Build all discovered WASM app bundles, each at its crate's own version.
///
/// # Panics
///
/// Panics if a WASM app build fails.
pub fn build_all_wasm_apps(apps: &[AppDistribution], public_dir: &Path) {
    for app in apps {
        let version = crate_version(&app.crate_dir);
        build_wasm_app(&app.crate_dir, &app_version_dir(public_dir, &app.name, &version));
    }
}

/// Build a single WASM app for wasmtime (Surface 3 — in-process WASM runtime).
///
/// Scans for `#[wasm_app]` annotations, compiles the crate to `wasm32-wasip1`,
/// and copies the `.wasm` binary to the output directory. Unlike
/// `build_wasm_app` (which targets wasm32-unknown-unknown for the WebView),
/// this targets wasip1 so the module runs in wasmtime with WASI support.
///
/// # Panics
///
/// Panics if `cargo build` fails or the output directory can't be created.
/// Build a wasmtime app crate and copy its artefact to `out_dir`.
///
/// Explicitly called from `build.rs` — no annotation gate here. The caller
/// named this crate, and an annotation-based discovery filter belongs in
/// auto-discovery (`generate_platform_code`), not on explicit calls.
///
/// # Panics
///
/// Panics if `cargo build` fails or the output artefact is missing.
pub fn build_wasmtime_app(app_dir: &Path, out_dir: &Path) {
    println!("cargo:warning=Building wasmtime app: {}", app_dir.display());

    // Cargo sets `$CARGO` in build-script environments to the path of the
    // cargo binary that invoked the script. Using it resolves the same
    // toolchain and avoids PATH issues from nested invocations.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let inner_target = app_dir.join("target");
    let app_dir_display = app_dir.display().to_string();
    let output = std::process::Command::new(&cargo)
        .args([
            "build",
            "--target", "wasm32-wasip1",
            "--release",
            "--target-dir", inner_target.to_str().unwrap_or("target"),
        ])
        .current_dir(app_dir)
        .output()
        .unwrap_or_else(|e| panic!("could not launch {} for wasmtime app {}: {e}", cargo, app_dir_display));

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        for line in stderr.lines().chain(stdout.lines()) {
            if !line.is_empty() {
                println!("cargo:warning=[{}] {}", app_dir_display, line);
            }
        }
        println!(
            "cargo:warning=wasmtime app {} failed (exit {})",
            app_dir_display, output.status,
        );
        return;
    }

    let package = wasm_package_name(app_dir);
    let app_id = app_dir.file_name().and_then(|n| n.to_str()).unwrap_or("wasm_app");

    let wasm_src = inner_target
        .join("wasm32-wasip1/release")
        .join(format!("{package}.wasm"));
    assert!(
        wasm_src.exists(),
        "wasmtime app {} built but produced no module at {}. \
         Expected package name {package:?} — check [package].name in its Cargo.toml.",
        app_dir.display(), wasm_src.display()
    );

    std::fs::create_dir_all(out_dir).ok();
    let wasm_dst = out_dir.join(format!("{app_id}.wasm"));
    std::fs::copy(&wasm_src, &wasm_dst)
        .unwrap_or_else(|e| panic!("failed to copy wasmtime app to output: {e}"));
    println!("cargo:warning=wasmtime app copied to {}", wasm_dst.display());
}

/// The cargo package name for a crate directory.
fn wasm_package_name(app_dir: &Path) -> String {
    std::fs::read_to_string(app_dir.join("Cargo.toml"))
        .ok()
        .and_then(|toml| {
            toml.lines()
                .skip_while(|l| !l.trim_start().starts_with("name"))
                .next()
                .and_then(|l| l.split('=').nth(1))
                .map(|v| v.trim().trim_matches(['"', '\''].as_slice()).to_string())
        })
        .unwrap_or_else(|| {
            app_dir.file_name().and_then(|n| n.to_str()).unwrap_or("wasm_app").replace('-', "_")
        })
}

/// Build a single WASM app from its crate directory.
///
/// # Panics
///
/// Panics if the WASM build pipeline fails.
pub fn build_wasm_app(app_dir: &Path, out_dir: &Path) {
    use foundation_wasm_ui::build_tools::WasmBundleGenerator;

    let wasm: Vec<_> = scan_for_annotations(&app_dir.join("src"))
        .into_iter()
        .filter(|a| {
            matches!(
                a.kind,
                AnnotationKind::WasmBin | AnnotationKind::WasmWorker | AnnotationKind::WasmService
            )
        })
        .collect();
    if wasm.is_empty() {
        return;
    }

    let pairs: Vec<(&str, &str)> = wasm
        .iter()
        .map(|a| (mode_str(a.kind), a.name.as_str()))
        .collect();
    let gen = match WasmBundleGenerator::from_annotations(app_dir, out_dir, &pairs, true) {
        Ok(g) => g,
        Err(e) => {
            println!("cargo:warning=WasmBundleGenerator: {e}");
            return;
        }
    };

    let repo_root = app_dir
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let wasm_js = repo_root.join("backends/foundation_wasm/runtime/foundation-wasm.js");
    let wasm_ui_js = repo_root.join("backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js");
    let interceptor =
        repo_root.join("backends/foundation_wasm_ui/runtimes/platform-scheme-interceptor.js");
    let ipc_bridge =
        repo_root.join("backends/foundation_wasm_ui/runtimes/ipc-bridge.js");
    std::fs::create_dir_all(out_dir).ok();
    match gen.execute(
        false,
        false,
        &[
            ("foundation-wasm.js", wasm_js.as_path()),
            ("ipc-bridge.js", ipc_bridge.as_path()),
            ("foundation-wasm-ui.js", wasm_ui_js.as_path()),
            ("platform-scheme-interceptor.js", interceptor.as_path()),
        ],
    ) {
        Ok(written) => println!(
            "cargo:warning=WasmBundleGenerator: {} files written ({})",
            written.len(),
            written.iter().map(|p| p.path.file_name().unwrap().to_string_lossy().to_string()).collect::<Vec<_>>().join(", ")
        ),
        Err(e) => println!("cargo:warning=WasmBundleGenerator failed: {e}"),
    }
}

/// Generate per-app Rust module files inside `generated/`.
///
/// Generates a `MobileDirectory`-derived struct with a `root: PathBuf` field.
/// At runtime, `AppAssets::build(root)` receives the resolved resource directory
/// from `PlatformSession`, enabling disk-backed asset serving for desktop dev
/// and OTA updates without recompilation (F22).
///
/// # Panics
///
/// Panics if file I/O fails.
fn generate_app_modules(apps: &[AppDistribution], generated_dir: &Path) {
    let mut mod_lines = String::from("// Auto-generated\n\n");
    for app in apps {
        let module_name = app.name.replace('-', "_");
        let content = format!(
            "// Generated \u{2014} AppAssets for \"{name}\" (F22, mounted per F40)\n\
             // Route prefix: {route_prefix}\n\
             //\n\
             // `build(session)` mounts the responder at this app's active version\n\
             // directory and lets it read through the session's asset manager.\n\
             // That indirection is what makes Android work: APK-bundled assets are\n\
             // never on disk, so only the VFS overlay can reach them.\n\
             //\n\
             // `build_at(root)` is the unmounted form \u{2014} a plain disk root, for\n\
             // embeddings that have no PlatformAssetManager.\n\n\
             use foundation_macros::MobileDirectory;\n\
             use foundation_platform::{{MobileApp, PlatformSession}};\n\
             use std::path::PathBuf;\n\
             use std::sync::Arc;\n\n\
             pub const APP_ID: &str = \"{name}\";\n\n\
             #[derive(MobileDirectory)]\n\
             #[source = \"$CARGO_MANIFEST_DIR/public/{public_subdir}\"]\n\
             pub struct AppAssets {{\n    pub root: PathBuf\n}}\n\n\
             impl AppAssets {{\n\
             \x20   pub fn build(session: Arc<PlatformSession>) -> MobileApp<AppAssets> {{\n\
             \x20       MobileApp::mounted_at(AppAssets {{ root: session.app_root(APP_ID) }}, APP_ID)\n\
             \x20   }}\n\n\
             \x20   pub fn build_at(root: PathBuf) -> MobileApp<AppAssets> {{ MobileApp::new(AppAssets {{ root }}) }}\n\
             }}\n",
            name = app.name, route_prefix = app.route_prefix, public_subdir = app.name,
        );
        std::fs::write(generated_dir.join(format!("{module_name}.rs")), &content).ok();
        let _ = writeln!(mod_lines, "pub mod {module_name};");
    }
    std::fs::write(generated_dir.join("mod.rs"), &mod_lines).ok();
    println!("cargo:warning=generated {} app modules", apps.len());
}

/// Create `keys/ota_public.key` (and the private seed) if absent (F40).
///
/// Idempotent: an existing public key is read, never regenerated —
/// regenerating would invalidate every already-shipped binary that baked the
/// old one. Only the public half is meant to be committed; a `keys/.gitignore`
/// is written so the private seed cannot be added by accident. In CI the seed
/// comes from `EWE_OTA_PRIVATE_KEY` and never touches the working tree.
///
/// # Panics
///
/// Panics if the key pair cannot be prepared. Every manifest is signed, so a
/// build that cannot sign cannot produce a shippable bundle — failing here
/// says so, where continuing would produce artefacts every device rejects.
fn ensure_ota_keys(manifest_dir: &Path) -> crate::manifest::KeyPair {
    println!("cargo:rerun-if-env-changed={}", crate::manifest::PRIVATE_KEY_ENV);
    crate::manifest::ensure_keys(manifest_dir).unwrap_or_else(|e| {
        panic!(
            "F40: could not prepare the OTA key pair under {}: {e}\n\
             Manifests are always signed. Either make the directory writable so a \
             key pair can be minted, or set {} to a base64 Ed25519 seed.",
            manifest_dir.display(),
            crate::manifest::PRIVATE_KEY_ENV,
        )
    })
}

/// Write a signed `.ewe_manifest.json` into every app directory under
/// `public/` (F40).
///
/// WHY: a version directory without a manifest is a directory nothing can
/// verify, roll back to, or reason about. Generating them here — rather than
/// asking every project to add a `build.rs` step — means the guarantee
/// "every version directory has a signed manifest" holds without anyone
/// opting in.
///
/// # Panics
///
/// Panics if signing is unavailable or generation fails. There is no unsigned
/// mode to degrade to: a key pair is minted automatically, so the only way to
/// reach this is a broken build environment, and a bundle whose manifests
/// cannot be verified is not worth shipping.
/// Write a signed `.ewe_manifest.json` into each app's version directory.
///
/// Each app's version comes from its own `Cargo.toml` — `app/` at v0.2.0
/// and `app-hello/` at v0.1.0 write manifests into different directories,
/// with their respective versions declared inside the manifest itself.
fn generate_app_manifests(
    public_dir: &Path,
    apps: &[AppDistribution],
    keypair: &crate::manifest::KeyPair,
) {
    println!("cargo:rerun-if-env-changed={}", crate::manifest::MANIFEST_DOMAIN_ENV);

    assert!(
        keypair.can_sign(),
        "F40: no OTA private key is available, so app manifests cannot be signed.\n\
         Set {} to a base64 Ed25519 seed, or let the build mint one by making \
         keys/ writable.",
        crate::manifest::PRIVATE_KEY_ENV,
    );

    let domain = crate::manifest::manifest_domain_from_env();

    for app in apps {
        let version = crate_version(&app.crate_dir);
        let dir = app_version_dir(public_dir, &app.name, &version);
        if !dir.is_dir() {
            continue;
        }
        match crate::manifest::generate_app_manifest(&dir, &app.name, &version, &domain, keypair) {
            Ok(manifest) => println!(
                "cargo:warning=signed manifest for {}/v{version} ({} files)",
                app.name, manifest.apps[0].files.len()
            ),
            Err(e) => panic!("F40: manifest generation for {} failed: {e}", app.name),
        }
    }
}

/// Generate per-app Rust modules for wasmtime shell apps (F33, F40).
///
/// The module is loaded at RUNTIME through the asset manager, not embedded
/// with `include_bytes!`. An embedded module needs a native rebuild and a
/// store release to change — the thing F40 exists to remove.
fn generate_wasmtime_modules(apps: &[AppDistribution], generated_dir: &Path) {
    let shell_dir = generated_dir.join("shell");
    std::fs::create_dir_all(&shell_dir).ok();

    let mut mod_lines = String::from("// Auto-generated (F33 — wasmtime shell, F40 — runtime-loaded)\n\n");

    for app in apps {
        let module_name = app.name.replace('-', "_");
        let app_id = app.name.as_str();
        let version = crate_version(&app.crate_dir);
        let content = format!(
            "// Generated — wasmtime shell for \"{app_id}\" (F33, Surface 3, F40)\n\
             // Route prefix: {route_prefix}\n\
             //\n\
             // The module is bundled at public/{app_id}/v{version}/{app_id}.wasm\n\
             // and loaded at runtime through the asset manager. That is what\n\
             // lets an OTA replace it — embedded bytes would need a native\n\
             // rebuild and a store release to change.\n\n\
             use foundation_platform::PlatformSession;\n\
             use foundation_wasmtime::WasmtimeBuilder;\n\n\
             pub const APP_ID: &str = \"{app_id}\";\n\
             pub const MODULE_FILE: &str = \"{app_id}.wasm\";\n\n\
             /// Load this shell's module for the app's currently active version.\n\
             ///\n\
             /// Returns `None` when the module is absent — a build that did\n\
             /// not produce it, or an OTA mid-flight. The caller decides\n\
             /// whether that is fatal.\n\
             #[must_use]\n\
             pub fn builder(session: &PlatformSession) -> Option<WasmtimeBuilder> {{\n\
             \x20   let manager = session.asset_manager()?;\n\
             \x20   let bytes = manager.read_app_file(APP_ID, MODULE_FILE).ok()?;\n\
             \x20   Some(WasmtimeBuilder::new(bytes).with_name(APP_ID))\n\
             }}\n",
            app_id = app_id,
            route_prefix = app.route_prefix,
            version = version,
        );
        std::fs::write(shell_dir.join(format!("{module_name}.rs")), &content).ok();
        let _ = writeln!(mod_lines, "pub mod {module_name};");
    }
    std::fs::write(shell_dir.join("mod.rs"), &mod_lines).ok();

    let parent_mod = generated_dir.join("mod.rs");
    let existing = std::fs::read_to_string(&parent_mod).unwrap_or_default();
    if !existing.contains("pub mod shell;") {
        let updated = format!("{existing}pub mod shell;\n");
        std::fs::write(&parent_mod, updated).ok();
    }

    println!("cargo:warning=generated {} wasmtime shell modules", apps.len());
}

fn patch_tauri_conf_for_ewe(manifest_dir: &Path, route_prefix: &str) {
    let conf_path = manifest_dir.join("tauri.conf.json");
    let Ok(content) = std::fs::read_to_string(&conf_path) else { return };

    // Tauri's multi-platform config: JSON objects concatenated with newlines.
    // The first is the base; subsequent ones are per-platform overrides
    // (e.g. {"platforms": ["android"], "bundle": {...}}).
    // Patch EVERY object so the base AND all platform overrides get the fix.
    let mut patched_any = false;
    let mut out = String::with_capacity(content.len());
    for segment in content.split("\n\n") {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let Ok(mut val) = serde_json::from_str::<serde_json::Value>(segment) else {
            continue;
        };

        // 1. Route through ewe:// — the custom protocol handler.
        let target_url = format!("ewe://localhost{route_prefix}");
        if let Some(windows) = val
            .get_mut("app")
            .and_then(|a| a.get_mut("windows"))
            .and_then(|w| w.as_array_mut())
            .and_then(|arr| arr.first_mut())
        {
            if windows.get("url").and_then(|u| u.as_str()) != Some(&target_url) {
                windows["url"] = serde_json::Value::String(target_url.clone());
                patched_any = true;
            }
        }

        // 2. Force asset embedding — Android has no dev server.
        //    Tauri in dev mode with devUrl set generates an empty EmbeddedAssets
        //    phf map (tauri-codegen context.rs:178-179). Dropping devUrl and
        //    enabling the bundle forces Tauri to embed all frontendDist files
        //    into the binary so AssetResolver::get() finds them.
        if val.get("build").and_then(|b| b.get("devUrl")).is_some() {
            if let Some(build) = val.get_mut("build").and_then(|b| b.as_object_mut()) {
                build.remove("devUrl");
                patched_any = true;
            }
        }
        if val
            .get("bundle")
            .and_then(|b| b.get("active"))
            .and_then(|a| a.as_bool())
            != Some(true)
        {
            let obj = val.as_object_mut().expect("tauri.conf must be object");
            let bundle = obj
                .entry("bundle".to_string())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
            if let Some(b) = bundle.as_object_mut() {
                b.insert("active".to_string(), serde_json::Value::Bool(true));
                patched_any = true;
            }
        }

        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&serde_json::to_string(&val).unwrap_or_else(|_| segment.to_string()));
    }

    if patched_any && out != content {
        std::fs::write(&conf_path, out).ok();
        println!("cargo:warning=patched tauri.conf.json (ewe URL, devUrl removed, bundle.active=true)");
    }
}

/// Replace `bundle.resources` public/ entries with [`discover_bundle_entries`]
/// output, preserving every non-public entry the project authored.
///
/// Public so tests can drive the config write without touching disk.
pub fn replace_bundle_resources(
    conf: &mut serde_json::Value,
    entries: &[(String, String, String)],
) {
    let Some(bundle) = conf.get_mut("bundle").and_then(serde_json::Value::as_object_mut) else {
        return;
    };
    let mut resources: serde_json::Map<String, serde_json::Value> = bundle
        .get("resources")
        .and_then(serde_json::Value::as_object)
        .map(|existing| {
            existing
                .iter()
                .filter(|(s, _)| !s.starts_with("public/"))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();

    for (source, app_id, version) in entries {
        resources.insert(
            source.clone(),
            serde_json::Value::String(format!("{app_id}/v{version}/")),
        );
    }
    bundle.insert("resources".to_string(), serde_json::Value::Object(resources));
}

/// Derive `bundle.resources` from what `public/` actually contains (F40).
///
/// WHY: a second list (the config) is a second thing to keep in sync with the
/// first (the build output). An app built but absent from the config ships
/// with no assets — a blank page on device. Reading the directory the build
/// just wrote means the config describes exactly what exists, with the
/// version each app actually carries rather than a single version for all.
///
/// Returns the app ids registered.
pub fn sync_bundle_resources(src_tauri_dir: &Path) -> Vec<String> {
    let conf_path = src_tauri_dir.join("tauri.conf.json");
    let Ok(content) = std::fs::read_to_string(&conf_path) else { return Vec::new() };
    let Ok(mut conf) = serde_json::from_str::<serde_json::Value>(&content) else { return Vec::new() };

    let entries = discover_bundle_entries(&src_tauri_dir.join("public"));

    let mut app_ids: Vec<String> = entries.iter().map(|(_, id, _)| id.clone()).collect();
    app_ids.sort();
    app_ids.dedup();

    replace_bundle_resources(&mut conf, &entries);

    if let Ok(patched) = serde_json::to_string_pretty(&conf) {
        if patched != content {
            std::fs::write(&conf_path, &patched).ok();
            println!("cargo:warning=registered {} app(s) in bundle.resources: {}",
                entries.len(), app_ids.join(", "));
        }
    }
    app_ids.sort();
    app_ids.dedup();
    app_ids
}

/// `(source_glob, app_id, version)` for every `{app_id}/v{version}/` under `public/`.
#[must_use]
pub fn discover_bundle_entries(public_dir: &Path) -> Vec<(String, String, String)> {
    let Ok(apps) = std::fs::read_dir(public_dir) else { return Vec::new() };
    let mut entries = Vec::new();
    for app in apps.flatten() {
        if !app.path().is_dir() { continue }
        let app_id = app.file_name().to_string_lossy().to_string();
        let Ok(versions) = std::fs::read_dir(app.path()) else { continue };
        for v in versions.flatten() {
            let name = v.file_name().to_string_lossy().to_string();
            let bare = match name.strip_prefix('v') {
                Some(b) => b.to_string(),
                None => continue,
            };
            if !v.path().is_dir() { continue }
            let parts: Vec<&str> = bare.split('.').collect();
            if parts.len() != 3 || parts.iter().any(|p| p.parse::<u64>().is_err()) { continue }
            entries.push((
                format!("public/{app_id}/v{bare}/**/*"),
                app_id.clone(),
                bare,
            ));
        }
    }
    entries.sort();
    entries
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub name: String,
    pub kind: AnnotationKind,
    pub file: PathBuf,
    pub target: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationKind {
    WasmBin,
    WasmWorker,
    WasmService,
    PlatformBin,
    /// Surface 3: WASM compiled to wasm32-wasip1, hosted in wasmtime.
    WasmApp,
}
fn mode_str(k: AnnotationKind) -> &'static str {
    match k {
        AnnotationKind::WasmBin => "wasm_bin",
        AnnotationKind::WasmWorker => "wasm_worker",
        AnnotationKind::WasmService => "wasm_service",
        AnnotationKind::PlatformBin => "platform_bin",
        AnnotationKind::WasmApp => "wasm_app",
    }
}

#[must_use]
pub fn scan_for_annotations(dir: &Path) -> Vec<Annotation> {
    let mut v = Vec::new();
    let Ok(es) = std::fs::read_dir(dir) else {
        return v;
    };
    for e in es.filter_map(std::result::Result::ok) {
        let p = e.path();
        if p.is_dir()
            && p.file_name()
                .is_some_and(|n| n != "target" && !n.to_string_lossy().starts_with('.'))
        {
            v.extend(scan_for_annotations(&p));
        } else if p.extension().is_some_and(|x| x == "rs") {
            if let Ok(c) = std::fs::read_to_string(&p) {
                let ls: Vec<&str> = c.lines().collect();
                for i in 0..ls.len() {
                    let t = ls[i].trim();
                    let kind = if t.starts_with("#[wasm_bin") {
                        Some(AnnotationKind::WasmBin)
                    } else if t.starts_with("#[wasm_worker") {
                        Some(AnnotationKind::WasmWorker)
                    } else if t.starts_with("#[wasm_service") {
                        Some(AnnotationKind::WasmService)
                    } else if t.starts_with("#[platform_bin") {
                        Some(AnnotationKind::PlatformBin)
                    } else if t.starts_with("#[wasm_app") {
                        Some(AnnotationKind::WasmApp)
                    } else {
                        None
                    };
                    if let Some(k) = kind {
                        let name = (i..ls.len()).find_map(|j| {
                            ls[j].find("fn ").map(|p2| {
                                ls[j][p2 + 3..]
                                    .split(|c: char| !c.is_alphanumeric() && c != '_')
                                    .find(|s| !s.is_empty())
                                    .unwrap_or("unknown")
                                    .to_string()
                            })
                        });
                        // Parse annotation target: #[wasm_bin(target = "wasip1", ...)]
                        let target = t.find("target")
                            .and_then(|pos| t[pos + 6..].trim_start().strip_prefix('='))
                            .map(|rest| rest.trim().trim_matches('"').trim_matches('#'))
                            .map(|s| s.split(',').next().unwrap_or(s).trim_matches('"').to_string())
                            .unwrap_or_else(|| String::from("wasm32-unknown-unknown"));
                        if let Some(n) = name {
                            v.push(Annotation {
                                name: n,
                                kind: k,
                                file: p.clone(),
                                target,
                            });
                        }
                    }
                }
            }
        }
    }
    v
}
