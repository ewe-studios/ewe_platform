//! Build script for `foundation_db`.
//!
//! It defines the `json_file_backend` cfg. That cfg is set when the target has
//! filesystem access, so code behind it can use files.

use std::env;

fn main() {
    // Register the cfg so clippy and rustc do not warn about an unknown cfg.
    println!("cargo::rustc-check-cfg=cfg(json_file_backend)");
    println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_FAMILY");
    println!("cargo::rerun-if-env-changed=CARGO_CFG_TARGET_OS");

    let target_is_wasm = env::var("CARGO_CFG_TARGET_FAMILY")
        .is_ok_and(|families| families.split(',').any(|family| family == "wasm"));

    // A wasm target with WASI (WebAssembly System Interface) has filesystem
    // access. `wasm32-unknown-unknown` does not.
    let has_filesystem =
        !target_is_wasm || env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("wasi");

    if has_filesystem {
        println!("cargo::rustc-cfg=json_file_backend");
    }
}
