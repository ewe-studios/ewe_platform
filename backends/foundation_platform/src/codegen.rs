use std::path::{Path, PathBuf};

/// Called from `src-tauri/build.rs`. Runs Tauri codegen + wasm build.
pub fn generate_platform_code() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"),
    );
    let app_dir = manifest_dir.parent().unwrap().join("app");
    let public_dir = manifest_dir.join("public");

    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=build.rs");

    if app_dir.join("Cargo.toml").exists() {
        build_wasm_app(&app_dir, &public_dir);
    }

    tauri_build::build();

    let generated_dir = manifest_dir.join("src").join("generated");
    std::fs::create_dir_all(&generated_dir).ok();
    if !generated_dir.join("mod.rs").exists() {
        std::fs::write(generated_dir.join("mod.rs"), "// Auto-generated\n").ok();
    }

    copy_public_to_android_assets(&public_dir, &manifest_dir);
}

/// Compile the wasm crate and copy artifacts to public/.
fn build_wasm_app(app_dir: &Path, public_dir: &Path) {
    use foundation_wasm_ui::build_tools::WasmBundleGenerator;

    let wasm: Vec<_> = scan_for_annotations(&app_dir.join("src"))
        .into_iter()
        .filter(|a| matches!(a.kind, AnnotationKind::WasmBin | AnnotationKind::WasmWorker | AnnotationKind::WasmService))
        .collect();

    if wasm.is_empty() {
        println!("cargo:warning=no wasm annotations in app/src/");
        return;
    }

    let pairs: Vec<(&str, &str)> = wasm.iter().map(|a| (mode_str(a.kind), a.name.as_str())).collect();
    let gen = match WasmBundleGenerator::from_annotations(app_dir, public_dir, &pairs, /* jsruntime_single */ true) {
        Ok(g) => g,
        Err(e) => { println!("cargo:warning=WasmBundleGenerator: {e}"); return; }
    };

    let repo_root = app_dir.parent().unwrap().parent().unwrap().parent().unwrap();
    let wasm_js = repo_root.join("backends/foundation_wasm/runtime/foundation-wasm.js");
    let wasm_ui_js = repo_root.join("backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js");
    let interceptor = repo_root.join("backends/foundation_wasm_ui/runtimes/platform-scheme-interceptor.js");
    std::fs::create_dir_all(public_dir).ok();

    // Pass ALL runtimes as assets so WasmBundleGenerator bundles them
    // into bundle.js when jsruntime_single=true.
    let runtime_assets: &[(&str, &Path)] = &[
        ("foundation-wasm.js", wasm_js.as_path()),
        ("foundation-wasm-ui.js", wasm_ui_js.as_path()),
        ("platform-scheme-interceptor.js", interceptor.as_path()),
    ];
    let _ = gen.execute(false, false, runtime_assets);

    // Generate minimal index.html — non-module init via WasmLoader (in bundle.js).
    // Android WebView: document.baseURI=about:blank breaks ES module imports.
    // bundle.js IIFE-wraps all runtimes, WasmLoader/FoundationWasm are on globalThis.
    let bins: Vec<&str> = wasm.iter().filter(|a| a.kind == AnnotationKind::WasmBin).map(|a| a.name.as_str()).collect();
    if !bins.is_empty() {
        let mut inits = String::new();
        for name in &bins {
            inits.push_str(&format!(
                "var l=new FoundationWasmRuntime.WasmLoader();l.loadURL('./{name}.wasm').then(function(r){{r.instance.exports.{name}();document.getElementById('s').textContent='OK'}}).catch(function(e){{document.getElementById('s').textContent=String(e)}});"
            ));
        }
        std::fs::write(public_dir.join("index.html"), format!(
            "<!DOCTYPE html><html><head><meta charset=utf-8><meta name=viewport content='width=device-width,initial-scale=1'><title>FP</title>\
             <style>body{{font-family:sans-serif;padding:16px;background:#0a0a1a;color:#ccd6f6}}h1{{color:#64ffda;font-size:20px}}#s{{margin:8px 0;font-size:12px;color:#8892b0}}</style></head>\
             <body><h1>Foundation Platform</h1><div id=s>Loading...</div>\
             <script src=\"bundle.js\"></script>\
             <script>{inits}</script></body></html>"
        )).ok();
    }
}

fn copy_public_to_android_assets(public_dir: &Path, manifest_dir: &Path) {
    let assets = manifest_dir.join("gen/android/app/src/main/assets");
    if !assets.parent().map_or(false, |p| p.exists()) { return; }
    std::fs::create_dir_all(&assets).ok();
    if let Ok(entries) = std::fs::read_dir(public_dir) {
        for e in entries.filter_map(|e| e.ok()) {
            let p = e.path();
            let _ = std::fs::copy(&p, assets.join(p.file_name().unwrap()));
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
