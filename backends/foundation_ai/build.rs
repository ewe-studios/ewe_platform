//! build.rs — Conditionally builds llama-server from the llama.cpp `CMake` tree.
//!
//! Only runs when `LLAMA_SERVER_BUILD=1` is set in the environment.
//! Normal `cargo build` is unaffected.
//!
//! Output: `support/bin/llama-server` (relative to the project root).

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    // Only re-run when the env var changes.
    println!("cargo:rerun-if-env-changed=LLAMA_SERVER_BUILD");

    let should_build = env::var("LLAMA_SERVER_BUILD").is_ok_and(|v| v == "1");

    if !should_build {
        return;
    }

    // CARGO_MANIFEST_DIR is backends/foundation_ai; project root is two levels up.
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let project_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("expected project root two levels up from foundation_ai");

    let llama_dir = project_root.join("infrastructure/llama-bindings/llama.cpp");
    let output_dir = project_root.join("support/bin");
    let output_bin = output_dir.join("llama-server");

    // Source tree must exist.
    if !llama_dir.join("CMakeLists.txt").exists() {
        println!(
            "cargo:warning=llama.cpp source not found at {}",
            llama_dir.display()
        );
        println!("cargo:warning=Run: git submodule update --init infrastructure/llama-bindings/llama.cpp");
        return;
    }

    let build_dir = llama_dir.join("build");
    let cmake_cache = build_dir.join("CMakeCache.txt");
    let src_bin = build_dir.join("bin/llama-server");

    // Copy existing binary if already built and nothing needs cmake.
    if src_bin.exists() && output_bin.exists() && cmake_cache.exists() {
        println!(
            "cargo:warning=llama-server already built at {}",
            output_bin.display()
        );
        println!(
            "cargo:rustc-env=LLAMA_SERVER_BIN_PATH={}",
            output_bin.display()
        );
        return;
    }

    // Configure.
    fs::create_dir_all(&build_dir).unwrap();

    let status = Command::new("cmake")
        .current_dir(&build_dir)
        .args([
            "..",
            "-DLLAMA_BUILD_SERVER=ON",
            // The server lives under tools/ in newer llama.cpp (>= b98xx), gated
            // by LLAMA_BUILD_TOOLS; set it ON explicitly so the server subdir is
            // added regardless of standalone detection.
            "-DLLAMA_BUILD_TOOLS=ON",
            "-DLLAMA_BUILD_TESTS=OFF",
            "-DLLAMA_BUILD_EXAMPLES=OFF",
            // Don't build the new unified `llama` binary (app/) or the embedded
            // Web UI / HTML — we only want the server executable.
            "-DLLAMA_BUILD_APP=OFF",
            "-DLLAMA_BUILD_UI=OFF",
            "-DLLAMA_BUILD_HTML=OFF",
            "-DLLAMA_BUILD_COMMON=ON",
            // Newer llama.cpp defaults BUILD_SHARED_LIBS=ON in a standalone
            // build, which makes llama-server a tiny stub that dynamically loads
            // libllama/libggml from the build tree via an absolute rpath — it
            // breaks once build/ is cleaned and isn't portable. Force static so
            // support/bin/llama-server is self-contained, as it was before.
            "-DBUILD_SHARED_LIBS=OFF",
            "-DCMAKE_BUILD_TYPE=Release",
        ])
        .status()
        .expect("failed to run cmake");

    assert!(status.success(), "cmake configure failed for llama-server");

    // Build only the server target.
    let nproc =
        std::thread::available_parallelism().map_or_else(|_| "4".into(), |n| n.get().to_string());

    let status = Command::new("make")
        .current_dir(&build_dir)
        .args(["llama-server", &format!("-j{nproc}")])
        .status()
        .expect("failed to run make");

    assert!(status.success(), "make llama-server failed");

    assert!(
        src_bin.exists(),
        "llama-server binary not found at {} after build",
        src_bin.display()
    );

    // Copy to support/bin for a stable, discoverable location.
    fs::create_dir_all(&output_dir).unwrap();
    fs::copy(&src_bin, &output_bin).unwrap();

    println!(
        "cargo:warning=llama-server built at {}",
        output_bin.display()
    );
    println!(
        "cargo:rustc-env=LLAMA_SERVER_BIN_PATH={}",
        output_bin.display()
    );
}
