//! Tests extracted from cranelift.rs
mod tests {
    use foundation_buildtools::*;

    #[test]
    fn detection_without_env() {
        // When CARGO_ENCODED_RUSTFLAGS is absent, cranelift is not active.
        assert!(!is_cranelift_active());
    }
}

mod profile_detection {
    use foundation_buildtools::{profile_codegen_backend, profile_from_out_dir};
    use std::path::Path;

    const MANIFEST: &str = r#"
[workspace]
members = ["a"]

[profile.dev]
opt-level = 0
codegen-backend = "cranelift" # fast debug builds

[profile.uat]
inherits = "dev"
codegen-backend = "llvm"

[profile.ci]
inherits = "dev"

[profile.dev.package."*"]
opt-level = 0

[profile.release]
lto = true
"#;

    #[test]
    fn profile_comes_from_the_out_dir() {
        let out = |p: &str| profile_from_out_dir(Path::new(p));
        assert_eq!(out("/t/debug/build/foundation_core-1a2b/out").as_deref(), Some("dev"));
        assert_eq!(out("/t/uat/build/foundation_core-1a2b/out").as_deref(), Some("uat"));
        assert_eq!(out("/t/release/build/x-9/out").as_deref(), Some("release"));
        assert_eq!(
            out("/t/x86_64-unknown-linux-gnu/uat/build/x-9/out").as_deref(),
            Some("uat"),
            "with an explicit --target"
        );
        assert_eq!(out("/nowhere/out"), None);
    }

    #[test]
    fn backend_follows_the_active_profile_not_any_mention() {
        assert_eq!(profile_codegen_backend(MANIFEST, "dev").as_deref(), Some("cranelift"));
        assert_eq!(
            profile_codegen_backend(MANIFEST, "uat").as_deref(),
            Some("llvm"),
            "uat inherits dev but sets llvm"
        );
        assert_eq!(
            profile_codegen_backend(MANIFEST, "ci").as_deref(),
            Some("cranelift"),
            "inherited from dev"
        );
        assert_eq!(
            profile_codegen_backend(MANIFEST, "test").as_deref(),
            Some("cranelift"),
            "test's built-in parent is dev"
        );
        assert_eq!(profile_codegen_backend(MANIFEST, "release"), None);
        assert_eq!(profile_codegen_backend(MANIFEST, "bench"), None);
    }

    #[test]
    fn a_cycle_ends() {
        let cyclic = "[profile.a]\ninherits = \"b\"\n[profile.b]\ninherits = \"a\"\n";
        assert_eq!(profile_codegen_backend(cyclic, "a"), None);
    }
}
