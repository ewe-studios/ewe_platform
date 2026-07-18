use std::path::{Path, PathBuf};

pub struct AppDistribution {
    pub name: String,
    pub crate_dir: PathBuf,
    pub route_prefix: String,
}

pub fn generate_platform_code() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let project_root = manifest_dir.parent().unwrap().to_path_buf();

    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=build.rs");

    let mut apps = Vec::new();
    let app_dir = project_root.join("app");
    if app_dir.join("Cargo.toml").exists() {
        apps.push(AppDistribution { name: "app".into(), crate_dir: app_dir, route_prefix: "/app/".into() });
    }
    if let Ok(entries) = std::fs::read_dir(&project_root) {
        for e in entries.filter_map(|e| e.ok()) {
            let p = e.path();
            let n = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            if n.starts_with("app-") && p.is_dir() && p.join("Cargo.toml").exists() {
                apps.push(AppDistribution { name: n.clone(), crate_dir: p, route_prefix: format!("/{}/", n) });
            }
        }
    }

    if !apps.is_empty() {
        let public_dir = manifest_dir.join("public");
        build_all_wasm_apps(&apps, &public_dir);
        let generated_dir = manifest_dir.join("src").join("generated");
        std::fs::create_dir_all(&generated_dir).ok();
        generate_app_modules(&apps, &generated_dir);
    }

    tauri_build::build();

    if let Some(first) = apps.first() {
        patch_tauri_conf_for_ewe(&manifest_dir, &first.route_prefix);
    } else {
        patch_tauri_conf_for_ewe(&manifest_dir, "/__platform__/");
    }
}

pub fn build_all_wasm_apps(apps: &[AppDistribution], public_dir: &Path) {
    for app in apps {
        build_wasm_app(&app.crate_dir, &public_dir.join(&app.name));
    }
}

fn build_wasm_app(app_dir: &Path, out_dir: &Path) {
    use foundation_wasm_ui::build_tools::WasmBundleGenerator;

    let wasm: Vec<_> = scan_for_annotations(&app_dir.join("src"))
        .into_iter()
        .filter(|a| matches!(a.kind, AnnotationKind::WasmBin | AnnotationKind::WasmWorker | AnnotationKind::WasmService))
        .collect();
    if wasm.is_empty() { return; }

    let pairs: Vec<(&str, &str)> = wasm.iter().map(|a| (mode_str(a.kind), a.name.as_str())).collect();
    let gen = match WasmBundleGenerator::from_annotations(app_dir, out_dir, &pairs, true) {
        Ok(g) => g,
        Err(e) => { println!("cargo:warning=WasmBundleGenerator: {e}"); return; }
    };

    let repo_root = app_dir.parent().unwrap().parent().unwrap().parent().unwrap();
    let wasm_js = repo_root.join("backends/foundation_wasm/runtime/foundation-wasm.js");
    let wasm_ui_js = repo_root.join("backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js");
    let interceptor = repo_root.join("backends/foundation_wasm_ui/runtimes/platform-scheme-interceptor.js");
    std::fs::create_dir_all(out_dir).ok();
    let _ = gen.execute(false, false, &[
        ("foundation-wasm.js", wasm_js.as_path()),
        ("foundation-wasm-ui.js", wasm_ui_js.as_path()),
        ("platform-scheme-interceptor.js", interceptor.as_path()),
    ]);
}

fn generate_app_modules(apps: &[AppDistribution], generated_dir: &Path) {
    let mut mod_lines = String::from("// Auto-generated\n\n");
    for app in apps {
        let module_name = app.name.replace('-', "_");
        let content = format!(
            "// Generated — WebviewApp for \"{name}\"\n// Route prefix: {route_prefix}\n\
             // Usage: builder.route_with(\"{route_prefix}\", webview_app(), generated::{module_name}::AppAssets::build());\n\n\
             use foundation_macros::EmbedDirectoryAs;\n\
             use foundation_platform::WebviewApp;\n\n\
             #[derive(EmbedDirectoryAs)]\n#[source = \"public/{public_subdir}\"]\n\
             pub struct AppAssets;\n\n\
             impl AppAssets {{\n    pub fn build() -> WebviewApp<AppAssets> {{ WebviewApp::new(AppAssets {{}}) }}\n}}\n",
            module_name = module_name, name = app.name, route_prefix = app.route_prefix, public_subdir = &app.name,
        );
        std::fs::write(generated_dir.join(format!("{module_name}.rs")), &content).ok();
        mod_lines.push_str(&format!("pub mod {module_name};\n"));
    }
    std::fs::write(generated_dir.join("mod.rs"), &mod_lines).ok();
    println!("cargo:warning=generated {} app modules", apps.len());
}

fn patch_tauri_conf_for_ewe(manifest_dir: &Path, route_prefix: &str) {
    let target_url = format!("http://ewe.localhost{}", route_prefix.trim_end_matches('/'));
    if let Ok(c) = std::fs::read_to_string(&manifest_dir.join("tauri.conf.json")) {
        let patched = c.replace(r#""url": "index.html""#, &format!(r#""url": "{target_url}""#));
        if patched != c { std::fs::write(&manifest_dir.join("tauri.conf.json"), &patched).ok(); }
    }
}

#[derive(Debug, Clone)] pub struct Annotation { pub name: String, pub kind: AnnotationKind, pub file: PathBuf, pub target: String }
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum AnnotationKind { WasmBin, WasmWorker, WasmService, PlatformBin }
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
