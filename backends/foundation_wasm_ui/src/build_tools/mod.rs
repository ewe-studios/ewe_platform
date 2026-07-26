//! WHY: Annotating a fn with `#[wasm_bin]`/`#[wasm_worker]`/`#[wasm_service]`
//! should be ALL a user does — discovery, WASM compilation, per-mode JS
//! wrappers, optional single-file bundling, and runtime-asset copying are the
//! pipeline's job (feature 10, decisions 014/016/017).
//!
//! WHAT: [`WasmBundleGenerator`] — wraps the existing [`WasmBinGenerator`]
//! (entrypoint discovery + `bin/` generation) and adds the JS layer; plus the
//! mode/packaging vocabulary the scanner extracts from the attributes.
//!
//! HOW: Scans for all four attribute names (`wasm_entrypoint` + the three
//! mode markers — `CrateScanner` reads SOURCE, so the convenience attributes
//! are visible directly), maps each to a [`BundleEntrypoint`], and emits
//! wrappers from the templates in [`js_wrapper`] (pure string fns — unit
//! testable) and bundles via [`bundler`].

// Hosted-prelude import: this module is target-gated (native-only) inside a no_std crate.
#[allow(unused_imports)]
use std::prelude::rust_2021::*;

pub mod bundler;
pub mod js_wrapper;

use std::path::{Path, PathBuf};
use std::process::Command;

use std::collections::HashMap;

use foundation_codegen::{AttributeValue, CrateScanner, ItemKind, RegistryExt};

use foundation_wasm::build_tools::error::WasmBinError;
use foundation_wasm::build_tools::WasmBinGenerator;

fn io_err(path: &Path, source: std::io::Error) -> WasmBinError {
    WasmBinError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Execution mode (decision 014).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleMode {
    /// Main thread (`#[wasm_bin]`).
    Bin,
    /// Web worker (`#[wasm_worker]`).
    Worker,
    /// Service worker with a route table (`#[wasm_service]`).
    Service,
}

/// How the WASM bytes reach the page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JsPackaging {
    /// `{name}.js` fetches `{name}.wasm` (default).
    Separate,
    /// WASM bytes embedded into the JS file.
    SingleFile(Encoding),
}

/// Single-file embedding form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Encoding {
    /// `Uint8Array` literal (default — no decode step).
    #[default]
    Uint8Array,
    /// base64 string + `atob` decoder (smaller source, decode cost).
    B64,
}

/// One discovered mode-annotated entrypoint.
#[derive(Clone, Debug)]
pub struct BundleEntrypoint {
    pub name: String,
    pub mode: BundleMode,
    pub packaging: JsPackaging,
    /// `wasm_service` route prefixes.
    pub routes: Vec<String>,
    /// Combine all JS runtimes into one bundle.js (default true).
    pub jsruntime_single: bool,
}

/// What `plan()`/`execute()` will produce for one entrypoint.
#[derive(Clone, Debug)]
pub struct PlannedFile {
    pub path: PathBuf,
    pub kind: &'static str,
}

pub struct WasmBundleGenerator {
    inner: WasmBinGenerator,
    entrypoints: Vec<BundleEntrypoint>,
    crate_dir: PathBuf,
    output_dir: PathBuf,
}

impl WasmBundleGenerator {
    /// Scan `crate_dir` for mode-annotated entrypoints.
    ///
    /// # Errors
    /// Propagates crate validation/scan failures from the inner generator and
    /// malformed mode attributes.
    pub fn new(crate_dir: &Path, output_dir: &Path) -> Result<Self, WasmBinError> {
        let inner = WasmBinGenerator::new(crate_dir)?;
        let mut entrypoints = Vec::new();
        for (attr, mode) in [
            ("wasm_bin", BundleMode::Bin),
            ("wasm_worker", BundleMode::Worker),
            ("wasm_service", BundleMode::Service),
        ] {
            let scanner = CrateScanner::new(attr);
            let registry = scanner.scan_crate(crate_dir)?;
            for target in registry.filter_by_kind(&ItemKind::Function) {
                entrypoints.push(entrypoint_from_attrs(
                    &target.item_name,
                    mode,
                    &target.attributes,
                ));
            }
        }
        Ok(Self {
            inner,
            entrypoints,
            crate_dir: crate_dir.to_path_buf(),
            output_dir: output_dir.to_path_buf(),
        })
    }

