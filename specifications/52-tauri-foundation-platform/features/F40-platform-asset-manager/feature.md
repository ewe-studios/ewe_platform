---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F40-platform-asset-manager"
this_file: "specifications/52-tauri-foundation-platform/features/F40-platform-asset-manager/feature.md"

status: in-progress
priority: critical
created: 2026-07-23
updated: 2026-07-23

depends_on:
  - "F22-mobile-directory"
  - "F13-cross-platform-builds"
  - "F21-multi-app-distribution-and-webview"

tasks:
  completed: 0
  uncompleted: 16
  total: 16
  completion_percentage: 0%
---
# F40 — Platform Asset Manager: cross-platform bundle resource I/O

## Problem

`MobileDirectory` responders (F22) and the OTA `PackageDirectorate` (F29 Stage 6)
both read and write files via `std::fs`. This works on desktop and iOS where
`resource_dir()` returns a real filesystem path. On **Android**, Tauri's
`resource_dir()` returns `asset://localhost/` — a content URI, not a
filesystem path. `std::fs::read()` and `std::fs::write()` cannot operate on it.

The current `MobileDisk::read_utf8_for()` calls `std::fs::read()` unconditionally,
which silently returns `None` for every bundled asset on Android. The symptom:
WASM app pages return "Not Found: index.html" because the assets are inside the
APK and were never extracted to disk.

### Why `resolve()` + `fs::copy()` doesn't work

The Tauri pattern shown in docs uses `app.path().resolve("file", BaseDirectory::Resource)`
followed by `std::fs::copy()`. On Android, `resolve()` with `BaseDirectory::Resource`
calls `resource_dir()` internally, which returns `asset://localhost/`. The
resulting path is `asset://localhost/path/to/file` — still a content URI.
`std::fs::copy()` from this URI **fails** because it's not a real filesystem path.

### Impact

| Platform | `resource_dir()` returns | `std::fs::read()` works? | Backend |
|----------|-------------------------|------------------------|---------|
| Desktop | Real path (`/path/to/resources`) | Yes | `NativeFs` |
| iOS | Real path (`{exe_dir}/assets`) | Yes | `NativeFs` |
| Android | Content URI (`asset://localhost/`) | **No** | `OverlayFileSystem<AssetResolverFs, NativeFs>` |

### Why the VFS stack instead of file extraction

The initial design extracted APK assets to disk (copy from `AssetResolver` →
AppData on first launch). This has fundamental issues:

1. **Write amplification** — every file is written to disk at startup even though
   the APK already has those bytes on disk. Wastes flash lifetime and startup
   time on cold launch.
2. **Extraction atomicity** — crash mid-extraction leaves a partially-populated
   version directory requiring complex `.tmp` staging and cleanup.
3. **Custom security code** — we would write `safe_join()`, `safe_write()`,
   symlink detection, path traversal rejection from scratch.

The `foundation_nativeapis` VFS stack (F37, `spec 60`) provides a battle-tested
set of traits and implementations:

- `OverlayFileSystem<B, D>` — read-only base + writable delta with CoW, whiteouts,
  and built-in path traversal rejection
- `NativeFs` — `VfsFileSystem` over `std::fs` (canonicalizes root, inherits OS
  symlink semantics)
- `MemoryFs` — in-memory `VfsFileSystem` (tests need no temp dirs)
- `ObservableFs<F>` — wraps any `VfsFileSystem`, emits audit events
- `SyncFs<A>` — bridges async VFS impls to synchronous access

On Android, instead of extracting files to disk at startup, we build:

```
OverlayFileSystem<AssetResolverFs, NativeFs>
  │                        │            └─ Delta: AppData/native_fs (OTA writes land here)
  │                        └─ Base: APK assets via AssetResolver (lazy, on-demand)
  └─ read_file("/app/index.html"):
       1. Check delta (OTA'd version on disk) → serve from disk
       2. Check base (APK bundle) → resolve from AssetResolver
       3. Neither → NotFound
```

**Zero startup extraction**. Reads from the APK base are lazy — `AssetResolver`
only fetches files that are actually requested. OTA writes go to the delta
(NativeFs on AppData). The overlay naturally prefers delta over base.

On desktop/iOS, just `NativeFs` pointing at `resource_dir()`.

### Versioned directory layout

Each app gets its own version lifecycle. Versions are nested under the app
directory, not the other way around:

```
resource_root/
  app/
    v0.1.0/              ← APK bundled
      .ewe_manifest.json  ← codegen-generated at build time (not signed)
      index.html
      bundle.js
      platform_dashboard.wasm
    v0.1.1/              ← OTA-created from manifest sequence=42
      .ewe_manifest.json  ← signed manifest from server, atomically written
      bundle.js           ← only changed files shipped
      platform_dashboard.wasm
  app-hello/
    v0.1.0/              ← APK bundled (never OTA-updated)
      .ewe_manifest.json  ← codegen-generated
      index.html
      hello_dashboard.js
  app-settings/
    v0.2.0/              ← APK bundled at a different version
      .ewe_manifest.json
      index.html
      settings.js
    v0.2.1/              ← OTA update for just this app
      .ewe_manifest.json  ← signed server manifest
      settings.js
```

**Every version directory has a `.ewe_manifest.json`.** No special cases.

**APK-bundled manifest**: generated by the build pipeline at compile time
(see requirement 13). Contains the app's `bundle_version`, a list of all
bundled files with their `sha256` hashes and `size`, and a `source: "apk"`
field. The `signature` field is `null` — the APK itself is the trust root.
It is written into the version directory by `initialize()` alongside the
other extracted assets.

**OTA manifest**: the signed manifest received from the CDN, written
atomically as the final OTA step. `source: "ota"`, `signature` present.
Version directories with a signed manifest are OTA-authored; directories
with an unsigned manifest are APK-bundled.

This uniform structure means every tool, script, or rollback decision can
rely on `.ewe_manifest.json` being present — never a "maybe it's there"
situation.

**Why versions as directories**: a version is a namespace. The APK picks a
version at build time and bundles all its assets under `/{version}/`. An OTA
update can target the same version (in-place update) or a new version
(creates a side directory). Either way, no existing files are ever overwritten
by extraction — extraction only creates new version directories that don't
already exist.

**Why keep 2 versions**: pruning deletes the 3rd-oldest version directory when
a 3rd version appears. This gives us:

- **Current version** — what the app serves from
- **Previous version** — instant rollback
- Older versions — automatically pruned

### Where the version ID comes from

The bundle version is **already baked into every Tauri build**. No new config,
no env var, no build script needed.

```rust
// In builder.rs .setup():
let bundle_version = app.package_info().version.to_string();
// On desktop: reads tauri.conf.json → version ("0.1.0")
// On Android: reads tauri.conf.json → version → APK versionName
```

Tauri's `PackageInfo::version` is populated from the `version` field in
`tauri.conf.json`:

```json
{
  "version": "0.1.0"
}
```

This is **always semver** by Tauri's schema validation. `cargo tauri build`
rejects non-semver versions. It is **always increasing** because that's how
releases work — you don't ship v0.1.0 after v0.1.1.

The version string becomes the directory name: `v{version}` (e.g. `v0.1.0`).
Semver ordering matches lexical ordering (`"0.1.0" < "0.1.1" < "0.2.0"`), so
pruning sorts version directory names naturally.

### Lifecycle mechanics

#### Setup flow

```
PlatformBuilder::build()
  └─► setup(|app| {
        let bundle_version = app.package_info().version.to_string();
        // e.g. "0.1.0"

        let manager = PlatformAssetManager::initialize(&app, &bundle_version);
        // Desktop/iOS: NativeFs(resource_dir()).
        // Android: OverlayFileSystem(AssetResolverFs, NativeFs(app_data/app/v0.1.0/)).

        let session = PlatformSession::new(manager.base_root().to_path_buf(), injector);
        // MobileDirectory responders call session.asset_manager().app_root("app")
        // → base_root/app/v0.1.0/
      })
```

