use std::path::{Path, PathBuf};

// ── AppDistribution ────────────────────────────────────────────────────

/// Describes one WASM app crate to build and deploy.
///
/// Each app gets its own `public/{name}/` subdirectory. The route prefix
/// determines the URL path (e.g. `"/app/dashboard/"` → `http://ewe.localhost/app/dashboard/`).
pub struct AppDistribution {
    /// Subdirectory name for assets (e.g. `"app-hello"` → `public/app-hello/`).
    pub name: String,
    /// Path to the wasm crate root (e.g. `root.join("app")`).
    pub crate_dir: PathBuf,
    /// Route prefix this app serves at (e.g. `"/app/"` or `"/app/dashboard/"`).
    pub route_prefix: String,
}

// ── Entry point: src-tauri/build.rs ───────────────────────────────────

/// Called from `src-tauri/build.rs`. Scans for wasm app crates, builds
/// each, and runs Tauri codegen.
pub fn generate_platform_code() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"),
    );
    let project_root = manifest_dir.parent().unwrap().to_path_buf();

    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=build.rs");

    // Discover WASM app crates: looks for app/ and app-*/ directories
    let mut apps = Vec::new();
    let app_dir = project_root.join("app");
    if app_dir.join("Cargo.toml").exists() {
        apps.push(AppDistribution {
            name: "app".into(),
            crate_dir: app_dir,
            route_prefix: "/app/".into(),
        });
    }
    // Scan for app-*/ crates
    if let Ok(entries) = std::fs::read_dir(&project_root) {
        for e in entries.filter_map(|e| e.ok()) {
            let p = e.path();
            let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if fname.starts_with("app-") && p.is_dir() && p.join("Cargo.toml").exists() {
                let name = fname.to_string();
                let prefix = format!("/{}/", fname);
                apps.push(AppDistribution { name, crate_dir: p, route_prefix: prefix });
            }
        }
    }

    if !apps.is_empty() {
        let public_dir = manifest_dir.join("public");
        build_all_wasm_apps(&apps, &public_dir);

        // Generate src/generated/ per app
        let generated_dir = manifest_dir.join("src").join("generated");
        std::fs::create_dir_all(&generated_dir).ok();
        generate_app_modules(&apps, &generated_dir);
    }

    tauri_build::build();

    // Patch initial URL to bypass WebViewAssetLoader
    if let Some(first) = apps.first() {
        patch_tauri_conf_for_ewe(&manifest_dir, &first.route_prefix);
    } else {
        patch_tauri_conf_for_ewe(&manifest_dir, "/__platform__/");
    }
}

// ── Public API: root build.rs ──────────────────────────────────────────

/// Builds multiple wasm app crates, each into `public/{name}/`.
pub fn build_all_wasm_apps(apps: &[AppDistribution], public_dir: &Path) {
    for app in apps {
        let app_public = public_dir.join(&app.name);
        build_wasm_app(&app.crate_dir, &app_public, &app.name);
    }
}

fn build_wasm_app(app_dir: &Path, out_dir: &Path, _name: &str) {
    use foundation_wasm_ui::build_tools::WasmBundleGenerator;

    let wasm: Vec<_> = scan_for_annotations(&app_dir.join("src"))
        .into_iter()
        .filter(|a| matches!(a.kind, AnnotationKind::WasmBin | AnnotationKind::WasmWorker | AnnotationKind::WasmService))
        .collect();

    if wasm.is_empty() {
        println!("cargo:warning=no wasm annotations in {}", app_dir.display());
        return;
    }

    let pairs: Vec<(&str, &str)> = wasm.iter().map(|a| (mode_str(a.kind), a.name.as_str())).collect();
    let gen = match WasmBundleGenerator::from_annotations(app_dir, out_dir, &pairs, true) {
        Ok(g) => g,
        Err(e) => { println!("cargo:warning=WasmBundleGenerator: {e}"); return; }
    };

    let repo_root = app_dir.parent().unwrap().parent().unwrap().parent().unwrap();

    // Find runtimes relative to repo root
    let runtime_dir = |relative: &str| -> PathBuf {
        // Try the standard backends/ path first
        let p = repo_root.join("backends").join(relative);
        if p.exists() { return p; }
        // Fallback: search
        repo_root.join(relative)
    };

    let wasm_js = runtime_dir("foundation_wasm/runtime/foundation-wasm.js");
    let wasm_ui_js = runtime_dir("foundation_wasm_ui/runtimes/foundation-wasm-ui.js");
    let interceptor = runtime_dir("foundation_wasm_ui/runtimes/platform-scheme-interceptor.js");

    std::fs::create_dir_all(out_dir).ok();

    let runtime_assets: &[(&str, &Path)] = &[
        ("foundation-wasm.js", wasm_js.as_path()),
        ("foundation-wasm-ui.js", wasm_ui_js.as_path()),
        ("platform-scheme-interceptor.js", interceptor.as_path()),
    ];
    // WasmBundleGenerator handles everything: .wasm, JS wrappers,
    // bundle.js, runtime assets, and index.html generation.
    let _ = gen.execute(false, false, runtime_assets);
}

