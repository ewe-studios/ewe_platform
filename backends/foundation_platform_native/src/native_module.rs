//! `NativeModule` trait + configuration types (F42).
//!
//! A native module provides platform-specific code, permissions, and
//! dependencies for an IPC handler. The codegen pipeline calls these
//! during `build.rs` to inject Kotlin/Swift files into Tauri's gen/
//! directories.
//!
//! Each module (camera, modal, etc.) implements this trait in its own
//! directory alongside its Kotlin/Swift sources.

use std::path::PathBuf;

// ── NativeModule trait ───────────────────────────────────────────────────

pub trait NativeModule: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn android(&self) -> Option<AndroidModuleConfig> { None }
    fn ios(&self) -> Option<IosModuleConfig> { None }
}

// ── Source types ─────────────────────────────────────────────────────────

pub enum KotlinSource {
    File(PathBuf),
    Inline { filename: String, source: &'static str },
}

pub enum SwiftSource {
    File(PathBuf),
    Inline { filename: String, source: &'static str },
}

// ── Platform configs ─────────────────────────────────────────────────────

pub struct AndroidModuleConfig {
    pub kotlin_sources: Vec<KotlinSource>,
    pub gradle_dependencies: Vec<String>,
    pub permissions: Vec<String>,
}

pub struct IosModuleConfig {
    pub swift_sources: Vec<SwiftSource>,
    pub swift_dependencies: Vec<String>,
    pub info_plist_entries: Vec<(String, String)>,
    pub frameworks: Vec<String>,
}