#### New APK (version bump: 0.1.0 → 0.1.1)

```
Launch v0.1.1:
  ┌─ resource_root/app/v0.1.0/ already exists (from prior launch)
  ├─ resource_root/app/v0.1.1/ does NOT exist → ensure NativeFs delta dir exists
  ├─ app_root("app") = resource_root/app/v0.1.1/
  ├─ app_root("app-hello") = resource_root/app-hello/v0.1.0/ (unchanged)
  └─ prune per app: app/ has v0.1.0+v0.1.1 → nothing to delete

Launch v0.1.1 again:
  ┌─ All version dirs exist → no-op
  ├─ app_root() returns current active versions per app
  └─ prune: same → no change
```

#### OTA update — in-place same version for one app

```
OTA manifest: { apps: [{ app_id: "app", bundle_version: "0.1.1", files: [...] }] }

  ┌─ OTA writes over resource_root/app/v0.1.1/bundle.js
  ├─ .ewe_manifest.json atomically updated (sequence bumped)
  ├─ app-hello/ and app-settings/ untouched
  └─ Next page load: app_root("app") reads the updated file
```

#### OTA update — new version for one app

```
OTA manifest: { apps: [{ app_id: "app", bundle_version: "0.1.2", files: [...] }] }

  ┌─ OTA writes files to resource_root/app/v0.1.2/
  ├─ ALL files SHA-256 verified
  ├─ .ewe_manifest.json written atomically
  ├─ app_root("app") → resource_root/app/v0.1.2/
  ├─ app_root("app-hello") → resource_root/app-hello/v0.1.0/ (unchanged)
  └─ prune per app: app/ deletes v0.1.0 (3rd), keeps v0.1.1+v0.1.2
                    app-hello/ v0.1.0 unchanged (only 1 version)
```

#### OTA update — multiple apps, different versions

```
OTA manifest: {
  apps: [
    { app_id: "app", bundle_version: "0.1.2", files: [...] },
    { app_id: "app-settings", bundle_version: "0.2.1", files: [{"path":"settings.js",...}] },
  ]
}

  ┌─ app/v0.1.2/ gets new files + .ewe_manifest.json
  ├─ app-settings/v0.2.1/ gets new files + .ewe_manifest.json
  ├─ Each app independently activates its new version
  └─ Prune runs per-app after all writes complete
```

#### Rollback (per-app)

```
User hits "rollback" for app/:
  ┌─ app_root("app") → resource_root/app/v0.1.1/ (previous version)
  ├─ Instant — v0.1.1/ still on disk, not pruned yet
  ├─ v0.1.2/ and its .ewe_manifest.json remain (can roll forward)
  └─ app-hello/ and app-settings/ unaffected
```

#### APK upgrade AFTER OTA

```
APK version: 0.1.1 (v0.1.1 code, but OTA already created app/v0.1.2/)
  ┌─ app/v0.1.1/ exists (APK bundled at prior launch), v0.1.2/ exists (OTA)
  ├─ APK declares bundle_version "0.1.1" for all apps
  ├─ app_root("app") = app/v0.1.1/ (APK's declared version)
  ├─ OTA'd v0.1.2 still on disk (can activate if safe)
  └─ No extraction — AssetResolverFs serves APK bundles lazily

New APK version: 0.1.3 (newer than OTA's 0.1.2)
  ┌─ app/v0.1.3/ does NOT exist → NativeFs delta dir created
  ├─ AssetResolverFs base layer provides APK's 0.1.3 assets
  ├─ OTA'd v0.1.2/ still on disk but shadowed by the base
  └─ prune per app: app/ deletes v0.1.1, keeps v0.1.2+v0.1.3
```

#### OTA-forced rollback (manifest declares `rollback_to`)

```
OTA manifest: {
  apps: [{ app_id: "app", bundle_version: "0.1.3" }],
  rollback_to: "0.1.0"          ← force rollback for all apps
}

  ┌─ OTA writes files to app/v0.1.3/ (staging for future use)
  ├─ manifest declares rollback_to: "0.1.0"
  ├─ manager checks: app/v0.1.0/ exists? YES → activate
  ├─ app_root("app") now = app/v0.1.0/ (instant, files already there)
  ├─ v0.1.3/ written + .ewe_manifest.json (available if server changes mind)
  └─ v0.1.1 and v0.1.2 were the BAD releases (the rollback avoids them)
```

#### OTA-forced rollback with cleanup (`rollback_to` + `delete_after`)

```
OTA manifest: {
  apps: [{ app_id: "app", bundle_version: "0.1.3" }],
  rollback_to: "0.1.0",
  delete_after: "0.1.2"        ← delete the bad version for ALL apps
}

  ┌─ app_root("app") = app/v0.1.0/
  ├─ app/v0.1.2/ deleted (including its .ewe_manifest.json)
  └─ app/v0.1.0/ + app/v0.1.3/ remain (good + rollback artifacts)
```

#### Manifest structure (APK-bundled and OTA share the same schema)

```json
{
  "schema": 1,
  "source": "ota",
  "manifest_domain": "cdn.ewe.studio",
  "created_at": "2026-07-23T10:00:00Z",
  "sequence": 42,
  "base_url": "https://cdn.ewe.studio/bundles",
  "rollback_to": null,
  "delete_after": null,
  "signature": "dGhpcyBpcyBhIGZha2Ugc2lnbmF0dXJl...",
  "apps": [{
    "app_id": "app",
    "bundle_version": "0.1.2",
    "files": [{
      "path": "index.html",
      "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
      "size": 1024
    }]
  }]
}
```

**OTA manifest** (`source: "ota"`): signed by the CDN's Ed25519 private key,
fetched from `https://{baked_domain}/ewe-manifest.json`. The app verifies
the `signature` against the baked public key. Rejected if invalid.

**APK-bundled manifest** (`source: "apk"`): generated by the codegen build
pipeline at compile time (see requirement 13). One per app. The `signature`
field is `null` — the APK's code signature is the trust root. Written into
the version directory by `initialize()`. Contains the exact files bundled in
the APK with their `sha256` and `size`.

Fields:
- `source`: `"apk"` or `"ota"` — distinguishes origin
- `manifest_domain`: the baked domain (must match; verified for OTA manifests)
- `signature`: Ed25519 signature over canonical JSON (null for APK-bundled)
- `sequence`: monotonic integer (OTA only; APK uses 0)
- `rollback_to` (optional): semver version to activate after download
- `delete_after` (optional): semver version to delete after rollback

### Extraction is now creation, not overwrite

Versioned directories make extraction trivially idempotent:

```rust
fn extract_if_needed(app: &App, app_data: &Path, bundle_version: &str) -> PathBuf {
    let target = app_data.join(bundle_version);

    // If this version directory already exists, extraction is a no-op.
    // It was either extracted previously or created by OTA.
    if !target.exists() {
        std::fs::create_dir_all(&target).ok();
        let resolver = app.asset_resolver();
        for asset_entry in resolver.iter() {
            let asset_key: String = (*asset_entry.0).to_string();
            let dest = target.join(&asset_key);
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Some(asset) = resolver.get(asset_key) {
                let _ = std::fs::write(&dest, &asset.bytes);
            }
        }
    }

    target
}
```

No per-file `exists()` checks. No sentinel files. If the **version directory**
exists, skip everything. If it doesn't, create it and populate it. One atomic
decision per launch.

### Pruning policy

```rust
fn prune_old_versions(app_data: &Path, keep: usize) {
    let mut versions: Vec<(String, PathBuf)> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(app_data) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('v') && entry.path().is_dir() {
                versions.push((name, entry.path()));
            }
        }
    }
    versions.sort_by(|a, b| a.0.cmp(&b.0));
    // Keep the N most recent versions, delete the rest.
    while versions.len() > keep {
        let (_, path) = versions.remove(0);
        let _ = std::fs::remove_dir_all(&path);
    }
}
```

