//! Root build script — discovers WASM entrypoints in `app/src/`,
//! compiles `app/` to wasm32, copies artifacts + runtimes to
//! `src-tauri/public/`, generates JS wrappers + index.html.
//!
//! Each `#[wasm_bin]` / `#[wasm_worker]` / `#[wasm_service]`
//! function in `app/src/` produces a separate `.wasm` + `.js` pair.
//! All share one `cargo build` invocation (one crate, one target).

use std::path::{Path, PathBuf};
use std::process::Command;

// ── Annotation types (mirrors foundation_platform::codegen) ─────────

#[derive(Debug, Clone)]
struct WasmEntrypoint {
    name: String,
    kind: WasmKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WasmKind { Bin, Worker, Service }

fn main() {
    let root = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"),
    );
    let app_dir = root.join("app");
    let public_dir = root.join("src-tauri").join("public");

    println!("cargo:rerun-if-changed=app/src/");
    println!("cargo:rerun-if-changed=app/Cargo.toml");

    // 1. Scan for wasm entrypoints
    let entrypoints = scan_wasm_entrypoints(&app_dir.join("src"));
    if entrypoints.is_empty() {
        println!("cargo:warning=no wasm entrypoints found in app/src/ — skipping");
        return;
    }

    println!("cargo:warning=found {} wasm entrypoints: {:?}",
        entrypoints.len(),
        entrypoints.iter().map(|e| &*e.name).collect::<Vec<_>>()
    );

    // 2. Compile app/ to wasm32
    // Clear host RUSTFLAGS — they may contain linker flags (e.g. -fuse-ld=lld)
    // that the wasm32 backend doesn't understand.
    let target = resolve_wasm_target(&entrypoints);
    let status = Command::new("cargo")
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .args(["build", "--manifest-path"])
        .arg(app_dir.join("Cargo.toml"))
        .args(["--target", &target, "--profile", "uat"])
        .status()
        .expect("cargo build (wasm) failed to start");

    if !status.success() {
        panic!("WASM build failed for target {target}");
    }

    // 3. Copy .wasm files, JS runtimes, generate wrappers + index.html
    std::fs::create_dir_all(&public_dir).ok();
    copy_wasm_binaries(&app_dir, &public_dir, &target, &entrypoints);
    copy_js_runtimes(&public_dir);
    generate_js_wrappers(&public_dir, &entrypoints);
    generate_index_html(&public_dir, &entrypoints);
}

// ── Annotation scanner ────────────────────────────────────────────

fn scan_wasm_entrypoints(src_dir: &Path) -> Vec<WasmEntrypoint> {
    let mut eps = Vec::new();
    let Ok(entries) = std::fs::read_dir(src_dir) else { return eps; };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().map_or(false, |n| n != "target" && !n.to_string_lossy().starts_with('.')) {
                eps.extend(scan_wasm_entrypoints(&path));
            }
        } else if path.extension().map_or(false, |e| e == "rs") {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let lines: Vec<&str> = content.lines().collect();
                let mut i = 0;
                while i < lines.len() {
                    let trimmed = lines[i].trim();
                    let kind = if trimmed.starts_with("#[wasm_bin") {
                        Some(WasmKind::Bin)
                    } else if trimmed.starts_with("#[wasm_worker") {
                        Some(WasmKind::Worker)
                    } else if trimmed.starts_with("#[wasm_service") {
                        Some(WasmKind::Service)
                    } else {
                        None
                    };

                    if let Some(kind) = kind {
                        // Look ahead for the function name — annotation
                        // is typically on the line before `fn name()`.
                        let name = (i..lines.len())
                            .find_map(|j| extract_fn_name(lines[j]));
                        if let Some(name) = name {
                            eps.push(WasmEntrypoint { name, kind });
                        }
                    }
                    i += 1;
                }
            }
        }
    }
    eps
}

fn extract_fn_name(line: &str) -> Option<String> {
    // Annotation and function are typically on separate lines:
    //   #[wasm_bin(extern = "true")]
    //   fn my_app() { ... }
    // If `fn` isn't on this line, return a placeholder — the WASM export
    // name comes from `#[no_mangle]` anyway. The build script only needs a
    // handle to generate the JS wrapper; the actual export name is in .wasm.
    line.find("fn ").map(|fn_pos| {
        let after = &line[fn_pos + 3..];
        after.split(|c: char| !c.is_alphanumeric() && c != '_')
            .find(|s| !s.is_empty())
            .unwrap_or("unknown")
            .to_string()
    })
}

// ── Target resolution ─────────────────────────────────────────────

fn resolve_wasm_target(_eps: &[WasmEntrypoint]) -> String {
    // All entrypoints share one crate → one target.
    // Default: wasm32-unknown-unknown (browser/WebView).
    // Future: scan annotation attrs for `target = "wasip1"`.
    "wasm32-unknown-unknown".to_string()
}

// ── Artifact copying ─────────────────────────────────────────────

fn copy_wasm_binaries(app_dir: &Path, public: &Path, target: &str, _eps: &[WasmEntrypoint]) {
    // One crate = one .wasm file. All entrypoint exports live in it.
    // JS wrappers reference it by the crate name.
    let crate_name = "platform_android_wasm"; // matches app/Cargo.toml package name
    let wasm_path = app_dir
        .join("target").join(target).join("uat")
        .join(format!("{crate_name}.wasm"));

    std::fs::copy(&wasm_path, public.join(format!("{crate_name}.wasm")))
        .unwrap_or_else(|e| panic!("copy .wasm: {e}"));
}

