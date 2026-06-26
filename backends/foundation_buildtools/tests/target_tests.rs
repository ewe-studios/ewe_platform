//! Tests extracted from target.rs
mod tests {
    use foundation_buildtools::*;

    #[test]
    fn native_triples() {
        let info = TargetInfo::from_triple("x86_64-unknown-linux-gnu").unwrap();
        assert!(matches!(info.os, TargetOs::Linux));
        assert!(info.is_native());

        let info = TargetInfo::from_triple("x86_64-pc-windows-msvc").unwrap();
        assert!(matches!(info.os, TargetOs::Windows(WindowsVariant::Msvc)));

        let info = TargetInfo::from_triple("aarch64-apple-darwin").unwrap();
        assert!(matches!(info.os, TargetOs::Apple(AppleVariant::MacOS)));

        let info = TargetInfo::from_triple("aarch64-apple-ios").unwrap();
        assert!(matches!(info.os, TargetOs::Apple(AppleVariant::Other)));

        let info = TargetInfo::from_triple("aarch64-linux-android").unwrap();
        assert!(matches!(info.os, TargetOs::Android));
    }

    #[test]
    fn wasm_triples() {
        let info = TargetInfo::from_triple("wasm32-unknown-unknown").unwrap();
        assert!(info.is_wasm());
        assert!(matches!(
            info.os,
            TargetOs::Wasm(WasmFlavor::UnknownUnknown)
        ));
    }

    #[test]
    fn unknown_triple_errors() {
        assert!(TargetInfo::from_triple("riscv64gc-unknown-none-elf").is_err());
    }
}