## Solution

A new `PlatformAssetManager` module in `foundation_platform` that wraps the
`foundation_nativeapis` VFS stack behind a unified API. It is the **single
authority** for all bundle resource I/O.

### Architecture

```
PlatformAssetManager
  │
  ├─ Desktop / iOS:  NativeFs(resource_dir())
  │   ┌─ canonicalized root path
  │   ├─ std::fs for all reads/writes
  │   └─ version dirs not used (single canonical path)
  │
  ├─ Android:  OverlayFileSystem<AssetResolverFs, NativeFs>
  │   ┌─ Base layer: AssetResolverFs — lazy reads from APK AssetResolver
  │   │   ┌─ read_file("/app/index.html") → resolver.get("app/index.html")
  │   │   ├─ exists("/app/index.html")     → resolver.get().is_some()
  │   │   └─ Zero RAM preload. Zero startup extraction. On-demand.
  │   │
  │   └─ Delta layer: NativeFs(app_data/v{version}/)
  │       ┌─ OTA writes go here (std::fs against real AppData path)
  │       ├─ Path traversal rejected by OverlayFileSystem (inherited)
  │       ├─ CoW on write: read from base, write to delta
  │       └─ Whiteout on delete: marks base entries as removed
  │
  └─ Opt-in audit: ObservableFs wraps the VFS (behind #[cfg(debug_assertions)])
      ┌─ Emits VfsEvent for every read/write/exists call
      ├─ Controlled by feature flag: vfs-audit
      └─ Off by default in release builds — zero overhead
```

### Key invariant

After `PlatformAssetManager::initialize(app, bundle_version)`, all reads and
writes go through a `Box<dyn VfsFileSystem>`. The VFS backend is chosen at
init time based on the platform:

| Platform | Backend | Base reads from | Writes go to |
|----------|---------|----------------|--------------|
| Desktop | `NativeFs` | `resource_dir()` (disk) | `resource_dir()` (disk) |
| iOS | `NativeFs` | `{exe_dir}/assets` (disk) | `{exe_dir}/assets` (disk) |
| Android | `OverlayFileSystem` | APK `AssetResolver` (lazy) | `AppData/v{version}/` (disk) |

### New type: `AssetResolverFs`

A thin `VfsFileSystem` implementation backed by Tauri's `AssetResolver`. ~40
lines, lives in `foundation_platform/src/assets.rs`:

```rust
use foundation_nativeapis::vfs::{VfsFileSystem, VfsFile, VfsDirectory, VfsResult, VfsError,
    VfsMetadata, VfsCapabilities, VfsFileType, VfsDirEntry, OpenMode};
use tauri::{Manager, Runtime};
use tauri::app::AssetResolver;

/// Read-only VFS backed by Tauri's embedded APK asset bundle.
/// Lazy — no preload, no RAM cost beyond the AssetResolver handle.
pub struct AssetResolverFs<R: Runtime> {
    resolver: AssetResolver<R>,
}

impl<R: Runtime> AssetResolverFs<R> {
    pub fn new(resolver: AssetResolver<R>) -> Self { Self { resolver } }

    fn get_asset(&self, path: &str) -> VfsResult<Vec<u8>> {
        let clean = path.trim_start_matches('/');
        self.resolver.get(clean.to_string())
            .map(|a| a.bytes)
            .ok_or_else(|| VfsError::NotFound { path: clean.to_string() }.into())
    }
}

impl<R: Runtime> VfsFileSystem for AssetResolverFs<R> {
    type File = ReadOnlyVfsFile;
    type SeekableFile = ReadOnlyVfsSeekableFile;
    type Directory = ReadOnlyVfsDirectory
}
```

`ReadOnlyVfsFile` — holds `Vec<u8>` and cursor, delegates `read_at`/`size`/`metadata`.
`ReadOnlyVfsDirectory` — precomputes entries from `AssetResolver::iter()`, rejects
creates/writes. These are ~80 lines total, also in `assets.rs`.

**Total new code in `assets.rs`**: ~200 lines. All I/O security (path traversal,
CoW, symlink detection, atomic writes) comes from `foundation_nativeapis`'s VFS
stack — already tested, already audited.

### API surface

