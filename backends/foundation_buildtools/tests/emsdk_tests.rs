//! Tests extracted from emsdk.rs
mod tests {
    use foundation_buildtools::*;

    #[test]
    fn from_env_returns_some_when_set() {
        // If EMSDK_DIR is set (as in our workspace), from_env should return Some.
        // If unset, it should return None. Either way, no panic.
        let result = Emsdk::from_env();
        if std::env::var("EMSDK_DIR").is_ok() {
            assert!(result.is_some());
        } else {
            assert!(result.is_none());
        }
    }

    #[test]
    fn from_workspace_missing() {
        let tmp = std::env::temp_dir().join("foundation_buildtools_test_emsdk");
        assert!(Emsdk::from_workspace(&tmp).is_none());
    }
}