    /// Construct from raw annotation + function name pairs.
    ///
    /// This is THE entry point for callers that scan source code directly
    /// (e.g. `foundation_platform::codegen`). Each pair `("wasm_bin",
    /// "my_app")` is converted into:
    ///
    /// 1. A [`foundation_wasm::build_tools::WasmEntrypoint`] — feeds the
    ///    inner `WasmBinGenerator` so it can generate `src/bin/{name}/main.rs`
    ///    stubs (with the correct function import) and Cargo `[[bin]]` entries.
    /// 2. A [`BundleEntrypoint`] — feeds the JS wrapper stage after compilation.
    ///
    /// No `CrateScanner` is used. No expansion into the source file.
    /// The proc macro `#[wasm_bin]` is a validator + passthrough — this
    /// constructor does all the build-time work.
    ///
    /// # Errors
    /// Propagates crate validation failure.
    pub fn from_annotations(
        crate_dir: &Path,
        output_dir: &Path,
        annotations: &[(&str, &str)],  // &[(mode_str, fn_name)]
        jsruntime_single: bool,
    ) -> Result<Self, WasmBinError> {
        let crate_name = WasmBinGenerator::from_crate_only(crate_dir)?
            .crate_name()
            .to_string();

        // Build WasmEntrypoints for the bin generator (planner needs
        // function_name, qualified_path, source_file, line).
        let wasm_eps: Vec<_> = annotations
            .iter()
            .map(|&(_, fn_name)| {
                foundation_wasm::build_tools::wasm_entrypoint_from_fn(&crate_name, fn_name)
            })
            .collect();

        let inner = WasmBinGenerator::from_entrypoints(crate_dir, wasm_eps)?;

        // Build BundleEntrypoints for the JS wrapper stage
        let mut bundle_eps = Vec::with_capacity(annotations.len());
        for &(mode, fn_name) in annotations {
            let (bundle_mode, routes) = match mode {
                "wasm_bin" => (BundleMode::Bin, Vec::new()),
                "wasm_worker" => (BundleMode::Worker, Vec::new()),
                "wasm_service" => (BundleMode::Service, Vec::new()),
                _ => continue,
            };
            bundle_eps.push(BundleEntrypoint {
                name: fn_name.to_string(),
                mode: bundle_mode,
                packaging: JsPackaging::Separate,
                routes,
                jsruntime_single,
            });
        }

        Ok(Self {
            inner,
            entrypoints: bundle_eps,
            crate_dir: crate_dir.to_path_buf(),
            output_dir: output_dir.to_path_buf(),
        })
    }

    /// Construct from pre-built entrypoints — skips `CrateScanner`.
    ///
    /// Use this when the caller has already built [`BundleEntrypoint`]
    /// structs (e.g. from parsed annotation attributes).
    ///
    /// # Errors
    /// Propagates crate validation failure from the inner generator.
    pub fn from_entrypoints(
        crate_dir: &Path,
        output_dir: &Path,
        entrypoints: Vec<BundleEntrypoint>,
        jsruntime_single: bool,
    ) -> Result<Self, WasmBinError> {
        // Apply jsruntime_single to all entrypoints
        let entrypoints: Vec<_> = entrypoints.into_iter().map(|mut ep| { ep.jsruntime_single = jsruntime_single; ep }).collect();
        let inner = WasmBinGenerator::from_crate_only(crate_dir)?;
        Ok(Self {
            inner,
            entrypoints,
            crate_dir: crate_dir.to_path_buf(),
            output_dir: output_dir.to_path_buf(),
        })
    }

