//! WHY: Feature 10's wrappers and bundles are the deployable surface — their
//! structure per mode, the single-file embedding forms, and the CLI flag
//! contract must hold without compiling any actual WASM.
//!
//! WHAT: Spec tests 3 (flag conflict), 6-8 (mode macro validation lives
//! in-macro), 9-14 (wrapper/bundle structure) + attribute mapping.

use foundation_wasm_ui::build_tools::{
    bundler, entrypoint_from_attrs, js_wrapper, BundleMode, Encoding, JsPackaging,
};

/// Test 9 — bin wrapper structure.
#[test]
fn bin_wrapper_structure() {
    let js = js_wrapper::bin_wrapper("auth_app");
    assert!(js.contains("WebAssembly.instantiate"));
    assert!(js.contains("fetch(WASM_URL)"));
    assert!(js.contains("export async function init"));
    assert!(js.contains("instance.exports.auth_app()"));
    assert!(js.contains("./auth_app.wasm"));
}

/// Test 10 — worker pair.
#[test]
fn worker_wrapper_pair() {
    let worker = js_wrapper::worker_wrapper("data_worker");
    assert!(worker.contains("self.postMessage({ ready: true })"));
    assert!(worker.contains("self.onmessage"));

    let host = js_wrapper::worker_host("data_worker");
    assert!(host.contains("new Worker(workerUrl"));
    assert!(host.contains("./data_worker-worker.js"));
    assert!(host.contains("terminate"));
}

/// Test 11 — service worker: fetch listener + route table.
#[test]
fn service_wrapper_routes() {
    let js = js_wrapper::service_wrapper(
        "api_service",
        &[String::from("/api/auth"), String::from("/api/data")],
    );
    assert!(js.contains("addEventListener('fetch'"));
    assert!(js.contains("const ROUTES = ['/api/auth', '/api/data'];"));
    assert!(js.contains("url.pathname.startsWith(route)"));
    assert!(js.contains("skipWaiting"));
}

/// Tests 12 + 14 — uint8array bundle embeds bytes and drops fetch.
#[test]
fn uint8array_bundle_self_contained() {
    let wrapper = js_wrapper::bin_wrapper("app");
    let bundled = bundler::bundle_single_file(&wrapper, &[0, 97, 115, 109], Encoding::Uint8Array);
    assert!(bundled.contains("new Uint8Array([0,97,115,109])"));
    assert!(!bundled.contains("fetch(WASM_URL)"), "no network load");
}

/// Test 13 — b64 bundle embeds base64 + atob, drops fetch.
#[test]
fn b64_bundle_self_contained() {
    let wrapper = js_wrapper::bin_wrapper("app");
    let bundled = bundler::bundle_single_file(&wrapper, b"\0asm", Encoding::B64);
    assert!(bundled.contains("atob("));
    assert!(bundled.contains("AGFzbQ=="), "base64 of \\0asm: {bundled}");
    assert!(!bundled.contains("fetch(WASM_URL)"));
}

/// Base64 edge cases (padding).
#[test]
fn base64_padding() {
    assert_eq!(bundler::base64_encode(b""), "");
    assert_eq!(bundler::base64_encode(b"f"), "Zg==");
    assert_eq!(bundler::base64_encode(b"fo"), "Zm8=");
    assert_eq!(bundler::base64_encode(b"foo"), "Zm9v");
}

/// Attribute → entrypoint mapping (the scanner-side contract).
#[test]
fn attribute_mapping() {
    use foundation_codegen::AttributeValue;
    use std::collections::HashMap;

    let mut attrs = HashMap::new();
    attrs.insert(
        String::from("js"),
        AttributeValue::String(String::from("single-file")),
    );
    attrs.insert(
        String::from("encoded"),
        AttributeValue::String(String::from("b64")),
    );
    let ep = entrypoint_from_attrs("auth_app", BundleMode::Bin, &attrs);
    assert_eq!(ep.name, "auth_app");
    assert_eq!(ep.packaging, JsPackaging::SingleFile(Encoding::B64));

    let empty = HashMap::new();
    let ep = entrypoint_from_attrs("w", BundleMode::Worker, &empty);
    assert_eq!(
        ep.packaging,
        JsPackaging::Separate,
        "separate is the default"
    );

    let mut routed = HashMap::new();
    routed.insert(
        String::from("routes"),
        AttributeValue::List(vec![AttributeValue::String(String::from("/api"))]),
    );
    let ep = entrypoint_from_attrs("svc", BundleMode::Service, &routed);
    assert_eq!(ep.routes, vec!["/api"]);
}

/// Spec test 3 — `--release --dev` conflict at the CLI layer.
#[test]
fn release_dev_flags_conflict() {
    let cmd = foundation_wasm_ui::cli::command();
    let err = cmd
        .try_get_matches_from(["wasm-bundle", "build", "--release", "--dev"])
        .unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
}

