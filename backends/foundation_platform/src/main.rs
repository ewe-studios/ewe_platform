//! `ewe-manifest` binary entry point (F40).
//!
//! WHY: manifest keys, generation, signing, and verification are needed
//! outside a build script — rotating a key, inspecting a manifest a device
//! rejected, or signing a bundle produced by another pipeline.
//!
//! WHAT: a wrapper around [`foundation_platform::cli::run`]. All logic lives
//! in the library so the CLI and every project's `build.rs` share one
//! implementation and cannot drift.
//!
//! HOW: target-gated. On wasm32 there is no filesystem to operate on, so the
//! binary exists only to keep the target compiling.

#[cfg(not(target_family = "wasm"))]
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(foundation_platform::cli::run(&args));
}

#[cfg(target_family = "wasm")]
fn main() {}