    /// Execute for a cdylib (lib) crate — skips `self.inner.generate()`
    /// (no per-entrypoint `src/bin/*.rs` or `[[bin]]` needed).
    /// The wasm binary is assumed to already be compiled to
    /// `target/wasm32-unknown-unknown/{profile}/{crate_name}.wasm`.
    pub fn execute_for_lib_crate(
        &self,
        crate_name: &str,
        release: bool,
        skip_runtimes: bool,
        runtime_assets: &[(&str, &Path)],
    ) -> Result<Vec<PlannedFile>, WasmBinError> {
        std::fs::create_dir_all(&self.output_dir)
            .map_err(|e| io_err(&self.output_dir, e))?;

        let profile_dir = if release { "release" } else { "uat" };
        let wasm_path = self
            .crate_dir
            .join("target/wasm32-unknown-unknown")
            .join(profile_dir)
            .join(format!("{crate_name}.wasm"));
        let wasm_bytes =
            std::fs::read(&wasm_path).map_err(|e| io_err(&wasm_path, e))?;

        let mut written = Vec::new();
        for ep in &self.entrypoints {
            written.extend(self.write_entrypoint(ep, &wasm_bytes)?);
        }

        let do_single_js = self.entrypoints.iter().any(|ep| ep.jsruntime_single);
        if do_single_js {
            let mut bundle = String::new();
            for (file_name, source) in runtime_assets {
                if let Ok(content) = std::fs::read_to_string(source) {
                    let stripped = content.lines().map(|l| {
                        let t = l.trim();
                        if t.starts_with("export ") { &t[7..] } else { l }
                    }).collect::<Vec<_>>().join("\n");
                    bundle.push_str(&format!(
    "\n/* ═════════ {file_name} ═════════ */\n{stripped}\n"
));
                }
            }
            if !bundle.is_empty() {
                let dest = self.output_dir.join("bundle.js");
                std::fs::write(&dest, &bundle).map_err(|e| io_err(&dest, e))?;
                written.push(PlannedFile { path: dest, kind: "runtime bundle" });
            }
        }
        if !skip_runtimes && !do_single_js {
            for (file_name, source) in runtime_assets {
                let dest = self.output_dir.join(file_name);
                std::fs::copy(source, &dest).map_err(|e| io_err(&dest, e))?;
                written.push(PlannedFile {
                    path: dest,
                    kind: "runtime asset",
                });
            }
        }
        Ok(written)
    }

    #[must_use]
    pub fn entrypoints(&self) -> &[BundleEntrypoint] {
        &self.entrypoints
    }

    /// Dry-run: the files `execute` would write.
    #[must_use]
    pub fn plan(&self) -> Vec<PlannedFile> {
        let mut files = Vec::new();
        for ep in &self.entrypoints {
            let base = self.output_dir.clone();
            match ep.mode {
                BundleMode::Bin => files.push(PlannedFile {
                    path: base.join(format!("{}.js", ep.name)),
                    kind: "bin wrapper",
                }),
                BundleMode::Worker => {
                    files.push(PlannedFile {
                        path: base.join(format!("{}-worker.js", ep.name)),
                        kind: "worker",
                    });
                    files.push(PlannedFile {
                        path: base.join(format!("{}-worker-host.js", ep.name)),
                        kind: "worker host",
                    });
                }
                BundleMode::Service => files.push(PlannedFile {
                    path: base.join(format!("{}-sw.js", ep.name)),
                    kind: "service worker",
                }),
            }
            if !matches!(ep.packaging, JsPackaging::SingleFile(_)) {
                files.push(PlannedFile {
                    path: base.join(format!("{}.wasm", ep.name)),
                    kind: "wasm binary",
                });
            }
        }
        files
    }