/// Spec tests 1-2 / 4-5 surface: flags parse with their defaults.
#[test]
fn cli_flags_parse() {
    let cmd = foundation_wasm_ui::cli::command();
    let matches = cmd
        .try_get_matches_from([
            "wasm-bundle",
            "build",
            "--release",
            "--output",
            "dist",
            "--target",
            "my-crate",
            "--skip-runtimes",
        ])
        .unwrap();
    let ("build", sub) = matches.subcommand().unwrap() else {
        panic!("expected build subcommand");
    };
    assert!(sub.get_flag("release"));
    assert!(sub.get_flag("skip_runtimes"));
    assert_eq!(
        sub.get_one::<std::path::PathBuf>("output").unwrap(),
        std::path::Path::new("dist")
    );
    assert_eq!(
        sub.get_one::<std::path::PathBuf>("crate_directory")
            .unwrap(),
        std::path::Path::new("my-crate")
    );
}

#[test]
fn scanned_modes_generate_the_same_rust_sources_that_build_uses() {
    use foundation_wasm_ui::build_tools::WasmBundleGenerator;
    use std::fs;
    let fixture = std::env::temp_dir().join(format!("ewe-bundle-modes-{}", std::process::id()));
    fs::create_dir(&fixture).unwrap();
    fs::create_dir(fixture.join("src")).unwrap();
    fs::write(fixture.join("Cargo.toml"), "[workspace]\n[package]\nname='sample'\nversion='1.0.0'\nedition='2021'\n[lib]\ncrate-type=['cdylib','rlib']\n").unwrap();
    fs::write(fixture.join("src/lib.rs"), "pub mod screen;\n").unwrap();
    fs::write(fixture.join("src/screen.rs"), "#[wasm_bin]\npub fn dashboard(){}\n#[wasm_worker]\npub fn background(){}\n#[wasm_service]\npub fn service(){}\n").unwrap();
    let generator = WasmBundleGenerator::new(&fixture, &fixture.join("out")).unwrap();
    let plan = generator.generate_sources().unwrap();
    assert_eq!(plan.entrypoints.len(), 3);
    for name in ["dashboard", "background", "service"] {
        let entry = plan.entrypoints.iter().find(|e| e.name == name).unwrap();
        assert_eq!(entry.qualified_path, format!("sample::screen::{name}"));
        assert!(entry.source_file.ends_with("screen.rs"));
        let main = fs::read_to_string(fixture.join(format!("bin/{name}/main.rs"))).unwrap();
        assert!(main.contains(&format!("use sample::screen::{name};")));
    }
    fs::remove_dir_all(fixture).unwrap();
}

#[cfg(feature = "tasks")]
#[test]
fn runtime_task_checks_both_outputs_without_rewriting_them() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = directory.path().join("runtime");
    let output = directory.path().join("output");
    std::fs::create_dir(&runtime).unwrap();
    std::fs::write(
        runtime.join("runtime-entry.js"),
        "globalThis.booted = true;",
    )
    .unwrap();
    let run = |check: bool| {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_ewe-wasm-bundle"));
        command
            .args(["task", "run", "wasm-bundle:runtime", "--runtime-dir"])
            .arg(&runtime)
            .arg("--output")
            .arg(&output);
        if check {
            command.arg("--check");
        }
        command.output().unwrap()
    };
    assert!(run(false).status.success());
    assert!(run(true).status.success());
    for name in ["foundation-ui-runtime.js", "foundation-ui-runtime.min.js"] {
        let path = output.join(name);
        let original = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"changed").unwrap();
        let result = run(true);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(name));
        assert_eq!(std::fs::read(&path).unwrap(), b"changed");
        std::fs::write(&path, original).unwrap();
    }
    assert!(run(true).status.success());
}

#[cfg(feature = "build-hook")]
#[test]
fn cargo_hook_generates_both_distributions() {
    const CHILD: &str = "EWE_TEST_BROWSER_BUILD_HOOK";
    if std::env::var_os(CHILD).is_some() {
        foundation_wasm_ui::build_hook::generate_from_cargo().unwrap();
        return;
    }
    let output = tempfile::tempdir().unwrap();
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "cargo_hook_generates_both_distributions",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .env("OUT_DIR", output.path())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    let tracked = String::from_utf8(result.stdout).unwrap();
    for name in [
        "runtime-entry.js",
        "foundation-ui-actions.js",
        "foundation-ui-vocab.js",
    ] {
        assert!(
            tracked
                .lines()
                .any(|line| line.starts_with("cargo:rerun-if-changed=") && line.ends_with(name)),
            "{tracked}"
        );
    }
    let names = ["foundation-ui-runtime.js", "foundation-ui-runtime.min.js"];
    let readable = std::fs::read(output.path().join(names[0])).unwrap();
    let minified = std::fs::read(output.path().join(names[1])).unwrap();
    assert!(!minified.is_empty() && minified.len() < readable.len());
}
