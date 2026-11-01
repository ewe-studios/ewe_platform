//! Build the `LD_PRELOAD` shim's C entry points.
//!
//! `open` and `openat` are variadic in libc, so stable Rust cannot define them
//! with a correct ABI. The shim ships a small C file that reads the optional
//! mode argument and forwards to the Rust handlers. It is compiled only when
//! the `vfs-preload` feature is enabled on Linux, which is the only target
//! where the shim (and `LD_PRELOAD`) exists.

fn main() {
    let preload = std::env::var("CARGO_FEATURE_VFS_PRELOAD").is_ok();
    let linux = std::env::var("CARGO_CFG_TARGET_OS")
        .map(|os| os == "linux")
        .unwrap_or(false);

    if preload && linux {
        cc::Build::new()
            .file("src/native/vfs/shim/vfs_shim.c")
            .compile("vfs_shim");
    }
}