```rust
/// Cross-platform bundle resource manager backed by the VFS stack.
///
/// On desktop/iOS: wraps NativeFs(resource_dir()).
/// On Android: wraps OverlayFileSystem<AssetResolverFs, NativeFs>.
///
/// Opt-in audit logging via feature flag `vfs-audit` — wraps the
/// VFS in ObservableFs at init time (debug/default) or omitted (release).
pub struct PlatformAssetManager {
    /// The VFS backend. All I/O goes through this — never std::fs directly.
    fs: Box<dyn VfsFileSystem>,
    /// Base directory for version directories (Android: app_data; desktop: resource_dir).
    base_root: PathBuf,
    /// The currently active version directory (Android: v{version}/; desktop: same as base).
    active_root: PathBuf,
    /// The bundle version that's currently active (semver string).
    bundle_version: String,
    /// Baked Ed25519 public key for manifest verification (32 bytes, optional).
    manifest_key: Option<[u8; 32]>,
    /// Last accepted manifest sequence number (anti-replay).
    last_manifest_seq: RwLock<u64>,
    /// Ops lock — serializes activate, prune, and manifest processing.
    ops_lock: Mutex<()>,
}

impl PlatformAssetManager {
    /// Initialize the asset manager.
    ///
    /// `bundle_version` comes from `app.package_info().version` —
    /// semver from `tauri.conf.json`, always increasing.
    ///
    /// Backend selection per platform:
    /// - Desktop/iOS: `NativeFs::new(resource_dir())`
    /// - Android: `OverlayFileSystem::new(AssetResolverFs, NativeFs(app_data/v{version}))`
    ///
    /// On Android, `app_data/v{version}/` is created if it doesn't exist
    /// (first launch or new version). OTA writes to this delta layer.
    /// No extraction — APK assets served lazily via AssetResolverFs.
    ///
    /// Audit: under `cfg(feature = "vfs-audit")`, wraps the VFS in
    /// ObservableFs for event logging. Off in release builds.
    pub fn initialize<R: Runtime>(
        app: &App<R>,
        bundle_version: &str,
        manifest_key: Option<[u8; 32]>,
    ) -> Self;

    /// The active resource root path (version directory).
    /// MobileDirectory responders pass this to their `build(root)`.
    pub fn active_root(&self) -> &Path;

    /// The base directory containing all version directories.
    pub fn base_root(&self) -> &Path;

    /// The currently active bundle version string.
    pub fn bundle_version(&self) -> &str;

    /// Read a file through the VFS. Delegates to `self.fs.read_file(path)`.
    /// On Android: checks delta (OTA) first, then base (APK) — lazy, no extraction.
    pub fn read(&self, path: &str) -> VfsResult<Vec<u8>>;

    /// Write a file through the VFS. Delegates to `self.fs.write_file(path, data)`.
    /// On Android: writes to the delta layer (AppData). Creates parent dirs.
    /// Path traversal and symlink following rejected by the VFS stack.
    pub fn write(&self, path: &str, data: &[u8]) -> VfsResult<()>;

    /// Check if a file exists through the VFS.
    pub fn exists(&self, path: &str) -> VfsResult<bool>;

    /// List available version directories (sorted by semver, newest last).
    /// Returns bare version strings (no `v` prefix).
    /// On desktop/iOS: returns empty vec (single canonical dir).
    pub fn list_versions(&self) -> Vec<String>;

    /// Activate a different version directory (OTA, rollback).
    /// On Android: switches the delta layer to a new version dir.
    /// On desktop/iOS: no-op.
    pub fn activate(&self, version: &str) -> Result<(), String>;

    /// Delete a specific version directory. Validates:
    /// - Not the active version
    /// - Not the rollback target
    /// - At least one version remains
    /// No-op (warning log) if the version doesn't exist.
    pub fn delete_version(&self, version: &str, rollback_target: Option<&str>) -> Result<(), String>;

    /// Verify and process an OTA manifest.
    /// 1. Verify Ed25519 signature (if key baked)
    /// 2. Validate base_url against baked domain
    /// 3. Anti-replay: check sequence > last accepted
    /// 4. Validate paths (delegated to VFS path checks)
    /// 5. Enforce file size/count limits
    /// 6. Process rollback_to / delete_after
    pub fn process_manifest(&self, manifest_json: &str) -> Result<Vec<OtaDownload>, String>;

    // ── Test helpers ──
    #[doc(hidden)]
    pub fn _test_active_root(&self) -> PathBuf { self.active_root.clone() }
    #[doc(hidden)]
    pub fn _test_base_root(&self) -> &Path { &self.base_root }
    /// Build a manager with an explicit VFS backend (for tests).
    #[doc(hidden)]
    pub fn from_vfs(fs: Box<dyn VfsFileSystem>, base_root: PathBuf, active_root: PathBuf,
        bundle_version: &str, manifest_key: Option<[u8; 32]>) -> Self;
}
```
```

### Integration plan

1. **`assets.rs` (NEW)**: `PlatformAssetManager` + `AssetResolverFs` +
   `ReadOnlyVfsFile`/`ReadOnlyVfsDirectory`. ~200 lines total.
   Depends on `foundation_nativeapis` (vfs feature) + `tauri`.

2. **`session.rs`**: Add `asset_manager: PlatformAssetManager` field.
   `bundle_root()` delegates to `asset_manager.active_root()`.
   `resource_root` field stays for backward compat (set from `active_root()`).

3. **`builder.rs`**: Replace `resolve_resource_root()` with
   `PlatformAssetManager::initialize(&app, &bundle_version, manifest_key)`.
   `bundle_version` from `app.package_info().version.to_string()`.
   Add `ota_manifest_domain(domain) -> Self` and `ota_manifest_key(key) -> Self`.

4. **`responder.rs`**: No changes. `MobileDisk::read_utf8_for()` continues using
   `std::fs::read()` — this still works because on Android files read from the
   overlay delta are on disk (OTA'd files in AppData). The VFS layer handles
   the read path internally; `MobileDirectory` sees only the filesystem root.

5. **`ota.rs`**: `PackageDirectorate` writes through `PlatformAssetManager::write()`
   (VFS-backed) instead of raw `std::fs`. Manifest gains `signature`, `sequence`,
   `sha256` fields. Domain-locked init.

6. **`foundation_nativeapis`**: Enable the `vfs` feature in `foundation_platform`'s
   `Cargo.toml`. Optionally gate `vfs-audit` behind a feature flag.

7. **Android example**: Remove `embedded_apps.rs`. Standard `MobileDirectory`
   responders just work.

### Startup flow

```
PlatformBuilder::build()
  └─► setup(|app| {
        // 1. Bundle version from tauri.conf.json — always semver.
        let bundle_version = app.package_info().version.to_string();

        // 2. Create the VFS backend.
        //    Desktop: NativeFs::new(resource_dir())
        //    Android: OverlayFileSystem::new(
        //        AssetResolverFs::new(app.asset_resolver()),   // APK → lazy base
        //        NativeFs::new(app_data/v{version}/),          // OTA → delta
        //    )
        //    Audit: cfg(feature = "vfs-audit") → ObservableFs::new(fs)
        let manager = PlatformAssetManager::initialize(&app, &bundle_version, manifest_key);

        // 3. Create session with resolved root.
        let session = PlatformSession::new(manager.active_root().to_path_buf(), injector);
        session.set_asset_manager(manager);

        // 4. MobileDirectory responders work because:
        //    - Desktop/iOS: resource_dir is a real path
        //    - Android: AssetResolverFs resolves from APK (no disk extraction needed)
        // 5. OTA writes go to the delta layer (AppData via NativeFs)
      })
```

### Android VFS overlay details

The overlay pattern replaces extraction entirely. On Android init:

```rust
#[cfg(target_os = "android")]
fn build_android_vfs<R: Runtime>(app: &App<R>, version: &str) -> Box<dyn VfsFileSystem> {
    use foundation_nativeapis::vfs::{OverlayFileSystem, NativeFs, MemoryDelta};
    use tauri::Manager;

    let app_data = app.path().app_data_dir().expect("app_data_dir");
    let version_dir = app_data.join(format!("v{version}"));
    std::fs::create_dir_all(&version_dir).ok();

    // Base: APK assets via AssetResolver — lazy, no extraction.
    let base = AssetResolverFs::new(app.asset_resolver());

    // Delta: AppData + whiteout tracking (OTA writes, rollback support).
    let delta = MemoryDelta::new();
    let delta_fs = NativeFs::new(&version_dir)
        .expect("NativeFs on AppData");

    // Combine: base + delta with CoW and whiteout semantics.
    let overlay = OverlayFileSystem::new(base, delta_fs.clone());

    // Audit: only when feature is enabled.
    #[cfg(feature = "vfs-audit")]
    {
        use foundation_nativeapis::vfs::ObservableFs;
        return Box::new(ObservableFs::new(overlay));
    }
    #[cfg(not(feature = "vfs-audit"))]
    { Box::new(overlay) }
}
```

**No files are extracted at startup.** The overlay reads from the APK base
on demand. Only when an OTA update writes a file does anything land on disk
in the delta layer.

## Requirements

### 1. `PlatformAssetManager` struct (NEW)
- File: `backends/foundation_platform/src/assets.rs` (~200 lines)
- Fields: `fs: Box<dyn VfsFileSystem>`, `base_root`, `active_root`, `bundle_version`, `manifest_key`, `last_manifest_seq`, `ops_lock`
- `initialize(app, bundle_version, manifest_key) -> Self`: builds platform VFS backend
  - Desktop/iOS: `NativeFs::new(resource_dir())`
  - Android: `OverlayFileSystem::new(AssetResolverFs::new(app.asset_resolver()), NativeFs::new(app_data/v{version}/))`
  - Audit: under `cfg(feature = "vfs-audit")`, wraps in `ObservableFs`
  - Prunes old version directories on Android (keep 2)
- `active_root() -> &Path`: the version directory MobileDirectory responders use
- `base_root() -> &Path`: parent of version directories
- `bundle_version() -> &str`: current active version
- `read(path) -> VfsResult<Vec<u8>>`: delegates to `self.fs.read_file(path)`
- `write(path, data) -> VfsResult<()>`: delegates to `self.fs.write_file(path, data)`
- `exists(path) -> VfsResult<bool>`: delegates to `self.fs.exists(path)`
- `list_versions() -> Vec<String>`: sorted version dirs, newest last
- `activate(version) -> Result<(), String>`: switch active_root (Android: rebuilds overlay with new delta dir)
- `delete_version(version, rollback_target)`: validated deletion
- `process_manifest(json)`: manifest verification + processing (all red team fixes applied)
- `from_vfs(...)`: test constructor taking an explicit VFS backend
- I/O security (path traversal, CoW, symlinks) inherited from `foundation_nativeapis` VFS stack
- Does NOT depend on `tauri-plugin-fs`

### 2. `AssetResolverFs` supporting types (NEW)
- File: `backends/foundation_platform/src/assets.rs` (~80 lines)
- `AssetResolverFs<R>`: read-only `VfsFileSystem` backed by Tauri's `AssetResolver`
  - `read_file(path)` → `resolver.get(path).map(|a| a.bytes)`
  - `exists(path)` → `resolver.get(path).is_some()`
  - `list` → builds from `resolver.iter()`
- `ReadOnlyVfsFile`: `VfsFile` holding `Vec<u8>` + cursor; only `read_at`, `size`, `metadata` supported
- `ReadOnlyVfsDirectory`: `VfsDirectory` with precomputed entries; creates/writes return `ReadOnly` error

### 3. Integration into `PlatformSession`
- Add `asset_manager: PlatformAssetManager` field
- `new()` takes `PlatformAssetManager` + `ScriptInjector`
- `bundle_root()` delegates to `asset_manager.active_root()`
- `resource_root` field stays for backward compat
- `asset_manager() -> &PlatformAssetManager` accessor

### 4. `builder.rs` 
- Replace `resolve_resource_root()` with `PlatformAssetManager::initialize(&app, &bundle_version, manifest_key)`
- `bundle_version` from `app.package_info().version.to_string()`
- Add `ota_manifest_domain(domain) -> Self` (compile-time constant, immutable in APK)
- Add `ota_manifest_key(key: [u8; 32]) -> Self` (Ed25519 public key)
- Store manager on session

### 5. No changes to `MobileDirectory` / `MobileDisk` / `responder.rs`
- They continue using `std::fs::read()` — works because:
  - Desktop/iOS: `active_root` is a real filesystem path
  - Android: OTA'd files are on disk in the delta; APK files are served via the VFS overlay internally

### 6. `ota.rs` — manifest security + version-aware writes
- OTA writes through `PlatformAssetManager::write()` (VFS-backed)
- Manifest requires `sha256`, `sequence`, `created_at`, `signature` (Ed25519)
- `base_url` validated against baked domain
- All `path`/`app_id` validated by VFS (path traversal rejected by overlay)
- Anti-replay via `sequence` > stored
- Per-file max 50 MB, per-manifest max 500 files
- `rollback_to` + `delete_after` with validation
- Rollback loop breaker: 3 failures without 30s uptime → lock version
- **Manifest provenance**: the signed manifest JSON is stored as `.ewe_manifest.json`
  in the version directory after successful OTA download + verification
  - Written atomically (`.tmp` → rename) as the final OTA step
  - APK-bundled versions have no manifest file (the APK IS the manifest)
  - Versions without a manifest = APK-bundled; versions with one = OTA-created
  - Pruned alongside the version directory — no orphans

### 7. Android example cleanup
- Remove `embedded_apps.rs` if it exists
- Standard `MobileDirectory` responders

### 8. Desktop/iOS unaffected
- `NativeFs` ← resource_dir(); all existing behavior preserved

### 10. OTA manifest domain — baked at build time, not a runtime parameter
The allowed manifest URL domain(s) are **compile-time constants**, not runtime
arguments. This prevents a compromised or malicious route handler from pointing
the OTA system at an attacker's manifest.

```rust
// In the app's src-tauri/src/lib.rs, part of PlatformBuilder setup:
platform_run!(PlatformBuilder::new()
    .inject_platform_runtimes()
    .ota_manifest_domain("cdn.ewe.studio")   // ← baked at compile time
    .setup(setup_routes));