    /// Generate everything: `bin/` sources + Cargo `[[bin]]` entries (inner
    /// generator), `cargo build --target wasm32-unknown-unknown`, wrappers,
    /// bundles, and (unless `skip_runtimes`) the JS runtime assets.
    ///
    /// # Errors
    /// Any IO/compile failure, with the offending step in the message.
    pub fn execute(
        &self,
        release: bool,
        skip_runtimes: bool,
        runtime_assets: &[(&str, &Path)],
    ) -> Result<Vec<PlannedFile>, WasmBinError> {
        self.inner.generate()?;

        let mut cargo = Command::new("cargo");
        cargo
            // Host RUSTFLAGS (e.g. -fuse-ld=lld) may leak from parent
            // build scripts and aren't valid for the wasm32 backend.
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .arg("build")
            .arg("--target")
            .arg("wasm32-unknown-unknown")
            .current_dir(&self.crate_dir);
        // wasm32 needs the LLVM backend; this workspace's dev profile uses
        // Cranelift, so non-release builds go through the `uat` profile.
        if release {
            cargo.arg("--release");
        } else {
            cargo.arg("--profile").arg("uat");
        }
        let status = cargo
            .status()
            .map_err(|e| io_err(&self.crate_dir, e))?;
        if !status.success() {
            return Err(io_err(
                &self.crate_dir,
                std::io::Error::other("cargo build failed"),
            ));
        }

        std::fs::create_dir_all(&self.output_dir)
            .map_err(|e| io_err(&self.output_dir, e))?;

        let profile_dir = if release { "release" } else { "uat" };
        let mut written = Vec::new();
        for ep in &self.entrypoints {
            let wasm_path = self
                .crate_dir
                .join("target/wasm32-unknown-unknown")
                .join(profile_dir)
                .join(format!("{}.wasm", ep.name));
            let wasm_bytes = std::fs::read(&wasm_path)
                .map_err(|e| io_err(&wasm_path, e))?;
            written.extend(self.write_entrypoint(ep, &wasm_bytes)?);
        }

        let do_single_js = self.entrypoints.iter().any(|ep| ep.jsruntime_single);
        if do_single_js {
            let mut bundle = String::new();
            for (file_name, source) in runtime_assets {
                if let Ok(content) = std::fs::read_to_string(source) {
                    let stripped = content.lines().map(|l| {
                        let t = l.trim();
                        if t.starts_with("export ") { &t[7..] } else { l }
                    }).collect::<Vec<_>>().join("\n");
                    bundle.push_str(&format!(
    "\n/* ═════════ {file_name} ═════════ */\n{stripped}\n"
));
                }
            }
            if !bundle.is_empty() {
                let dest = self.output_dir.join("bundle.js");
                std::fs::write(&dest, &bundle).map_err(|e| io_err(&dest, e))?;
                written.push(PlannedFile { path: dest, kind: "runtime bundle" });
            }
        }
        if !skip_runtimes && !do_single_js {
            for (file_name, source) in runtime_assets {
                let dest = self.output_dir.join(file_name);
                std::fs::copy(source, &dest).map_err(|e| io_err(&dest, e))?;
                written.push(PlannedFile {
                    path: dest,
                    kind: "runtime asset",
                });
            }
        }

        // Generate index.html — one regular <script> per entrypoint using
        // dynamic import(). Static imports inside <script type="module">
        // fail on Android WebView when served through Tauri's custom
        // protocol handler (ewe://localhost / http://ewe.localhost) because
        // the WebView resolves relative module specifiers against a base
        // URL it cannot navigate. Dynamic import() is an expression, works
        // in classic scripts on Chrome 63+, and resolves correctly.
        let bins: Vec<_> = self.entrypoints.iter()
            .filter(|ep| matches!(ep.mode, BundleMode::Bin))
            .collect();
        if !bins.is_empty() {
            let mut init_blocks = String::new();
            for ep in &bins {
                let name = &ep.name;
                init_blocks.push_str(&format!(
                    r#"  <script>
    // ── {name} ── (dynamic import — static <script type="module">
    //  fails on Android WebView through custom protocols)
    import('./{name}.js').then(function(m) {{
      return m.init();
    }}).then(function() {{
      console.log('[platform] {name}: WASM active');
    }}).catch(function(err) {{
      console.error('[platform] {name}:', err);
    }});
  </script>
"#
                ));
            }

            let html = format!(
                r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Foundation Platform</title>
  <style>
    body {{
      font-family: system-ui, sans-serif;
      padding: 16px;
      background: #0a0a1a;
      color: #ccd6f6;
    }}
  </style>
  <script src="bundle.js"></script>
  <script>
    setTimeout(function() {{
      if (typeof invokeIpc !== 'function') return;
      console.error('[TEST] firing present_modal...');
      invokeIpc('chrome', 'present_modal', {{url:'http://ewe.localhost/app/',style:'bottom_sheet',title:'WASM Test'}})
        .then(function(r) {{ console.error('[TEST] MODAL OK ' + JSON.stringify(r)); }})
        .catch(function(e) {{ console.error('[TEST] MODAL FAIL ' + e.message); }});
    }}, 2000);
  </script>
</head>
<body>
<body>
{init_blocks}
</body>
</html>
"#
            );
            let dest = self.output_dir.join("index.html");
            std::fs::write(&dest, &html).map_err(|e| io_err(&dest, e))?;
            written.push(PlannedFile { path: dest, kind: "index.html" });
        }

        Ok(written)
    }

