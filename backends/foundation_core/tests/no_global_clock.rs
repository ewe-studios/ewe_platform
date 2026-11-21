//! The effect lint is active in every core crate (spec 84 T4).
//!
//! A source scan proves that each crate carries a `clippy.toml` that disallows
//! the global clock, the global entropy call and the global ID generator. The
//! lint itself fails `cargo clippy -D warnings` at a new call site.

use std::path::{Path, PathBuf};

/// The crates with core logic. `foundation_platform_sandbox` is planned in spec
/// 77 and has no crate yet, so it is not in this list.
const CORES: &[&str] = &[
    "foundation_core",
    "foundation_auth",
    "foundation_connectrpc",
    "foundation_http",
    "foundation_ai",
];

/// The paths that core logic may not call.
const DISALLOWED: &[&str] = &[
    "std::time::SystemTime::now",
    "std::time::Instant::now",
    "rand::thread_rng",
    "foundation_compact::entropy::fill",
    "foundation_compact::ids::new_scru128",
    "foundation_compact::ids::new_scru128_string",
];

fn crate_root(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(name)
        .join("clippy.toml")
}

#[test]
fn no_global_clock_in_core() {
    for crate_name in CORES {
        let path = crate_root(crate_name);
        let config = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{} must exist: {error}", path.display()));
        assert!(
            config.contains("disallowed-methods"),
            "{crate_name}/clippy.toml must list disallowed methods"
        );
        for disallowed in DISALLOWED {
            assert!(
                config.contains(disallowed),
                "{crate_name}/clippy.toml must disallow `{disallowed}`"
            );
        }
    }
}
