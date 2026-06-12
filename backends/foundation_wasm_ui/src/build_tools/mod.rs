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

// Hosted-prelude import: this module is std-gated inside a no_std crate.
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

        if !skip_runtimes {
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

    fn write_entrypoint(
        &self,
        ep: &BundleEntrypoint,
        wasm_bytes: &[u8],
    ) -> Result<Vec<PlannedFile>, WasmBinError> {
        // (file name, contents, kind) — written in one pass below.
        let mut outputs: Vec<(String, Vec<u8>, &'static str)> = Vec::new();

        let wrapper = match ep.mode {
            BundleMode::Bin => js_wrapper::bin_wrapper(&ep.name),
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
    }
}
