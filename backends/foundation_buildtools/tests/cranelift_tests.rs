//! Tests extracted from cranelift.rs
mod tests {
    use foundation_buildtools::*;

    #[test]
    fn detection_without_env() {
        // When CARGO_ENCODED_RUSTFLAGS is absent, cranelift is not active.
        assert!(!is_cranelift_active());
    }
}
