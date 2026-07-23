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
        build_all_wasm_apps(&apps, &public_dir);
        let generated_dir = manifest_dir.join("src").join("generated");
        std::fs::create_dir_all(&generated_dir).ok();
        generate_app_modules(&apps, &generated_dir);
        // After the bundles exist and before Tauri packages them — the
        // manifest describes exactly the bytes that ship (F40).
        generate_app_manifests(&public_dir, &keypair);
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
            let shell_out = manifest_dir.join("shell_wasm");
            std::fs::create_dir_all(&shell_out).ok();
            for s in &shells {
                build_wasmtime_app(&s.crate_dir, &shell_out);
            }
            let generated_dir = manifest_dir.join("src").join("generated");
            generate_wasmtime_modules(&shells, &generated_dir);
        }
    }

    // Patch tauri.conf.json BEFORE tauri_build reads it.
    // Sets the initial window URL to bypass WebViewAssetLoader.
    if let Some(first) = apps.first() {
        patch_tauri_conf_for_ewe(&manifest_dir, &first.route_prefix);
    } else {
        patch_tauri_conf_for_ewe(&manifest_dir, "/__platform__/");
    }

    tauri_build::build();
}

/// Build all discovered WASM app bundles.
///
/// # Panics
///
/// Panics if a WASM app build fails.
pub fn build_all_wasm_apps(apps: &[AppDistribution], public_dir: &Path) {
    for app in apps {
        build_wasm_app(&app.crate_dir, &public_dir.join(&app.name));
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
pub fn build_wasmtime_app(app_dir: &Path, out_dir: &Path) {
    let apps: Vec<_> = scan_for_annotations(&app_dir.join("src"))
        .into_iter()
        .filter(|a| matches!(a.kind, AnnotationKind::WasmApp))
        .collect();
    if apps.is_empty() {
        return;
    }

    println!("cargo:warning=Building wasmtime app: {}", app_dir.display());

    let status = std::process::Command::new("cargo")
        .args(["build", "--target", "wasm32-wasip1", "--release"])
        .current_dir(app_dir)
        .status()
        .unwrap_or_else(|e| panic!("cargo build failed for wasmtime app {}: {e}", app_dir.display()));

    if !status.success() {
        println!("cargo:warning=wasmtime app build failed (exit {status})");
        return;
    }

    let name = app_dir.file_name().and_then(|n| n.to_str()).unwrap_or("wasm_app");
    let wasm_src = app_dir
        .join("target/wasm32-wasip1/release")
        .join(format!("{name}.wasm"));
    if wasm_src.exists() {
        std::fs::create_dir_all(out_dir).ok();
        let wasm_dst = out_dir.join(format!("{name}.wasm"));
        std::fs::copy(&wasm_src, &wasm_dst)
            .unwrap_or_else(|e| panic!("failed to copy wasmtime app to output: {e}"));
        println!("cargo:warning=wasmtime app copied to {}", wasm_dst.display());
    }
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
    std::fs::create_dir_all(out_dir).ok();
    let _ = gen.execute(
        false,
        false,
        &[
            ("foundation-wasm.js", wasm_js.as_path()),
            ("foundation-wasm-ui.js", wasm_ui_js.as_path()),
            ("platform-scheme-interceptor.js", interceptor.as_path()),
        ],
    );
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
             use std::path::PathBuf;\n\n\
             pub const APP_ID: &str = \"{name}\";\n\n\
             #[derive(MobileDirectory)]\n\
             #[source = \"$CARGO_MANIFEST_DIR/public/{public_subdir}\"]\n\
             pub struct AppAssets {{\n    pub root: PathBuf\n}}\n\n\
             impl AppAssets {{\n\
             \x20   pub fn build(session: &PlatformSession) -> MobileApp<AppAssets> {{\n\
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
fn generate_app_manifests(public_dir: &Path, keypair: &crate::manifest::KeyPair) {
    println!("cargo:rerun-if-env-changed={}", crate::manifest::MANIFEST_DOMAIN_ENV);

    assert!(
        keypair.can_sign(),
        "F40: no OTA private key is available, so app manifests cannot be signed.\n\
         Set {} to a base64 Ed25519 seed, or let the build mint one by making \
         keys/ writable.",
        crate::manifest::PRIVATE_KEY_ENV,
    );

    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());
    let domain = crate::manifest::manifest_domain_from_env();

    match crate::manifest::generate_all_manifests(public_dir, &version, &domain, keypair) {
        Ok(manifests) => println!(
            "cargo:warning=generated {} signed app manifests for v{version}",
            manifests.len()
        ),
        Err(e) => panic!("F40: manifest generation failed: {e}"),
    }
}

/// Generate per-app Rust modules for wasmtime shell apps (F33).
///
/// Produces `src/generated/shell/{name}.rs` with a `fn builder() -> WasmtimeBuilder`
/// that uses `include_bytes!` to embed the compiled `.wasm` binary.
fn generate_wasmtime_modules(apps: &[AppDistribution], generated_dir: &Path) {
    let shell_dir = generated_dir.join("shell");
    std::fs::create_dir_all(&shell_dir).ok();

    let mut mod_lines = String::from("// Auto-generated (F33 — wasmtime shell)\n\n");

    for app in apps {
        let module_name = app.name.replace('-', "_");
        // Use the crate name directly for the wasm file lookup.
        let wasm_name = app.crate_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&app.name);
        let content = format!(
            "// Generated — wasmtime shell for \"{name}\" (F33, Surface 3)\n\
             // Route prefix: {route_prefix}\n\
             //\n\
             // The compiled WASM binary lives in shell_wasm/{wasm_name}.wasm.\n\
             // Loads via include_bytes! and wraps in a WasmtimeBuilder.\n\n\
             use foundation_wasmtime::WasmtimeBuilder;\n\n\
             #[must_use]\n\
             pub fn builder() -> WasmtimeBuilder {{\n\
             \x20   let wasm_bytes: &[u8] = include_bytes!(concat!(\n\
             \x20       env!(\"CARGO_MANIFEST_DIR\"),\n\
             \x20       \"/shell_wasm/{wasm_name}.wasm\"\n\
             \x20   ));\n\
             \x20   WasmtimeBuilder::new(wasm_bytes).with_name(\"{name}\")\n\
             }}\n",
            name = app.name,
            route_prefix = app.route_prefix,
            wasm_name = wasm_name,
        );
        std::fs::write(shell_dir.join(format!("{module_name}.rs")), &content).ok();
        let _ = writeln!(mod_lines, "pub mod {module_name};");
    }
    std::fs::write(shell_dir.join("mod.rs"), &mod_lines).ok();

    // Declare `shell` in the parent `generated/mod.rs`. `generate_app_modules`
    // writes that file first and only knows about the WebView apps, so without
    // this append the whole `shell/` tree is unreachable — the F33 wasmtime
    // modules would be generated but never compiled in.
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
    if let Ok(content) = std::fs::read_to_string(&conf_path) {
        // Parse the JSON, add url to the first window, write back
        if let Ok(mut val) = serde_json::from_str::<serde_json::Value>(&content) {
            let target_url = format!("ewe://localhost{route_prefix}");
            if let Some(windows) = val
                .get_mut("app")
                .and_then(|a| a.get_mut("windows"))
                .and_then(|w| w.as_array_mut())
                .and_then(|arr| arr.first_mut())
            {
                windows["url"] = serde_json::Value::String(target_url);
                if let Ok(patched) = serde_json::to_string_pretty(&val) {
                    if patched != content {
                        std::fs::write(&conf_path, &patched).ok();
                        println!("cargo:warning=patched tauri.conf.json url");
                    }
                }
            }
        }
    }
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