```

`PackageDirectorate` stores the allowed domain, not a full URL. The manifest
URL is derived: `https://{domain}/ewe-manifest.json`. Only manifests fetched
from this domain are accepted.

```rust
pub struct PackageDirectorate {
    resource_root: PathBuf,
    /// Allowed manifest origin — baked at compile time via PlatformBuilder.
    manifest_domain: String,
    http: HttpBackend,
}

impl PackageDirectorate {
    /// Create a new directorate. `manifest_domain` is a compile-time constant
    /// set via `PlatformBuilder::ota_manifest_domain()`. It is NOT a user
    /// input or runtime parameter.
    pub fn new(resource_root: PathBuf, manifest_domain: &str) -> Self;

    /// Returns the full manifest URL: `https://{domain}/ewe-manifest.json`.
    pub fn manifest_url(&self) -> String;
}
```

**Why bake the domain**:
- Manifest fetched from `https://{baked_domain}/ewe-manifest.json`
- If a manifest redirects or is hosted on any other domain, it's rejected — no
  exception, no fallback, no override
- A compromised IPC handler can't redirect OTA to `evil-cdn.example.com`
- The domain is visible in the binary — auditable, not configurable at runtime
- Multiple domains (staging, production) are handled by per-build config, not runtime parameters
- **The domain is immutable after APK build.** Changing it requires a new APK
  release through the app store. No IPC, no OTA update, no script injection can
  alter it. The only way to change the manifest domain is a new native binary.

### 5. OTA manifest signing — Ed25519, baked public key

The OTA server (CDN) signs every manifest with an Ed25519 private key.
The corresponding public key (32 bytes) is baked into the APK at compile
time. Only the public key ships — the private key lives in CI/CD secrets
and never touches a device.

See requirement 11 for key management details.

## Red team findings — 26 issues, resolved

A full adversarial review found 5 CRITICAL, 8 HIGH, 8 MEDIUM, and 4 LOW
issues. The VFS stack (`foundation_nativeapis`) provides
production-tested solutions for many of the I/O-layer findings, reducing
the new code we must write and audit. Findings marked **→ VFS** are
solved by the overlay/`NativeFs`/path-utils stack.

### Critical fixes

#### C1 — Path traversal → RESOLVED (VFS stack)

`OverlayFileSystem`'s `OverlayDirectory::resolve_child_path()` already
rejects `..` and absolute paths (line 264 of `overlay_fs.rs`). `NativeFs`
canonicalizes its root at construction. `AssetResolverFs` strips leading
`/` and rejects traversal characters. We add only manifest-level validation:
`app_id` and `file_path` must not contain `..`, `\`, NUL. The VFS handles
the filesystem-level traversal protection.

#### C2 — OTA manifest `base_url` must be a sub-path of the baked domain → RESOLVED

Manifest `base_url` validated before downloads: must start with
`https://{baked_domain}/`. File URLs derived mechanically:
`https://{baked_domain}/bundles/{app_id}/{version}/{file_path}`.
SSRF vectors (`http://169.254.169.254/`, `http://127.0.0.1:8080/`)
fail the prefix check.

#### C3 — Manifest signing → RESOLVED (promoted from "future consideration")

Manifest signing is NOT deferred. It is required from the initial
implementation.

- An Ed25519 public key is baked at compile time alongside the domain
  (via `PlatformBuilder::ota_manifest_domain(domain).ota_manifest_key(pubkey)`)
- The manifest JSON includes a `signature` field: Base64-encoded Ed25519
  signature over the canonical JSON body (all fields EXCEPT `signature`)
- Before any processing, the signature is verified against the baked public key
- Unsigned manifests or manifests with invalid signatures are rejected
- The public key is immutable after APK build (same as the domain)

```json
{
  "version": 1,
  "created_at": "2026-07-23T10:00:00Z",
  "sequence": 42,
  "base_url": "https://cdn.ewe.studio/bundles",
  "rollback_to": null,
  "delete_after": null,
  "signature": "dGhpcyBpcyBhIGZha2Ugc2lnbmF0dXJl...",
  "apps": [...]
}
```

```rust
fn verify_manifest(json: &str, public_key: &[u8; 32]) -> Result<OtaManifest, String> {
    // 1. Parse as generic JSON, extract and remove "signature" field
    // 2. Canonicalize remaining JSON (sorted keys, no whitespace)
    // 3. Verify Ed25519 signature over canonical bytes
    // 4. Deserialize into OtaManifest
    // Fail on any step
}
```

This makes CDN compromise a non-event: the attacker can serve any manifest
they want, but without the private key, the signature won't verify.

#### C4 — Lexical version sort → RESOLVED

Parse versions as semver tuples, compare numerically:

