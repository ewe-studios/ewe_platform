//! Modal native module — native Dialog + BottomSheet with embedded WebView.
//!
//! Provides:
//!   - `register()` — injects ModalHelper.kt/Swift into gen/
//!   - `handler.rs` — ModalIpc (Ipc + PlatformIpc)
//!   - `wasm.rs` — typed WASM wrapper (Modal::present / dismiss)

use crate::native_module::{AndroidModuleConfig, KotlinSource, NativeModule};

/// Register this module with the codegen pipeline.
pub fn register(pipeline: &mut crate::pipeline::PlatformCodegen) {
    pipeline.register(Module);
}

struct Module;

impl NativeModule for Module {
    fn name(&self) -> &str {
        "modal"
    }

    fn android(&self) -> Option<AndroidModuleConfig> {
        Some(AndroidModuleConfig {
            kotlin_sources: vec![KotlinSource::Inline {
                filename: "ModalHelper.kt".into(),
                source: include_str!("android/ModalHelper.kt"),
            }],
            gradle_dependencies: vec![
                "com.google.android.material:material:1.12.0".into(),
            ],
            permissions: vec![],
        })
    }
}

pub mod handler;
pub mod wasm;
