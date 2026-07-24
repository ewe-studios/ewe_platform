//! Native module injection pipeline (F42).
//!
//! IPC handlers (camera, biometrics, modal) need platform-native code.
//! This module lets users declare Kotlin/Swift files, Gradle dependencies,
//! Android permissions, and iOS framework requirements — and injects them
//! into Tauri's `gen/android/` and `gen/apple/` directories during build.rs.
//!
//! Tauri's gen/ layout (ground truth):
//!   gen/android/app/src/main/java/{pkg}/MainActivity.kt   ← user-owned, once
//!   gen/android/app/src/main/java/{pkg}/generated/*.kt     ← auto, gitignored
//!   gen/android/app/build.gradle.kts                       ← auto, committed
//!
//! This pipeline adds:
//!   gen/android/app/src/main/java/{pkg}/native/{module}/*.kt  ← from modules
//!   gen/android/app/native_modules.gradle                      ← deps, regenerated

use std::path::{Path, PathBuf};

// ── NativeModule trait ───────────────────────────────────────────────────

/// A native module that provides platform-specific code, permissions,
/// and dependencies. IPC handlers implement this to declare their
/// native requirements. The codegen pipeline calls `inject()` during
/// `build.rs` to copy files and patch the Tauri project.
pub trait NativeModule: Send + Sync + 'static {
    /// Unique module name (e.g. "camera", "biometric", "modal").
    fn name(&self) -> &str;

    /// Android-specific configuration.
    fn android(&self) -> Option<AndroidModuleConfig> { None }

    /// iOS-specific configuration.
    fn ios(&self) -> Option<IosModuleConfig> { None }
}

// ── Source types ─────────────────────────────────────────────────────────

/// A Kotlin source file for Android.
pub enum KotlinSource {
    /// Path to a `.kt` file relative to the crate root.
    File(PathBuf),
    /// Inline source for small helpers.
    Inline { filename: String, source: &'static str },
}

/// A Swift source file for iOS.
pub enum SwiftSource {
    /// Path to a `.swift` file relative to the crate root.
    File(PathBuf),
    /// Inline source for small helpers.
    Inline { filename: String, source: &'static str },
}

// ── Platform configs ─────────────────────────────────────────────────────

/// Android-specific native module configuration.
pub struct AndroidModuleConfig {
    /// Kotlin source files to copy alongside MainActivity.kt (under `native/{module_name}/`).
    pub kotlin_sources: Vec<KotlinSource>,

    /// Gradle `implementation(...)` dependencies.
    pub gradle_dependencies: Vec<String>,

    /// Android permissions to add (e.g. "android.permission.CAMERA").
    pub permissions: Vec<String>,
}

/// iOS-specific native module configuration.
pub struct IosModuleConfig {
    /// Swift source files to copy into the Xcode project.
    pub swift_sources: Vec<SwiftSource>,

    /// CocoaPods / SPM dependencies.
    pub swift_dependencies: Vec<String>,

    /// Info.plist entries — (key, value) pairs.
    pub info_plist_entries: Vec<(String, String)>,

    /// Frameworks to link (e.g. "AVFoundation", "LocalAuthentication").
    pub frameworks: Vec<String>,
}

// ── Pipeline ─────────────────────────────────────────────────────────────

/// Collects registered native modules and injects their code during build.rs.
#[derive(Default)]
pub struct PlatformCodegen {
    modules: Vec<Box<dyn NativeModule>>,
}

impl PlatformCodegen {
    #[must_use]
    pub fn new() -> Self { Self { modules: Vec::new() } }

    /// Register a native module. Order of registration is preserved.
    pub fn register_native_module(&mut self, module: impl NativeModule) {
        self.modules.push(Box::new(module));
    }

    /// Run the injection pipeline for the given Tauri source directory
    /// (`src-tauri/`). Copies native files, patches manifests, writes
    /// Gradle dependency files. Safe to call multiple times — duplicate
    /// injections are detected and skipped.
    ///
    /// # Panics
    ///
    /// Panics if file I/O fails (build.rs environment — failing is correct).
    pub fn inject_native_code(&self, src_tauri: &Path) {
        let android_gen = src_tauri.join("gen/android");
        if android_gen.join("app/build.gradle.kts").exists() {
            self.inject_android(&android_gen);
        }

        let apple_gen = src_tauri.join("gen/apple");
        if apple_gen.join("Sources").exists() || apple_gen.join("Project.swift").exists() {
            self.inject_ios(&apple_gen);
        }
    }

    // ── Android injection ────────────────────────────────────────────────

