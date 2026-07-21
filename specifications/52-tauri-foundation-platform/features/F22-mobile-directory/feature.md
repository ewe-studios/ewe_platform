---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F22-mobile-directory"
this_file: "specifications/52-tauri-foundation-platform/features/F22-mobile-directory/feature.md"

status: completed
priority: critical
created: 2026-07-19

depends_on:
  - "F21-multi-app-distribution-and-webview"

tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---
# F22 — MobileDirectory: disk-backed asset serving

## Problem

`EmbedDirectoryAs` embeds file bytes as `&'static [u8]` into the `.so` (release
mode). Every app bundle update requires a full APK rebuild. For OTA
distribution (F21), `.wasm` + `.js` bundles must land on-device and be
servable without recompiling.

## Solution

A new **`MobileDirectory` trait** and a matching `#[derive(MobileDirectory)]`
proc macro. This is a standalone system — it does NOT wrap `EmbeddableDirectory`
or touch `embedders.rs`.

### Trait — `foundation_nostd::embeddable::MobileDirectory`

```rust
pub trait MobileDirectory {
    /// Static file metadata built at compile time from the #[source] directory.
    const FILES_METADATA: &'static [FileInfo];

    /// The runtime asset root. All file reads join against this path.
    fn root(&self) -> &Path;

    /// Read file content from disk: `self.root().join(target)`.
    fn read_utf8_for(&self, target: &str) -> Option<Vec<u8>> {
        let p = self.root().join(target);
        if !p.exists() || p.is_dir() { return None; }
        std::fs::read(p).ok()
    }

    /// Read file content as UTF-16 LE bytes.
    fn read_utf16_for(&self, target: &str) -> Option<Vec<u8>> {
        let p = self.root().join(target);
        if !p.exists() || p.is_dir() { return None; }
        std::fs::read_to_string(p).ok()
            .map(|s| s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect())
    }

    /// Iterate over compile-time metadata.
    fn info_iter(&self) -> core::slice::Iter<'static, FileInfo> {
        Self::FILES_METADATA.iter()
    }

    /// Look up metadata by source path.
    fn info_for(&self, source: &str) -> Option<&FileInfo> {
        Self::FILES_METADATA.iter()
            .find(|i| i.source_path == source || i.source_path_from_parent == source)
    }
}
```

### Derive macro — `#[derive(MobileDirectory)]`

New file: `backends/foundation_macros/src/mobile_directory.rs`

Same directory walk and metadata collection as `EmbedDirectoryAs`, but
generates a `MobileDirectory` trait impl instead — no `FILES_DATA`, no
byte embedding, no `DebugOrRelease` split. Always disk-backed.

The struct MUST have a `root: PathBuf` field — generated code references
`self.root`, so Rust catches missing fields at compile time.

```rust
#[derive(MobileDirectory)]
#[source = "$CARGO_MANIFEST_DIR/public/app"]
pub struct AppAssets { pub root: PathBuf }
```

### Generated code

```rust
impl MobileDirectory for AppAssets {
    const FILES_METADATA: &'static [FileInfo] = &[
        FileInfo::create(Some(1), "...", "index.html", ...),
        FileInfo::create(Some(2), "...", "bundle.js", ...),
        // ...
    ];

    fn root(&self) -> &Path { self.root.as_ref() }
    // read_utf8_for, read_utf16_for, info_iter, info_for use defaults
}
```

## Requirements

### 1. `MobileDirectory` trait ✅
- File: `backends/foundation_nostd/src/embeddable.rs` (or new `mobile.rs`)
- Associated constant: `FILES_METADATA`
- Method: `fn root(&self) -> &Path`
- Default methods: `read_utf8_for`, `read_utf16_for`, `info_iter`, `info_for`
- Re-exported from `foundation_nostd::embeddable`

### 2. `MobileDirectory` derive macro 🔄
- File: `backends/foundation_macros/src/mobile_directory.rs` (NEW)
- Registered in `lib.rs` as `#[proc_macro_derive(MobileDirectory, attributes(source))]`
- Scans `#[source]` directory, builds `FILES_METADATA`
- Generates `impl MobileDirectory` with `root()` and defaults
- Does NOT touch `embedders.rs`

### 3. `codegen.rs` update 🔄
- Generates `#[derive(MobileDirectory)]` instead of `#[derive(EmbedDirectoryAs)]`
- Generated struct has `pub root: PathBuf`

### 4. `WebviewApp` responder update 🔄
- Now generic over `MobileDirectory` trait
- Calls `self.assets.read_utf8_for(&file)` through the trait

### 5. Integration 🔄
- Platform demo + android example both work

## Verification

```bash
cargo build -p foundation_macros              # macro compiles
cargo build -p foundation_nostd               # trait compiles
cargo build                                    # codegen + integration
cargo tauri android build --target x86_64      # APK with MobileDirectory
adb install ... && adb shell am start ...
# console: "[platform] platform_dashboard: WASM active"
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_nostd/src/mobile.rs` | **NEW** — `MobileDirectory` trait |
| `backends/foundation_nostd/src/lib.rs` | Add `pub mod mobile;` + re-export |
| `backends/foundation_macros/src/mobile_directory.rs` | **NEW** — derive macro |
| `backends/foundation_macros/src/lib.rs` | Register derive |
| `backends/foundation_platform/src/codegen.rs` | Switch to `MobileDirectory` |
| `backends/foundation_platform/src/responder.rs` | Add `MobileDirectory` variant |
| `examples/platform_android/src-tauri/src/lib.rs` | Use `AppAssets::new(root)` |
