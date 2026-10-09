// Vendored from getrandom 0.4.2 (MIT OR Apache-2.0).
// Adaptation: removed `feature = "wasm_js"` gate — always provide wasm_js backend on wasm targets.

cfg_if::cfg_if! {
    if #[cfg(getrandom_backend = "custom")] {
        mod custom;
        pub use custom::*;
    } else if #[cfg(getrandom_backend = "linux_getrandom")] {
        mod getrandom;
        pub use getrandom::*;
    } else if #[cfg(getrandom_backend = "linux_raw")] {
        mod linux_raw;
        pub use linux_raw::*;
    } else if #[cfg(getrandom_backend = "rdrand")] {
        mod rdrand;
        pub use rdrand::*;
    } else if #[cfg(getrandom_backend = "rndr")] {
        mod rndr;
        pub use rndr::*;
    } else if #[cfg(getrandom_backend = "efi_rng")] {
        mod efi_rng;
        pub use efi_rng::*;
    } else if #[cfg(getrandom_backend = "windows_legacy")] {
        mod windows_legacy;
        pub use windows_legacy::*;
    } else if #[cfg(getrandom_backend = "unsupported")] {
        mod unsupported;
        pub use unsupported::*;
    } else if #[cfg(getrandom_backend = "extern_impl")] {
        pub(crate) mod extern_impl;
        pub use extern_impl::*;
    } else if #[cfg(all(target_os = "linux", target_env = ""))] {
        mod linux_raw;
        pub use linux_raw::*;
    } else if #[cfg(target_os = "espidf")] {
        mod esp_idf;
        pub use esp_idf::*;
    } else if #[cfg(any(
        target_os = "haiku",
        target_os = "redox",
        target_os = "nto",
        target_os = "aix",
    ))] {
        mod use_file;
        pub use use_file::*;
    } else if #[cfg(any(
        target_os = "macos",
        target_os = "openbsd",
        target_os = "vita",
        target_os = "emscripten",
    ))] {
        mod getentropy;
        pub use getentropy::*;
    } else if #[cfg(any(
        all(
            target_os = "android",
            any(
                target_arch = "aarch64",
                target_arch = "arm",
                target_arch = "x86",
                target_arch = "x86_64",
            ),
        ),
        all(
            target_os = "linux",
            any(
                target_arch = "aarch64",
                target_arch = "arm",
                target_arch = "powerpc",
                target_arch = "powerpc64",
                target_arch = "s390x",
                target_arch = "x86",
                target_arch = "x86_64",
                all(
                    target_env = "musl",
                    not(
                        any(
                            target_arch = "riscv64",
                            target_arch = "riscv32",
                        ),
                    ),
                ),
            ),
        )
    ))] {
        mod use_file;
        mod linux_android_with_fallback;
        pub use linux_android_with_fallback::*;
    } else if #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "hurd",
        target_os = "illumos",
        target_os = "cygwin",
        all(target_os = "horizon", target_arch = "arm"),
    ))] {
        mod getrandom;
        pub use getrandom::*;
    } else if #[cfg(target_os = "solaris")] {
        mod solaris;
        pub use solaris::*;
    } else if #[cfg(target_os = "netbsd")] {
        mod netbsd;
        pub use netbsd::*;
    } else if #[cfg(target_os = "fuchsia")] {
        mod fuchsia;
        pub use fuchsia::*;
    } else if #[cfg(any(
        target_os = "ios",
        target_os = "visionos",
        target_os = "watchos",
        target_os = "tvos",
    ))] {
        mod apple_other;
        pub use apple_other::*;
    } else if #[cfg(all(target_family = "wasm", target_os = "wasi"))] {
        cfg_if::cfg_if! {
            if #[cfg(target_env = "p1")] {
                mod wasi_p1;
                pub use wasi_p1::*;
            } else {
                mod wasi_p2_3;
                pub use wasi_p2_3::*;
            }
        }
    } else if #[cfg(target_os = "hermit")] {
        mod hermit;
        pub use hermit::*;
    } else if #[cfg(all(target_arch = "x86_64", target_os = "motor"))] {
        mod rdrand;
        pub use rdrand::*;
    } else if #[cfg(target_os = "vxworks")] {
        mod vxworks;
        pub use vxworks::*;
    } else if #[cfg(target_os = "solid_asp3")] {
        mod solid;
        pub use solid::*;
    } else if #[cfg(all(windows, target_vendor = "win7"))] {
        mod windows_legacy;
        pub use windows_legacy::*;
    } else if #[cfg(windows)] {
        mod windows;
        pub use windows::*;
    } else if #[cfg(all(target_arch = "x86_64", target_env = "sgx"))] {
        mod rdrand;
        pub use rdrand::*;
    } else if #[cfg(all(target_family = "wasm", any(target_os = "unknown", target_os = "none")))] {
        // KEY ADAPTATION: upstream gates this on `feature = "wasm_js"` and emits compile_error!
        // when the feature is absent. We always provide the wasm_js backend — this is the
        // whole reason for vendoring: no feature flag ceremony on wasm targets.
        mod wasm_js;
        pub use wasm_js::*;
    } else {
        compile_error!(
            "target is not supported by foundation_compact entropy. \
            You may need to set a `getrandom_backend` cfg to select a custom backend."
        );
    }
}
