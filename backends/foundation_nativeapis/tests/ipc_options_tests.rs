//! Tests extracted from shared/ipc/options.rs
mod tests {
    use foundation_nativeapis::shared::ipc::*;
    use foundation_nativeapis::shared::ipc::options::*;

    #[test]
    fn options_new() {
        let opts = Options::new("test-bus", Label::new("my-endpoint"));
        assert_eq!(opts.identifier, "test-bus");
        assert_eq!(opts.label.as_str(), "my-endpoint");
        assert!(opts.token.is_empty());
        assert!(!opts.controller_affinity);
    }

    #[test]
    fn options_builder() {
        let opts = Options::new("test-bus", Label::new("my-endpoint"))
            .token("secret")
            .controller_affinity(true);
        assert_eq!(opts.token, "secret");
        assert!(opts.controller_affinity);
    }
}
