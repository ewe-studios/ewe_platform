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
    assert_eq!(ep.packaging, JsPackaging::Separate, "separate is the default");

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
    assert_eq!(sub.get_one::<String>("output").unwrap(), "dist");
    assert_eq!(sub.get_one::<String>("crate_directory").unwrap(), "my-crate");
}
