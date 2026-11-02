//! Build the sqlite-vec loadable extension from source (feature `sqlite-vec`).
//!
//! libsql loads the extension with `load_extension`, which needs a shared
//! object. The `cc` crate builds it for the current target, so every platform
//! with a C compiler works — not only linux-x86_64. The build runs only when
//! the `sqlite-vec` feature is on.

use std::path::PathBuf;

fn main() {
    // The extension exists only for the `sqlite-vec` feature. Cargo turns the
    // feature name's `-` into `_` in `CARGO_FEATURE_*`.
    if std::env::var("CARGO_FEATURE_SQLITE_VEC").is_err() {
        return;
    }

    println!("cargo:rerun-if-changed=vendor/sqlite-vec.c");
    println!("cargo:rerun-if-changed=vendor/sqlite-vec.h");
    println!("cargo:rerun-if-changed=vendor/sqlite3ext.h");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").expect("target os is set by cargo");
    let ext = match target_os.as_str() {
        "macos" => "dylib",
        "windows" => "dll",
        _ => "so",
    };
    let out_path = out_dir.join(format!("vec0.{ext}"));

    // One compile-and-link step, the same as sqlite-vec's own `make loadable`:
    // `cc -fPIC -shared -Ivendor/ sqlite-vec.c -o vec0.so`. sqlite-vec resolves
    // its sqlite3_* calls through `sqlite3ext.h`'s api pointer, so the shared
    // object does not link against sqlite3.
    let compiler = cc::Build::new().get_compiler();
    let mut cmd = compiler.to_command();
    cmd.arg("-fPIC");
    if target_os == "macos" {
        cmd.arg("-dynamiclib");
        cmd.arg("-undefined").arg("dynamic_lookup");
    } else {
        cmd.arg("-shared");
    }
    cmd.arg("-Ivendor");
    cmd.arg("vendor/sqlite-vec.c");
    cmd.arg("-o").arg(&out_path);

    let status = cmd.status().expect("run the C compiler for sqlite-vec");
    assert!(status.success(), "building vendor/sqlite-vec.c failed");

    println!("cargo:rustc-env=VEC0_SO={}", out_path.display());
}