    fn inject_android(&self, android_dir: &Path) {
        let pkg_path = self.resolve_android_package_path(android_dir);
        let native_base = pkg_path.join("native");
        std::fs::create_dir_all(&native_base).ok();

        let mut all_deps: Vec<String> = Vec::new();
        let mut all_perms: Vec<String> = Vec::new();

        for module in &self.modules {
            if let Some(cfg) = module.android() {
                let module_dir = native_base.join(module.name());
                self.inject_kotlin_sources(&cfg.kotlin_sources, &module_dir);
                all_deps.extend(cfg.gradle_dependencies);
                all_perms.extend(cfg.permissions);
            }
        }

        if !all_deps.is_empty() {
            self.write_gradle_deps(android_dir, &all_deps);
        }
        if !all_perms.is_empty() {
            self.patch_android_manifest(&pkg_path, &all_perms);
        }
    }

    fn inject_kotlin_sources(&self, sources: &[KotlinSource], dest_dir: &Path) {
        std::fs::create_dir_all(dest_dir).ok();
        for src in sources {
            match src {
                KotlinSource::File(path) => {
                    let dest = dest_dir.join(path.file_name().unwrap_or_default());
                    // Skip if dest already exists with same content (idempotent)
                    if let Ok(existing) = std::fs::read_to_string(&dest) {
                        if let Ok(source_content) = std::fs::read_to_string(path) {
                            if existing == source_content { continue; }
                        }
                    }
                    std::fs::copy(path, &dest).unwrap_or_else(|e| {
                        panic!("F42: failed to copy {} → {}: {e}", path.display(), dest.display());
                    });
                    println!("cargo:warning=F42: injected native Kotlin: {}", dest.display());
                }
                KotlinSource::Inline { filename, source } => {
                    let dest = dest_dir.join(filename);
                    std::fs::write(&dest, source).unwrap_or_else(|e| {
                        panic!("F42: failed to write {}", dest.display());
                    });
                    println!("cargo:warning=F42: injected inline Kotlin: {}", dest.display());
                }
            }
        }
    }

    fn write_gradle_deps(&self, android_dir: &Path, deps: &[String]) {
        let gradle_file = android_dir.join("app/native_modules.gradle");
        let mut content = String::from(
            "// Auto-generated by F42 native module pipeline. DO NOT EDIT.\n"
        );
        content.push_str("dependencies {\n");
        for dep in deps {
            content.push_str(&format!("    implementation(\"{dep}\")\n"));
        }
        content.push_str("}\n");

        std::fs::write(&gradle_file, &content).unwrap_or_else(|e| {
            panic!("F42: failed to write native_modules.gradle: {e}");
        });
        println!(
            "cargo:warning=F42: wrote native_modules.gradle ({} deps)",
            deps.len()
        );

        // Patch app/build.gradle.kts to apply native_modules.gradle if needed
        let build_gradle = android_dir.join("app/build.gradle.kts");
        if build_gradle.exists() {
            let existing = std::fs::read_to_string(&build_gradle).unwrap_or_default();
            let marker = "// F42: native_modules.gradle";
            if !existing.contains(marker) {
                let patched = format!("{existing}\n{marker}\napply(from = \"native_modules.gradle\")\n");
                std::fs::write(&build_gradle, &patched).unwrap_or_else(|e| {
                    panic!("F42: failed to patch build.gradle.kts: {e}");
                });
                println!("cargo:warning=F42: patched build.gradle.kts to apply native_modules.gradle");
            }
        }
    }

    fn patch_android_manifest(&self, pkg_path: &Path, permissions: &[String]) {
        let manifest_path = pkg_path.join("AndroidManifest.xml");
        if !manifest_path.exists() {
            // The manifest is in gen/android/app/src/main/
            let alt = pkg_path.parent().map(|p| p.join("AndroidManifest.xml"));
            if let Some(alt_path) = alt {
                if alt_path.exists() {
                    self.patch_manifest_file(&alt_path, permissions);
                    return;
                }
            }
            return;
        }
        self.patch_manifest_file(&manifest_path, permissions);
    }

    fn patch_manifest_file(&self, manifest_path: &Path, permissions: &[String]) {
        let existing = std::fs::read_to_string(manifest_path).unwrap_or_default();
        let mut patched = existing.clone();
        for perm in permissions {
            let perm_xml = format!(
                "    <uses-permission android:name=\"{perm}\" />\n"
            );
            if !existing.contains(&perm_xml.trim()[4..]) {
                // Insert before <application or before </manifest>
                if let Some(pos) = patched.find("<application") {
                    patched.insert_str(pos, &perm_xml);
                } else if let Some(pos) = patched.find("</manifest>") {
                    patched.insert_str(pos, &perm_xml);
                }
            }
        }
        if patched != existing {
            std::fs::write(manifest_path, &patched).unwrap_or_else(|e| {
                panic!("F42: failed to patch AndroidManifest.xml: {e}");
            });
            println!(
                "cargo:warning=F42: patched AndroidManifest.xml ({} permissions)",
                permissions.len()
            );
        }
    }