// ── src/generated/ modules ─────────────────────────────────────────────

fn generate_app_modules(apps: &[AppDistribution], generated_dir: &Path) {
    let mut mod_lines = String::from("// Auto-generated by foundation_platform::codegen\n\n");

    for app in apps {
        let module_name = app.name.replace('-', "_");
        let route_prefix = &app.route_prefix;
        let public_subdir = &app.name;

        let content = format!(
            r#"// Generated — WebviewApp responder for "{name}".
// Route: {route_prefix}  Assets: public/{public_subdir}/
//
// Wire in src-tauri/src/lib.rs:
//   builder.route_with("{route_prefix}", webview_app().with_profile(Profile::App),
//       generated::{module_name}::AppAssets::build());

use foundation_macros::EmbedDirectoryAs;
use foundation_platform::responder::WebviewApp;

/// Embedded assets from public/{public_subdir}/.
#[derive(EmbedDirectoryAs)]
#[source = "public/{public_subdir}"]
pub struct AppAssets;

impl AppAssets {{
    pub fn build() -> WebviewApp {{
        WebviewApp::new(Self {{}})
    }}
}}
"#,
            module_name = app.name.replace('-', "_"),
            name = app.name,
            route_prefix = route_prefix,
            public_subdir = public_subdir,
        );

        std::fs::write(generated_dir.join(format!("{}.rs", module_name)), &content).ok();
        mod_lines.push_str(&format!("pub mod {module_name};\n"));
    }

    std::fs::write(generated_dir.join("mod.rs"), &mod_lines).ok();
    println!("cargo:warning=generated {}/mod.rs + {} app modules", generated_dir.display(), apps.len());
}

fn patch_tauri_conf_for_ewe(manifest_dir: &Path, route_prefix: &str) {
    let conf_path = manifest_dir.join("tauri.conf.json");
    let target_url = format!("http://ewe.localhost{}", route_prefix.trim_end_matches('/'));
    if let Ok(content) = std::fs::read_to_string(&conf_path) {
        let patched = content.replace(
            r#""url": "index.html""#,
            &format!(r#""url": "{target_url}""#),
        );
        let patched = patched.replace(
            r#""url":"index.html""#,
            &format!(r#""url":"{target_url}""#),
        );
        if patched != content {
            std::fs::write(&conf_path, &patched).ok();
            println!("cargo:warning=patched tauri.conf.json url → {target_url}");
        }
    }
}

// ── Scanner ────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Annotation { pub name: String, pub kind: AnnotationKind, pub file: PathBuf, pub target: String }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationKind { WasmBin, WasmWorker, WasmService, PlatformBin }

fn mode_str(k: AnnotationKind) -> &'static str {
    match k { AnnotationKind::WasmBin => "wasm_bin", AnnotationKind::WasmWorker => "wasm_worker", AnnotationKind::WasmService => "wasm_service", AnnotationKind::PlatformBin => "platform_bin" }
}

pub fn scan_for_annotations(dir: &Path) -> Vec<Annotation> {
    let mut v = Vec::new();
    let Ok(es) = std::fs::read_dir(dir) else { return v };
    for e in es.filter_map(|e| e.ok()) {
        let p = e.path();
        if p.is_dir() && p.file_name().map_or(false, |n| n != "target" && !n.to_string_lossy().starts_with('.')) { v.extend(scan_for_annotations(&p)); }
        else if p.extension().map_or(false, |x| x == "rs") {
            if let Ok(c) = std::fs::read_to_string(&p) {
                let ls: Vec<&str> = c.lines().collect();
                for i in 0..ls.len() {
                    let t = ls[i].trim();
                    let kind = if t.starts_with("#[wasm_bin") { Some(AnnotationKind::WasmBin) }
                    else if t.starts_with("#[wasm_worker") { Some(AnnotationKind::WasmWorker) }
                    else if t.starts_with("#[wasm_service") { Some(AnnotationKind::WasmService) }
                    else if t.starts_with("#[platform_bin") { Some(AnnotationKind::PlatformBin) }
                    else { None };
                    if kind.is_some() {
                        let name = (i..ls.len()).find_map(|j| ls[j].find("fn ").map(|p2| {
                            ls[j][p2+3..].split(|c: char| !c.is_alphanumeric() && c != '_').find(|s| !s.is_empty()).unwrap_or("unknown").to_string()
                        }));
                        if let Some(n) = name { v.push(Annotation { name: n, kind: kind.unwrap(), file: p.clone(), target: String::from("unknown") }); }
                    }
                }
            }
        }
    }
    v
}