```rust
fn parse_semver(version: &str) -> Option<(u64, u64, u64)> {
    let v = version.strip_prefix('v').unwrap_or(version);
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 3 { return None; }
    Some((parts[0].parse().ok()?, parts[1].parse().ok()?, parts[2].parse().ok()?))
}

fn sort_versions(versions: &mut [(String, PathBuf)]) {
    versions.sort_by(|a, b| {
        let va = parse_semver(&a.0);
        let vb = parse_semver(&b.0);
        va.cmp(&vb)
    });
}
```

`v0.10.0` now correctly sorts after `v0.2.0`.

#### C5 — Concurrency control → RESOLVED (VFS stack + ops_lock)

The VFS stack (`MemoryFs`, `OverlayFileSystem`) uses `Arc<RwLock<>>`
internally. `PlatformAssetManager` adds an `ops_lock: Mutex<()>` to
serialize `activate`, `delete_version`, and `process_manifest`.
Concurrent reads (`read`, `exists`) don't contend with each other.

### High fixes

#### H1 — Incomplete extraction → RESOLVED (no extraction)

With the VFS overlay, there is **no extraction to interrupt**. The base
layer (AssetResolverFs) serves APK assets lazily on demand. Only OTA
writes touch disk, and those use `NativeFs::write_file()` which delegates
to `std::fs::write` — the OS guarantees write atomicity at the page level.

#### H2 — TOCTOU → RESOLVED (no extraction)

No directory `exists()` check needed — the overlay's `read_file()` resolves
from base (APK) or delta (disk) in a single path lookup. No window between
check and use.

#### H3 — `delete_after` validation → RESOLVED

Before deleting, validate:
- `delete_after != active_version`
- `delete_after != rollback_to` (if rollback is happening)
- At least one version directory will remain after deletion

Any violation → the `delete_after` directive is ignored (warning log, not
a fatal error — OTA continues without the delete).

#### H4 — Symlink following → RESOLVED (VFS stack)

`NativeFs` canonicalizes its root on construction, catching symlinks in the
path. `OverlayFileSystem` operates on in-memory state for the base layer
and `NativeFs` for the delta — symlinks in the delta are handled by the OS
via `NativeFs`'s canonicalization. On desktop, the resource dir is
canonicalized at init.

#### H5 — File size limits → RESOLVED

Per-file max 50 MB, per-manifest max 500 files, per-version total
tracked. No startup extraction means no extraction size limit needed.

#### H6 — Atomic OTA writes → RESOLVED (VFS)

`NativeFs::write_file()` uses `std::fs::write` — atomic at page level.
`.part` temp file + rename for multi-page files. Stale `.part` files
cleaned at startup.

#### H7 — Checksum verification → RESOLVED

Manifest requires `sha256` per file. Verified after download before
`write_file()`. AssetResolverFs could optionally compute checksums on
read for defense-in-depth (future).

#### H8 — Manifest replay → RESOLVED by C3 + `sequence` field

Signed manifest includes monotonic `sequence`. Stored in `.ewe_manifest_seq`.
`sequence <= stored` → rejected.

### Medium fixes

| ID | Finding | Resolution |
|----|---------|-----------|
| M1 | Version string not sanitized | Validate: `[0-9a-zA-Z._\-+]+` regex + must parse as valid semver. Reject on failure. |
| M2 | No download resume | Store partial downloads as `*.part` with byte-range tracking. Resume via HTTP Range. |
| M3 | Infinite rollback loop | Track rollback count in `.ewe_rollback_state`. After 3 rollbacks without 30s uptime, lock to current version, ignore manifest `rollback_to`. |
| M4 | No rate limiting on manifest | Cache manifest locally with 5-minute TTL. Exponential backoff (1s, 2s, 4s, ... 60s max) on fetch failure. |
| M5 | `delete_after` nonexistent version | Explicitly defined: no-op, warning log, OTA continues. |
| M6 | No cleanup on partial extraction | N/A — no extraction. OTA writes use atomic temp+rename. |
| M7 | Too many files in manifest | Maximum 500 file entries per manifest. Exceed → rejected. |
| M8 | In-place OTA inconsistency | OTA writes to a new `v{version}.ota{N}/` staging dir. After all files written + verified, `activate()` atomically switches. The prior `v{version}/` becomes the rollback target. |

### Low fixes

| ID | Finding | Resolution |
|----|---------|-----------|
| L1 | Manifest `version` field undocumented | Documented: manifest schema version. App rejects unsupported schema versions. |
| L2 | `list_versions()` exposes history | Not exposed to WebView content. Only available through native API (Rust callers, not JS). |
| L3 | `remove_dir_all` follows symlinks | Resolved by VFS (NativeFs canonicalization). |
| L4 | `v` prefix convention | Internalized: APIs accept bare version, `v` prefixed internally. |

### 9. Tests — in `tests/`, not inline, VFS-backed

Per project standards, public-API tests go in `{crate}/tests/` — never in
`src/` files. Inline tests (`#[cfg(test)] mod tests {}`) are only for
private functions, proc-macro output, and WASM targets that can't use the
test harness.

| File | Tests |
|------|-------|
| `backends/foundation_platform/tests/assets_suite.rs` | **NEW** — `PlatformAssetManager` initialization, extraction idempotency, version listing, activation, pruning, desktop path resolution |
| `backends/foundation_platform/tests/ota_suite.rs` | **NEW** — `OtaManifest` deserialization with `rollback_to`/`delete_after`, domain rejection, version directory writes, in-place vs new-version OTA paths |
| `backends/foundation_nostd/tests/mobile_directory_suite.rs` | **NEW** — `MobileDirectory` trait methods, derive macro output validation |

#### assets_suite.rs test matrix

```
// ── Platform selection ──
initialize_desktop_builds_native_fs
initialize_android_builds_overlay_fs
initialize_android_asset_resolver_fs_is_base_layer
initialize_android_native_fs_is_delta_layer
initialize_ios_builds_native_fs

// ── VFS read/write ──
read_file_from_vfs_returns_bytes
read_file_missing_returns_not_found
write_file_through_vfs_succeeds
write_file_creates_parent_directories
exists_returns_true_for_written_file
exists_returns_false_for_missing

// ── Overlay semantics (Android) ──
overlay_reads_from_base_when_no_delta_entry
overlay_reads_from_delta_when_delta_entry_exists
overlay_delta_shadows_base_entry
overlay_write_goes_to_delta_not_base
overlay_write_then_read_reflects_delta

// ── AssetResolverFs ──
asset_resolver_fs_reads_from_apk_bundle
asset_resolver_fs_exists_from_apk_bundle
asset_resolver_fs_rejects_traversal_in_path
asset_resolver_fs_read_only_returns_error_on_write

// ── Version management ──
list_versions_returns_sorted_semver_dirs
list_versions_handles_multi_digit_components
list_versions_v0_10_after_v0_2
list_versions_empty_when_no_dirs
activate_switches_active_root
activate_errors_on_nonexistent_version
prune_keeps_exact_count
prune_handles_no_dirs_gracefully
prune_semver_sort_not_lexical
version_directory_name_is_v_prefix
bundle_version_from_package_info_is_semver
parse_semver_rejects_invalid
parse_semver_handles_v_prefix

// ── Concurrency ──
concurrent_reads_do_not_panic
concurrent_write_and_prune_do_not_corrupt
ops_lock_serializes_mutations
```

#### ota_suite.rs test matrix