    fn resolve_android_package_path(&self, android_dir: &Path) -> PathBuf {
        // The package path is derived from the MainActivity location.
        // Common patterns: .../java/com/ewe/platform/
        // Scan src/main/java for the first non-generated directory.
        let java_src = android_dir.join("app/src/main/java");
        if !java_src.exists() {
            return java_src;
        }

        fn find_pkg(dir: &Path) -> Option<PathBuf> {
            for entry in std::fs::read_dir(dir).ok()?.flatten() {
                if entry.path().is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name == "generated" { continue; }
                    // Check if this dir contains MainActivity.kt or is the package root
                    if entry.path().join("MainActivity.kt").exists() {
                        return Some(entry.path());
                    }
                    // Recurse into subdirectories (com/ewe/platform pattern)
                    if let Some(found) = find_pkg(&entry.path()) {
                        return Some(found);
                    }
                }
            }
            None
        }

        find_pkg(&java_src).unwrap_or_else(|| java_src.join("com/ewe/platform"))
    }

    // ── iOS injection (stub — Apple target not yet active) ────────────────

    fn inject_ios(&self, _apple_dir: &Path) {
        // iOS injection will be implemented when the Apple target is active.
        // The NativeModule::ios() config declares Swift sources, frameworks,
        // and Info.plist entries — injection copies them to gen/apple/Sources/
        // and patches the project file.
    }
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    struct TestModule {
        name: &'static str,
        deps: Vec<String>,
        perms: Vec<String>,
    }

    impl NativeModule for TestModule {
        fn name(&self) -> &str { self.name }
        fn android(&self) -> Option<AndroidModuleConfig> {
            Some(AndroidModuleConfig {
                kotlin_sources: vec![
                    KotlinSource::Inline {
                        filename: "TestHelper.kt".into(),
                        source: "package com.ewe.platform.native.test\n\nclass TestHelper",
                    },
                ],
                gradle_dependencies: self.deps.clone(),
                permissions: self.perms.clone(),
            })
        }
    }

    #[test]
    fn pipeline_injects_kotlin_sources() {
        let tmp = std::env::temp_dir().join("f42_test_inject_kotlin");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let android_dir = tmp.join("gen/android");
        let pkg_dir = android_dir.join("app/src/main/java/com/ewe/platform");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("MainActivity.kt"), "class MainActivity").unwrap();
        std::fs::write(android_dir.join("app/build.gradle.kts"), "// existing").unwrap();

        let mut pipeline = PlatformCodegen::new();
        pipeline.register_native_module(TestModule {
            name: "test_module",
            deps: vec!["androidx.core:core:1.12.0".into()],
            perms: vec!["android.permission.CAMERA".into()],
        });
        pipeline.inject_native_code(&tmp);

        // Verify Kotlin source was injected
        let kt_file = pkg_dir.join("native/test_module/TestHelper.kt");
        assert!(kt_file.exists(), "Kotlin file should exist at {}", kt_file.display());
        let content = std::fs::read_to_string(&kt_file).unwrap();
        assert!(content.contains("TestHelper"));

        // Verify Gradle deps file was written
        let gradle_file = android_dir.join("app/native_modules.gradle");
        assert!(gradle_file.exists());
        let gradle_content = std::fs::read_to_string(&gradle_file).unwrap();
        assert!(gradle_content.contains("androidx.core:core:1.12.0"));

        // Verify build.gradle.kts was patched
        let build_gradle = android_dir.join("app/build.gradle.kts");
        let bg_content = std::fs::read_to_string(&build_gradle).unwrap();
        assert!(bg_content.contains("native_modules.gradle"));
        assert!(bg_content.contains("// F42"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn pipeline_idempotent_across_runs() {
        let tmp = std::env::temp_dir().join("f42_test_idempotent");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let android_dir = tmp.join("gen/android");
        let pkg_dir = android_dir.join("app/src/main/java/com/ewe/platform");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(pkg_dir.join("MainActivity.kt"), "class MainActivity").unwrap();
        std::fs::write(android_dir.join("app/build.gradle.kts"), "// existing").unwrap();

        let mut pipeline = PlatformCodegen::new();
        pipeline.register_native_module(TestModule {
            name: "idem",
            deps: vec!["dep:one".into()],
            perms: vec![],
        });

        // Run twice — second run should not double-patch
        pipeline.inject_native_code(&tmp);
        let first = std::fs::read_to_string(android_dir.join("app/build.gradle.kts")).unwrap();

        pipeline.inject_native_code(&tmp);
        let second = std::fs::read_to_string(android_dir.join("app/build.gradle.kts")).unwrap();

        assert_eq!(first, second, "idempotent — second run should not modify files");

        // Kotlin source should still be there
        assert!(pkg_dir.join("native/idem/TestHelper.kt").exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