fn copy_js_runtimes(public: &Path) {
    // Relative from platform_android/examples/platform_android/
    // → ../../examples/.. is wrong. We go up 4 levels from the root:
    // platform_android → examples → repo_root
    let repo_root = public.parent().unwrap() // src-tauri
        .parent().unwrap() // platform_android
        .parent().unwrap() // examples
        .parent().unwrap(); // repo root

    for (src_rel, name) in &[
        ("backends/foundation_wasm/runtime/foundation-wasm.js", "foundation-wasm.js"),
        ("backends/foundation_wasm_ui/runtimes/foundation-wasm-ui.js", "foundation-wasm-ui.js"),
        ("backends/foundation_wasm_ui/runtimes/platform-scheme-interceptor.js", "platform-scheme-interceptor.js"),
    ] {
        std::fs::copy(repo_root.join(src_rel), public.join(name))
            .unwrap_or_else(|e| panic!("copy {}: {e}", name));
    }
}

// ── JS wrapper generation ─────────────────────────────────────────

fn generate_js_wrappers(public: &Path, eps: &[WasmEntrypoint]) {
    for ep in eps {
        let js = match ep.kind {
            WasmKind::Bin => bin_wrapper(&ep.name),
            WasmKind::Worker => worker_wrapper(&ep.name),
            WasmKind::Service => service_wrapper(&ep.name),
        };
        std::fs::write(public.join(format!("{}.js", ep.name)), js)
            .unwrap_or_else(|e| panic!("write {}.js: {e}", ep.name));
    }
}

const WASM_CRATE: &str = "platform_android_wasm";

fn bin_wrapper(name: &str) -> String {
    format!(
        r#"// Generated — wasm_bin "{name}".
import './foundation-wasm-ui.js';
import './platform-scheme-interceptor.js';
import {{ FoundationWasm }} from './foundation-wasm.js';

const WASM_URL = './{WASM_CRATE}.wasm';

export async function init(importOverrides = {{}}) {{
  const wasmBytes = await (await fetch(WASM_URL)).arrayBuffer();
  const rt = new FoundationWasm();
  const imports = {{ abi: {{ ...rt.web_abi, ...importOverrides }} }};
  const {{ instance }} = await WebAssembly.instantiate(wasmBytes, imports);
  rt.init(instance);
  instance.exports.{name}();
  return {{ runtime: rt, instance }};
}}
"#
    )
}

fn worker_wrapper(name: &str) -> String {
    format!(
        r#"// Generated — wasm_worker "{name}".
import './foundation-wasm-ui.js';
import './platform-scheme-interceptor.js';
import {{ FoundationWasm }} from './foundation-wasm.js';

const WASM_URL = './{WASM_CRATE}.wasm';

(async () => {{
  const wasmBytes = await (await fetch(WASM_URL)).arrayBuffer();
  const rt = new FoundationWasm();
  const {{ instance }} = await WebAssembly.instantiate(wasmBytes, {{ abi: rt.web_abi }});
  rt.init(instance);
  instance.exports.{name}();
  self.postMessage({{ ready: true }});
}})();
"#
    )
}

fn service_wrapper(name: &str) -> String {
    format!(
        r#"// Generated — wasm_service "{name}".
importScripts('./foundation-wasm.js');

const WASM_URL = './{WASM_CRATE}.wasm';
let runtimePromise = null;
async function ensureRuntime() {{
  if (!runtimePromise) {{
    runtimePromise = (async () => {{
      const wasmBytes = await (await fetch(WASM_URL)).arrayBuffer();
      const rt = new globalThis.FoundationWasmRuntime.FoundationWasm();
      const {{ instance }} = await WebAssembly.instantiate(wasmBytes, {{ abi: rt.web_abi }});
      rt.init(instance);
      instance.exports.{name}();
      return rt;
    }})();
  }}
  return runtimePromise;
}}

self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', (event) => event.waitUntil(self.clients.claim()));
"#
    )
}

// ── index.html generation ─────────────────────────────────────────

fn generate_index_html(public: &Path, eps: &[WasmEntrypoint]) {
    let bins: Vec<_> = eps.iter().filter(|e| e.kind == WasmKind::Bin).collect();

    let mut init_blocks = String::new();
    for ep in &bins {
        init_blocks.push_str(&format!(
            r#"<script type=module>
import {{ init }} from './{name}.js';
init()
  .then(() => document.getElementById('status').textContent = 'WASM active')
  .catch(e => document.getElementById('status').textContent = 'Error: ' + e.message);
</script>
"#,
            name = ep.name
        ));
    }

    let html = format!(
        r#"<!DOCTYPE html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Foundation Platform</title>
<style>
*{{margin:0;padding:0;box-sizing:border-box}}
body{{font-family:system-ui,sans-serif;padding:16px;background:#0a0a1a;color:#ccd6f6}}
h1{{color:#64ffda;font-size:22px}}
p{{color:#8892b0;font-size:12px}}
#status{{margin:12px 0;font-size:11px;color:#445566}}
</style></head><body>
<h1>Foundation Platform</h1>
<p>Android — WASM UI · columnar v1</p>
<div id="status">Loading WASM runtime...</div>
{init_blocks}
</body></html>"#
    );

    std::fs::write(public.join("index.html"), html).expect("write index.html");
}