    fn write_entrypoint(
        &self,
        ep: &BundleEntrypoint,
        wasm_bytes: &[u8],
    ) -> Result<Vec<PlannedFile>, WasmBinError> {
        // (file name, contents, kind) — written in one pass below.
        let mut outputs: Vec<(String, Vec<u8>, &'static str)> = Vec::new();

        let use_bundle = ep.jsruntime_single;
        let wrapper = match ep.mode {
            BundleMode::Bin => js_wrapper::bin_wrapper_with_bundle(&ep.name, use_bundle),
            BundleMode::Worker => {
                outputs.push((
                    format!("{}-worker-host.js", ep.name),
                    js_wrapper::worker_host(&ep.name).into_bytes(),
                    "worker host",
                ));
                js_wrapper::worker_wrapper(&ep.name)
            }
            BundleMode::Service => js_wrapper::service_wrapper(&ep.name, &ep.routes),
        };

        match ep.packaging {
            JsPackaging::Separate => {
                outputs.push((
                    format!("{}.wasm", ep.name),
                    wasm_bytes.to_vec(),
                    "wasm binary",
                ));
                outputs.push((wrapper_file_name(ep), wrapper.into_bytes(), "js wrapper"));
            }
            JsPackaging::SingleFile(encoding) => outputs.push((
                wrapper_file_name(ep),
                bundler::bundle_single_file(&wrapper, wasm_bytes, encoding).into_bytes(),
                "js wrapper",
            )),
        }

        let mut written = Vec::new();
        for (file, contents, kind) in outputs {
            let path = self.output_dir.join(file);
            std::fs::write(&path, contents).map_err(|e| io_err(&path, e))?;
            written.push(PlannedFile { path, kind });
        }
        Ok(written)
    }
}

fn wrapper_file_name(ep: &BundleEntrypoint) -> String {
    match ep.mode {
        BundleMode::Bin => format!("{}.js", ep.name),
        BundleMode::Worker => format!("{}-worker.js", ep.name),
        BundleMode::Service => format!("{}-sw.js", ep.name),
    }
}

/// Map one scanned attribute set onto a [`BundleEntrypoint`].
#[must_use]
pub fn entrypoint_from_attrs(
    fn_name: &str,
    mode: BundleMode,
    attrs: &HashMap<String, AttributeValue, impl std::hash::BuildHasher>,
) -> BundleEntrypoint {
    let string_of = |key: &str| match attrs.get(key) {
        Some(AttributeValue::String(s)) => Some(s.clone()),
        _ => None,
    };
    let single_file = string_of("js").as_deref() == Some("single-file");
    let encoding = match string_of("encoded").as_deref() {
        Some("b64") => Encoding::B64,
        _ => Encoding::Uint8Array,
    };
    let routes = match attrs.get("routes") {
        Some(AttributeValue::List(items)) => items
            .iter()
            .filter_map(|v| match v {
                AttributeValue::String(s) => Some(s.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };
    BundleEntrypoint {
        name: fn_name.to_string(),
        mode,
        packaging: if single_file {
            JsPackaging::SingleFile(encoding)
        } else {
            JsPackaging::Separate
        },
        routes,
        jsruntime_single: true,
    }
}
