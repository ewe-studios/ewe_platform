//! Tests extracted from wasm.rs
mod tests {
    use foundation_buildtools::*;

    #[test]
    fn flavor_from_triple() {
        assert_eq!(
            WasmFlavor::from_triple("wasm32-unknown-unknown"),
            WasmFlavor::UnknownUnknown
        );
        assert_eq!(
            WasmFlavor::from_triple("wasm32-unknown-emscripten"),
            WasmFlavor::Emscripten
        );
        assert_eq!(
            WasmFlavor::from_triple("wasm32-wasip1"),
            WasmFlavor::Wasip1
        );
        assert_eq!(
            WasmFlavor::from_triple("wasm32-wasip2"),
            WasmFlavor::Wasip2
        );
    }

    #[test]
    fn capability_queries() {
        assert!(!WasmFlavor::UnknownUnknown.has_libc());
        assert!(WasmFlavor::Emscripten.has_libc());
        assert!(WasmFlavor::Wasip1.has_libc());

        assert!(!WasmFlavor::UnknownUnknown.has_threads());
        assert!(WasmFlavor::Emscripten.has_threads());
        assert!(!WasmFlavor::Wasip1.has_threads());
        assert!(WasmFlavor::Wasip2.has_threads());

        assert!(WasmFlavor::UnknownUnknown.is_js_hosted());
        assert!(WasmFlavor::Emscripten.is_js_hosted());
        assert!(!WasmFlavor::Wasip1.is_js_hosted());

        assert!(!WasmFlavor::UnknownUnknown.supports_native_c_toolchain());
        assert!(WasmFlavor::Emscripten.supports_native_c_toolchain());
        assert!(!WasmFlavor::Wasip1.supports_native_c_toolchain());
    }

    #[test]
    fn round_trip_triple() {
        for flavor in [
            WasmFlavor::UnknownUnknown,
            WasmFlavor::Emscripten,
            WasmFlavor::Wasip1,
            WasmFlavor::Wasip2,
        ] {
            assert_eq!(WasmFlavor::from_triple(flavor.triple()), flavor);
        }
    }
}