```
manifest_deserializes_rollback_to_field
manifest_deserializes_delete_after_field
manifest_deserializes_sha256_field
manifest_deserializes_sequence_field
manifest_without_optional_fields_is_valid
manifest_domain_mismatch_is_rejected
manifest_domain_match_is_accepted
manifest_base_url_not_subpath_of_baked_domain_rejected
manifest_base_url_http_not_accepted
manifest_base_url_localhost_rejected
manifest_domain_is_compile_time_baked
manifest_signature_valid_is_accepted
manifest_signature_invalid_is_rejected
manifest_signature_missing_when_key_baked_is_rejected
manifest_unsigned_when_no_key_baked_is_accepted
manifest_source_apk_accepted_without_signature
manifest_source_ota_rejected_without_signature_when_key_baked
manifest_signature_key_derived_from_environment_seed
codegen_generates_manifest_per_app
codegen_manifest_contains_sha256_per_file
codegen_manifest_source_is_apk
codegen_manifest_sequence_is_zero
manifest_sequence_equal_to_stored_is_rejected_as_replay
manifest_sequence_less_than_stored_is_rejected_as_replay
manifest_sequence_greater_than_stored_is_accepted
manifest_with_path_traversal_in_file_path_rejected
manifest_with_path_traversal_in_app_id_rejected
manifest_file_count_exceeds_max_is_rejected
manifest_file_size_exceeds_max_is_rejected
ota_writes_to_version_directory_not_flat
ota_stores_manifest_in_version_dir
ota_manifest_written_atomically_with_tmp_rename
ota_manifest_missing_for_apk_bundled_version
ota_manifest_present_for_ota_created_version
ota_prune_deletes_manifest_alongside_version_dir
ota_in_place_writes_to_new_otaN_staging_dir
ota_sha256_mismatch_rejects_file
ota_sha256_match_accepts_file
ota_writes_to_dot_part_then_atomically_renames
ota_rollback_to_activates_target_version
ota_delete_after_removes_specified_version
ota_delete_after_active_version_is_rejected
ota_delete_after_rollback_target_is_rejected
ota_delete_after_only_version_is_rejected
ota_delete_after_nonexistent_version_is_noop
ota_rollback_to_nonexistent_version_is_error
ota_rollback_loop_three_failures_locks_version
package_directorate_manifest_url_is_https
```

#### assets_security_suite.rs test matrix (NEW)

```
// ── Path traversal (in VFS stack) ──
overlay_resolve_child_path_rejects_dot_dot
asset_resolver_fs_rejects_backslash_in_path
asset_resolver_fs_rejects_nul_in_path
vfs_write_rejects_absolute_path
vfs_write_rejects_windows_unc_path

// ── Version sanitization ──
version_string_with_slash_rejected
version_string_with_null_rejected
version_string_not_valid_semver_rejected

// ── Manifest security ──
manifest_with_internal_network_url_base_rejected
manifest_base_url_http_rejected
manifest_base_url_localhost_rejected

// ── Replay protection ──
manifest_sequence_equal_rejected
manifest_sequence_less_rejected
manifest_sequence_greater_accepted

// ── VFS audit (behind feature flag) ──
audit_fs_emits_read_event
audit_fs_emits_write_event
audit_fs_emits_exists_event
audit_fs_no_overhead_when_feature_disabled
```


### 6. Android example cleanup
- Remove `embedded_apps.rs` if it exists
- Restore standard `MobileDirectory` responders
- No per-app workarounds needed

### 7. iOS verified as working
- iOS `resource_dir()` returns `{exe_dir}/assets` — a real path
- `initialize()` on iOS is a no-op (active_root = resource_dir())
- Verify with `#[cfg(target_os = "ios")]` guard

### 8. Desktop unaffected
- Desktop `resource_dir()` returns real path
- `initialize()` on desktop is a no-op
- All existing behavior preserved

### 11. Key directory and auto-generation

Every ewe workspace has a canonical key directory: **`keys/`** at the
workspace root (alongside `Cargo.toml`). The codegen auto-generates the
key pair on first use so the developer doesn't need to find or generate
one manually.

#### Directory layout

```
keys/
  ota_public.key     — Ed25519 public key (32 bytes, base64). Checked into git.
  ota_private.key    — Ed25519 private key seed (32 bytes, base64). .gitignore'd.
  .gitignore          — Generated: contains "ota_private.key"
```

`ota_public.key` is **committed to git** — it's baked into every APK build.
`ota_private.key` is **never committed** — the codegen adds it to `.gitignore`.
In CI/CD, the private key comes from secrets (`EWE_OTA_PRIVATE_KEY` env var).

#### Auto-generation (codegen)

When `cargo tauri build` runs the codegen step:

1. Check if `keys/ota_public.key` exists
2. If it does → read it, bake into the binary
3. If it doesn't → generate a new Ed25519 key pair:
   - Generate 32 random bytes via `getrandom`
   - Derive the public key (ed25519-dalek)
   - Write `keys/ota_public.key` (base64-encoded public key)
   - Write `keys/ota_private.key` (base64-encoded seed)
   - Create `keys/.gitignore` with `ota_private.key`
   - Print: `[ewe] Generated OTA key pair in keys/. Replace ota_private.key with your production key before deploying.`
4. The public key is included via `include_str!("../../keys/ota_public.key")` in the generated app code

The developer replaces the auto-generated `ota_private.key` with their
production key when they're ready to deploy. The public key rotates
automatically — just replace both files and rebuild.

#### CLI: `cargo run -p foundation_platform -- init-keys`

A CLI binary in `foundation_platform` (behind `#[cfg(not(wasm32))]`) that
scaffolds the key directory and the workspace structure for new ewe projects:

```
$ cargo run -p foundation_platform -- init-keys [--workspace /path/to/project]

  [ewe] Checking keys/ ...
  [ewe] Generating Ed25519 key pair ...
  [ewe] Created: keys/ota_public.key
  [ewe] Created: keys/ota_private.key
  [ewe] Created: keys/.gitignore
  [ewe] Done. Replace keys/ota_private.key with your production key before deploying.
```

Also provides subcommands:

```
$ cargo run -p foundation_platform -- key-info
  Public key:  abc123... (32 bytes)
  Derived from private key: yes
  Private key location: keys/ota_private.key

$ cargo run -p foundation_platform -- sign-manifest keys/ota_private.key manifest.json
  Signature: iG18bBqR... (base64)

$ cargo run -p foundation_platform -- verify-manifest keys/ota_public.key manifest.json
  Signature: VALID
  Manifest sequence: 42
  Domain: cdn.ewe.studio
```

This CLI lives in `backends/foundation_platform/src/cli.rs`, target-gated
with `#[cfg(not(wasm32))]`. The `main()` is in `src/main.rs` (also gated).

#### PlatformBuilder API

```rust
// In codegen output (not hand-written):
platform_run!(PlatformBuilder::new()
    .inject_platform_runtimes()
    .ota_manifest_domain("cdn.ewe.studio")
    .ota_manifest_key_from_file(concat!(env!("CARGO_MANIFEST_DIR"), "/keys/ota_public.key"))
    //     ↑ reads the file at compile time, bakes the bytes
    .setup(setup_routes));
```

`ota_manifest_key_from_file(path)` is a `const fn`-compatible builder method
that uses `include_str!` internally to bake the key at compile time.

### 12. Manifest key: public key in keys/, private key in CI/CD secrets

The private key lives in CI/CD, not the source tree:

```
CI/CD pipeline:
  1. Loads EWE_OTA_PRIVATE_KEY from secrets (base64-encoded 32-byte seed)
  2. Derives public key from seed
  3. Passes public key to PlatformBuilder::ota_manifest_key(pubkey)
     → baked into the binary, immutable after APK build
  4. Signs manifests with the private key during CDN deployment
     → app verifies with the baked public key
```

The private key **never** ships in the APK. The public key comes from
`keys/ota_public.key` (committed, or overridden by CI). In local dev,
the codegen auto-generated private key works — manifests signed with it
will verify against the matching auto-generated public key.

When moving to production:
1. Replace `keys/ota_public.key` with the production public key
2. Set `EWE_OTA_PRIVATE_KEY` in CI/CD secrets with the production private key
3. All APK builds from that point embed the production public key
4. All CDN manifests are signed with the production private key

### 13. Codegen manifest generation (APK-bundled .ewe_manifest.json)
- The build pipeline (`cargo tauri build`) generates a `.ewe_manifest.json` per
  app as part of the codegen step (F11/F19)
- Computes `sha256` + `size` for every file in `public/{app}/`
- Writes manifest to `public/{app}/.ewe_manifest.json`
- `source: "apk"`, `sequence: 0`, `signature: null`
- Included in `bundle.resources` in `tauri.conf.json` (or picked up by the
  glob automatically)
- Extracted alongside the app files by `initialize()` into the version directory
- Enables `initialize()` to verify APK bundle integrity (compare manifest
  hashes against extracted files)

### 14. Version lifecycle
- APK version = bundle version (from `tauri.conf.json` → `app.package_info().version`)
- New APK → new version dir extracted, old versions pruned (keep 2)
- OTA same version → writes to existing dir, no new directory
- OTA new version → creates new dir, activates it, prunes oldest
- Rollback → `activate(previous_version)`, instant (files are still there)

## Platform behavior matrix

| Method | Desktop | iOS | Android |
|--------|---------|-----|---------|
| `initialize(app, version)` | `NativeFs(resource_dir())` | `NativeFs(resource_dir())` | `OverlayFileSystem(AssetResolverFs, NativeFs(app_data/v{version}/))` + prune |
| `active_root()` | `resource_dir()` | `resource_dir()` | `app_data/v{version}/` |
| `read(path)` | `fs.read_file(path)` → NativeFs | same | Overlay: delta first, then base (APK) |
| `write(path, data)` | `fs.write_file(path, data)` → NativeFs | same | `fs.write_file(path, data)` → delta layer (NativeFs) |
| `exists(path)` | `fs.exists(path)` → NativeFs | same | Overlay: delta first, then base |
| `list_versions()` | empty vec | empty vec | Sorted semver dirs in app_data |
| `activate(version)` | no-op | no-op | Rebuild overlay with new delta dir, set active_root |
| `prune(keep: 2)` | no-op | no-op | Delete oldest version dirs |
| Startup extraction | None | None | None — APK served lazily via AssetResolverFs |
| Audit logging | `cfg(feature = "vfs-audit")` | same | same — wraps in ObservableFs |

## Verification

```bash
# Unit/Integration tests (in tests/ directory)
cargo test -p foundation_platform -- assets_suite
cargo test -p foundation_platform -- ota_suite

# Desktop — unaffected
cargo test -p foundation_platform -- assets
cargo test -p foundation_platform -- mobile

# Android — APK with versioned extraction
cd examples/platform_android
cargo tauri android build --debug --target x86_64
adb install -r gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk

# First launch — version 0.1.0
adb shell monkey -p com.ewe.platform 1
adb logcat -d | grep -E "PlatformAssetManager|platform_dashboard"
# Expected:
# [PlatformAssetManager] extracting v0.1.0: 12 files
# [PlatformAssetManager] active version: v0.1.0
# [platform] platform_dashboard: WASM active

# Second launch — v0.1.0 already exists
adb shell am force-stop com.ewe.platform
adb shell monkey -p com.ewe.platform 1
# Expected:
# [PlatformAssetManager] v0.1.0 already extracted, skipping
# [PlatformAssetManager] active version: v0.1.0

# OTA in-place — overwrite a file in v0.1.0
adb shell "echo 'console.log(\"OTA v2\")' > /data/data/com.ewe.platform/v0.1.0/app/platform_dashboard.js"
adb shell am force-stop com.ewe.platform
adb shell monkey -p com.ewe.platform 1
# Expected: v0.1.0 still active, platform_dashboard.js is the OTA version

# OTA new version — create v0.1.1
adb shell "mkdir -p /data/data/com.ewe.platform/v0.1.1/app"
adb shell "echo '<html>OTA 0.1.1</html>' > /data/data/com.ewe.platform/v0.1.1/app/index.html"
# App calls manager.activate("v0.1.1")
# active_root now = v0.1.1/
# v0.1.0 still on disk (rollback target)

# APK upgrade to 0.1.1 — v0.1.1/ already exists from OTA
adb install -r app-v0.1.1.apk
adb shell monkey -p com.ewe.platform 1
# Expected:
# [PlatformAssetManager] v0.1.1 already extracted, skipping
# (OTA's v0.1.1 files are NOT overwritten by APK extraction)

# Pruning — 3 versions exist
adb shell "mkdir -p /data/data/com.ewe.platform/v0.1.2"
adb shell "ls /data/data/com.ewe.platform/ | grep '^v'"
# Before prune: v0.1.0  v0.1.1  v0.1.2
# After prune:  v0.1.1  v0.1.2  (v0.1.0 deleted)

# Rollback — activate previous version
# App calls manager.activate("v0.1.0") — instant, files already on disk
# Latest version listing shows all 3 versions, active points to v0.1.0

# OTA-forced rollback — manifest contains rollback_to
# Manifest: { "rollback_to": "0.1.0", "delete_after": "0.1.2", "apps": [...] }
# App downloads v0.1.3 files, then: activate("v0.1.0"), delete v0.1.2/
# Result: v0.1.0 (active) + v0.1.1 + v0.1.3 on disk, v0.1.2 deleted
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_platform/src/assets.rs` | **NEW** — `PlatformAssetManager` + `AssetResolverFs` + `ReadOnlyVfsFile`/`ReadOnlyVfsDirectory` (~200 lines) |
| `backends/foundation_platform/Cargo.toml` | Add `foundation_nativeapis = { path = "../foundation_nativeapis", features = ["vfs"] }` |
| `backends/foundation_platform/src/lib.rs` | Add `pub mod assets;` + re-export `PlatformAssetManager` |
| `backends/foundation_platform/src/session.rs` | Add `asset_manager` field, wire through `new()` and `bundle_root()` |
| `backends/foundation_platform/src/builder.rs` | Replace `resolve_resource_root()` with `PlatformAssetManager::initialize()` + `ota_manifest_domain()` + `ota_manifest_key()` |
| `backends/foundation_platform/src/ota.rs` | Add `rollback_to`/`delete_after`/`signature`/`sequence`/`sha256` fields; VFS-backed writes; domain-locked init |
| `backends/foundation_platform/tests/assets_suite.rs` | **NEW** — 30+ tests using `MemoryFs`, covering overlay semantics, version management, concurrency |
| `backends/foundation_platform/tests/ota_suite.rs` | **NEW** — 30+ tests: manifest signing, domain validation, replay detection, rollback, VFS writes |
| `backends/foundation_platform/tests/assets_security_suite.rs` | **NEW** — 20+ tests: path traversal, version sanitization, manifest security, audit events |
| `examples/platform_android/src-tauri/src/lib.rs` | Remove `embedded_apps.rs` workarounds, restore standard setup |
| `backends/foundation_platform/src/codegen.rs` | Generate `.ewe_manifest.json` per app during build pipeline; generate key pair if missing |
| `backends/foundation_platform/src/cli.rs` | **NEW** — `init-keys`, `key-info`, `sign-manifest`, `verify-manifest` (target-gated `#[cfg(not(wasm32))]`) |
| `backends/foundation_platform/src/main.rs` | **NEW** — CLI entrypoint (target-gated) |
| `keys/ota_public.key` | **NEW** — auto-generated Ed25519 public key (committed to git) |
| `keys/ota_private.key` | **NEW** — auto-generated Ed25519 private key seed (.gitignore'd) |
| `keys/.gitignore` | **NEW** — auto-generated, ignores ota_private.key |

## Non-goals

- We are NOT writing a general-purpose virtual filesystem — `foundation_nativeapis` provides that
- We are NOT wrapping every `std::fs` call — the VFS trait is the abstraction boundary
- We are NOT adding `tauri-plugin-fs` as a dependency — Tauri core `AssetResolver` is sufficient
- We are NOT changing the `MobileDirectory` trait or `MobileDisk` extension — they stay filesystem-based for OTA writes
- We are NOT enabling VFS audit in release builds by default — gated behind `cfg(feature = "vfs-audit")`
