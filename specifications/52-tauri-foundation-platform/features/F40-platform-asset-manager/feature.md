---
workspace_name: "ewe_platform"
spec_directory: "specifications/52-tauri-foundation-platform"
feature_directory: "specifications/52-tauri-foundation-platform/features/F40-platform-asset-manager"
this_file: "specifications/52-tauri-foundation-platform/features/F40-platform-asset-manager/feature.md"

status: completed
priority: critical
created: 2026-07-23
updated: 2026-07-23

depends_on:
  - "F22-mobile-directory"
  - "F13-cross-platform-builds"
  - "F21-multi-app-distribution-and-webview"

tasks:
  completed: 13
  uncompleted: 0
  total: 13
  completion_percentage: 100%
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

> **Resolved 2026-07-23 — app-first, not version-first.** Earlier drafts of
> this document carried both layouts: this one, and a version-first
> `resource_root/v{version}/{app_id}/` implied by an `active_root()` accessor.
> They are not reconcilable. Version-first gives one lifecycle for the whole
> bundle, which collapses every per-app scenario below — independent OTA,
> independent rollback, independent pruning, and the multi-app manifest where
> `app` ships 0.1.2 while `app-settings` ships 0.2.1. App-first is the design;
> `active_root()` is gone, replaced by `app_root(app_id)`.

**APK-bundled manifest**: generated by the build pipeline at compile time
(requirement 12). Contains the app's `bundle_version`, a list of all bundled
files with their `sha256` hashes and `size`, `source: "apk"`, and a
signature — the build mints a key pair if none exists, so every manifest is
signed regardless of origin (see "Every manifest is signed" below). It ships
inside the bundle, in the app's directory.

**OTA manifest**: the signed manifest received from the CDN, written
atomically as the final OTA step. `source: "ota"`, `sequence` > 0.

`source` is what distinguishes the two, not the presence of a signature.

This uniform structure means every tool, script, or rollback decision can
rely on `.ewe_manifest.json` being present *and verifiable* — never a
"maybe it's there", never a "maybe it's signed".

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

        let manager = PlatformAssetManager::initialize(&app, &bundle_version, key);
        // Desktop/iOS: NativeFs(resource_dir()), AssetLayout::Flat.
        // Android: OverlayFileSystem(AssetResolverFs, DirectoryDelta(app_data)),
        //          AssetLayout::Versioned.

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
pipeline at compile time (requirement 12). One per app, written into the
version directory. Contains the exact files bundled with their `sha256` and
`size`.

#### Every manifest is signed. There is no unsigned mode.

An earlier draft let `signature` be `null` for APK-bundled manifests, on the
grounds that the platform's own code signature is the trust root there, and
let a device with no baked public key accept whatever it was handed.

Both were wrong, for the same reason: **the build mints a key pair
automatically.** `manifest::ensure_keys` runs on every build and generates
an Ed25519 pair if `keys/ota_public.key` is absent. Signing is therefore
always available, so "unsigned" never describes a legitimate situation — only
a broken build environment or an attacker. Defining behaviour for it just
creates a path that skips the one check that establishes authorship.

So:

- `generate_app_manifest` **fails** with `NoPrivateKey` rather than emitting
  an unsigned manifest, and writes nothing when it fails — a half-written
  manifest is worse than none, because tooling treats presence as a promise.
- `codegen` **fails the build** if signing is unavailable. A bundle whose
  manifests cannot be verified is not shippable, so producing it quietly
  would only move the failure to the device.
- `process_manifest` **refuses outright** when no public key is baked in,
  with "OTA is disabled". It does not fall back to accepting unsigned
  manifests: a build that cannot answer *who wrote this* has no business
  installing anyone's code.
- `process_manifest` also rejects `source: "apk"` — a bundled manifest
  describes what already shipped, and accepting one as an update would let a
  replayed build artefact stand in for a release.

`signature` stays `Option<String>` on the wire type so a manifest that omits
it still *parses* and can be rejected with a reason. A parse failure would
report only "bad JSON", which is a worse answer to an attack.

Fields:
- `source`: `"apk"` or `"ota"` — distinguishes origin; only `"ota"` is
  installable
- `manifest_domain`: the baked domain (must match)
- `signature`: Ed25519 signature over canonical JSON — always present
- `sequence`: monotonic integer (OTA only; APK uses 0)
- `rollback_to` (optional): semver version to activate after download
- `delete_after` (optional): semver version to delete after rollback

### Chain of trust: two different things working together

The sha256 hash and the Ed25519 signature serve different purposes. Neither is
sufficient alone. Together they form a complete chain of trust.

**sha256 guarantees integrity, NOT authenticity.** Anyone can compute
`sha256("malware.js")` and put that hash in a manifest. An attacker who
compromises the CDN can serve any file they want with a matching hash that
*they* computed. The hash only proves the bytes haven't changed since
*someone* declared them — it doesn't prove *who* declared them or that
the declaration is trustworthy.

**Ed25519 signature guarantees authenticity of the declaration.** Only the
holder of the private key can produce a valid signature. The app verifies
this signature against the baked public key. If valid: the sha256 values,
file paths, versions, and all other manifest fields were declared by us —
not by an attacker.

**Together**: the signature proves *we* declared these sha256 values. The
sha256 verification proves the downloaded bytes match *our* declaration.
Neither step can be skipped.

```
Security model for each actor:

┌─ CDN (https://cdn.ewe.studio) ─────────────────────────────────────┐
│  The CDN is NOT trusted. It can serve whatever it wants.            │
│  The baked domain is a ROUTING guard, not a TRUST anchor:           │
│    - Prevents SSRF: no fetching from http://169.254.169.254/       │
│    - Prevents exfiltration: no fetching from evil-cdn.example.com  │
│    - Does NOT authenticate content: that's the signature's job     │
└────────────────────────────────────────────────────────────────────┘

┌─ Private key holder (CI/CD pipeline) ──────────────────────────────┐
│  Signs the manifest. The only entity that CAN sign.                │
│  If an attacker gets this key, they can sign ANY manifest.         │
│  Defense: private key in CI/CD secrets, never in source tree,      │
│           never in APK, never on any device.                        │
└────────────────────────────────────────────────────────────────────┘

┌─ Attacker with CDN access (compromised infra) ─────────────────────┐
│  Can serve:                                                         │
│    - Modified files → sha256 mismatch → REJECTED                   │
│    - Modified manifest → signature invalid → REJECTED              │
│    - Old valid manifest → sequence check → REJECTED                │
│    - Manifest signed by attacker → signature invalid (wrong key)   │
│                                                                     │
│  Cannot: forge a valid manifest (needs private key)                │
│          forge valid file hashes (SHA-256 preimage resistance)     │
│          replay an old manifest (sequence check)                    │
└────────────────────────────────────────────────────────────────────┘

┌─ Attacker with MITM position (network intercept) ──────────────────┐
│  Can intercept downloads. Same constraints as CDN attacker:        │
│    - TLS to baked domain provides transport encryption             │
│    - Signature + sha256 provide end-to-end content authentication  │
│    - Even if TLS is broken: sha256 verification catches tampering  │
└────────────────────────────────────────────────────────────────────┘
```

**The trust anchor is the baked public key.** The CDN, TLS, and domain
validation are defense-in-depth. The signature is the only thing that
authenticates content. The sha256 is the only thing that ties downloaded
bytes to the signed declaration.

**Per-file signatures would add nothing.** They'd be computed with the
same private key and verified against the same public key. A CDN
attacker who can't forge a manifest signature also can't forge a
per-file signature. The manifest-signature + per-file-hash pattern is
the industry standard: APK signing (v1/v2/v3), Docker Content Trust
(Notary/TUF), Python PEP 458/480, apt/deb repositories.

### Idempotency

Versioned directories make initialization trivially idempotent: for each
app, `mkdir -p {app_id}/v{bundle_version}/` on the delta layer and stop.
The directory is the *delta* for that version — empty until an OTA writes
into it, because the APK's own copy is served lazily from the base layer.
One `mkdir_all` per app, no per-file logic, nothing to leave half-done.
OTA creates version directories that cannot collide with APK-declared ones,
because a collision would mean the same version shipping different bytes.

### Pruning policy

Pruning runs **per app** — each app has its own version lifecycle, so a
global sweep would delete a rollback target that a different app still
depends on. Versions are compared as semver tuples, never lexically
(`v0.10.0` is newer than `v0.2.0`), and the active version is never a
deletion candidate no matter how old it is.

```rust
/// Keep the newest `keep` versions of ONE app. Runs through the VFS, so it
/// works against the Android delta layer as well as a real directory.
fn prune(&self, app_id: &str, keep: usize) -> Result<(), String> {
    let _guard = self.ops_lock.lock();
    let active = self.active_version(app_id);

    // Already semver-sorted, oldest first — see `list_versions`.
    let versions = self.list_versions(app_id);
    let mut remaining = versions.len();

    // Deleting the active version is never a pruning decision, so it is
    // filtered out of the candidates but still counts toward `keep`.
    let mut candidates = versions.iter().filter(|v| **v != active);

    while remaining > keep {
        let Some(victim) = candidates.next() else { break };
        // remove_all takes the whole version dir, manifest included —
        // no orphaned .ewe_manifest.json can survive its version.
        self.fs.read().remove_all(&self.version_path(app_id, victim))?;
        remaining -= 1;
    }
    Ok(())
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
  ├─ Android:  OverlayFileSystem<AssetResolverFs, DirectoryDelta>
  │   ┌─ Base layer: AssetResolverFs — lazy reads from APK AssetResolver
  │   │   ┌─ read_file("/app/index.html") → resolver.get("app/index.html")
  │   │   ├─ exists("/app/index.html")     → resolver.get().is_some()
  │   │   └─ Zero RAM preload. Zero startup extraction. On-demand.
  │   │
  │   └─ Delta layer: DirectoryDelta(app_data) — NativeFs + whiteouts
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

**Why `DirectoryDelta` and not `NativeFs` for the delta layer**:
`OverlayFileSystem<B, D>` requires `D: DeltaStore` — whiteout tracking is
what lets a delete in the delta shadow a file that still exists in the APK
base. `NativeFs` is a plain `VfsFileSystem` with no whiteout support.
`DirectoryDelta` *is* `NativeFs` plus `.wh.`-sentinel whiteouts on the same
real directory, so we get the AppData write path and the overlay semantics
from one type.

### Key invariant

After `PlatformAssetManager::initialize(app, bundle_version)`, all reads and
writes go through a `DynFs` — `foundation_nativeapis`'s type-erased VFS
handle. (`Box<dyn VfsFileSystem>` is not object-safe: the trait carries
`File`, `SeekableFile`, and `Directory` associated types. `DynFs` is the
erasure the VFS stack already provides for exactly this case, and it is
`Clone` + `Send + Sync`, which the manager needs to swap backends on
`activate()`.)

The VFS backend is chosen at init time based on the platform:

| Platform | Backend | Base reads from | Writes go to |
|----------|---------|----------------|--------------|
| Desktop | `NativeFs` | `resource_dir()` (disk) | `resource_dir()` (disk) |
| iOS | `NativeFs` | `{exe_dir}/assets` (disk) | `{exe_dir}/assets` (disk) |
| Android | `OverlayFileSystem` | APK `AssetResolver` (lazy) | `AppData/` (disk) |

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

Every path the manager takes is **VFS-relative to `base_root`**, e.g.
`/app/v0.1.0/index.html`. The manager never hands out `std::fs` paths for
I/O — `app_root()` exists only so `MobileDirectory` responders, which read
through `std::fs`, can be mounted at the right real directory.

```rust
/// Cross-platform bundle resource manager backed by the VFS stack.
///
/// On desktop/iOS: wraps NativeFs(resource_dir()).
/// On Android: wraps OverlayFileSystem<AssetResolverFs, DirectoryDelta>.
///
/// Opt-in audit logging via feature flag `vfs-audit` — wraps the
/// VFS in ObservableFs at init time (debug/default) or omitted (release).
pub struct PlatformAssetManager {
    /// The VFS backend. All I/O goes through this — never std::fs directly.
    /// Built once and never swapped: version directories live *inside* the
    /// VFS namespace (`/{app_id}/v{version}/…`), so changing the active
    /// version is a lookup change, not a remount.
    fs: DynFs,
    /// Directory containing the per-app directories. Never changes.
    base_root: PathBuf,
    /// The bundle version declared by the running binary (semver).
    /// Comes from `app.package_info().version`.
    bundle_version: String,
    /// Per-app active version. Keyed by `app_id`, value is a bare semver
    /// string. Defaults to `bundle_version`; `activate()` overrides one app.
    active: RwLock<HashMap<String, String>>,
    /// Flat (desktop/iOS: no version dirs) or Versioned (Android).
    layout: AssetLayout,
    /// Baked CDN domain. `None` disables OTA — it is not a permissive mode.
    manifest_domain: Option<String>,
    /// Baked Ed25519 public key. `None` disables OTA for the same reason.
    manifest_key: Option<[u8; 32]>,
    /// Last accepted manifest sequence number (anti-replay). Persisted to
    /// `base_root/.ewe_manifest_seq` — replay protection that resets on
    /// restart is not replay protection.
    last_manifest_seq: RwLock<u64>,
    /// Ops lock — serializes activate, prune, and manifest processing.
    ops_lock: Mutex<()>,
}

/// Whether this platform uses per-app version directories.
pub enum AssetLayout {
    /// Desktop / iOS. `app_root(id)` = `base_root/{id}`, no version dirs,
    /// `activate` and `prune` are no-ops.
    Flat,
    /// Android. `app_root(id)` = `base_root/{id}/v{version}`.
    Versioned,
}

impl PlatformAssetManager {
    /// Initialize the asset manager.
    ///
    /// `bundle_version` comes from `app.package_info().version` —
    /// semver from `tauri.conf.json`, always increasing.
    ///
    /// Backend selection per platform:
    /// - Desktop/iOS: `NativeFs::new(resource_dir())`, `AssetLayout::Flat`
    /// - Android: `OverlayFileSystem::new(AssetResolverFs, DirectoryDelta(app_data))`,
    ///   `AssetLayout::Versioned`
    ///
    /// On Android the per-app version directory `app_data/{app_id}/v{version}/`
    /// is created if absent (first launch or new version). OTA writes land
    /// there. No extraction — APK assets are served lazily by AssetResolverFs.
    ///
    /// Audit: under `cfg(feature = "vfs-audit")`, wraps the VFS in
    /// ObservableFs for event logging. Off in release builds.
    pub fn initialize<R: Runtime>(
        app: &App<R>,
        bundle_version: &str,
        manifest_key: Option<[u8; 32]>,
    ) -> Self;

    /// The real filesystem directory an app's assets are served from.
    /// `MobileDirectory` responders pass this to their `build(root)`.
    /// Versioned: `base_root/{app_id}/v{active_version}`.
    /// Flat: `base_root/{app_id}`.
    pub fn app_root(&self, app_id: &str) -> PathBuf;

    /// The base directory containing all per-app directories.
    /// This is what `PlatformSession::bundle_root()` returns.
    pub fn base_root(&self) -> &Path;

    /// The bundle version declared by the running binary.
    pub fn bundle_version(&self) -> &str;

    /// The active version for one app (its own lifecycle, not the binary's).
    pub fn active_version(&self, app_id: &str) -> String;

    /// Read a file through the VFS. Path is relative to `base_root`.
    /// On Android: checks delta (OTA) first, then base (APK) — lazy, no extraction.
    pub fn read(&self, path: &str) -> VfsResult<Vec<u8>>;

    /// Write a file through the VFS. Path is relative to `base_root`.
    /// On Android: writes to the delta layer (AppData). Creates parent dirs.
    /// Path traversal and symlink following rejected by the VFS stack.
    pub fn write(&self, path: &str, data: &[u8]) -> VfsResult<()>;

    /// Check if a file exists through the VFS.
    pub fn exists(&self, path: &str) -> VfsResult<bool>;

    /// Every app directory under `base_root`, sorted.
    pub fn list_apps(&self) -> Vec<String>;

    /// Version directories for ONE app (sorted by semver, newest last).
    /// Returns bare version strings (no `v` prefix).
    /// Flat layout: returns an empty vec (single canonical dir).
    pub fn list_versions(&self, app_id: &str) -> Vec<String>;

    /// Activate a different version for ONE app (OTA, rollback).
    /// Versioned: the version directory must exist; `app_root(app_id)` moves.
    /// Flat: no-op.
    pub fn activate(&self, app_id: &str, version: &str) -> Result<(), String>;

    /// Delete one app's version directory. Validates:
    /// - Not the app's active version
    /// - Not the rollback target
    /// - At least one version remains for that app
    /// No-op (warning log) if the version doesn't exist.
    pub fn delete_version(&self, app_id: &str, version: &str,
        rollback_target: Option<&str>) -> Result<(), String>;

    /// Keep the newest `keep` versions of one app, delete the rest.
    /// Never deletes the active version regardless of age.
    pub fn prune(&self, app_id: &str, keep: usize) -> Result<(), String>;

    /// Verify and process an OTA manifest.
    /// 1. Require a baked public key — no key means no OTA, never "trust it"
    /// 2. Verify the Ed25519 signature; reject unsigned and `source: "apk"`
    /// 3. Validate manifest_domain + base_url against the baked domain
    /// 4. Anti-replay: sequence must be strictly greater than last accepted
    /// 5. Validate app_id / version / file paths (no `..`, `\`, NUL, absolute)
    /// 6. Enforce file size / count limits, and that each sha256 is 64 hex
    /// 7. Return the download plan with mechanically derived URLs
    pub fn process_manifest(&self, manifest_json: &str) -> Result<OtaPlan, String>;

    /// Record a sequence as accepted, in memory and on disk. Called only
    /// after every download in the plan succeeded, so a partial run stays
    /// retryable.
    pub fn commit_manifest_sequence(&self, sequence: u64) -> Result<(), String>;

    /// Write a verified manifest into a version directory as provenance,
    /// staged through `.tmp` and renamed.
    pub fn write_version_manifest(&self, app_id: &str, version: &str,
        manifest: &Manifest) -> Result<(), String>;

    /// Build a manager with an explicit VFS backend and layout.
    /// This is the constructor the test suites use — `MemoryFs`, or
    /// `OverlayFileSystem<AssetResolverFs, MemoryDelta>` for the Android
    /// arrangement, gives full coverage on the host with no device and no
    /// Tauri `App`.
    pub fn from_vfs(fs: DynFs, base_root: PathBuf, bundle_version: &str,
        layout: AssetLayout, manifest_domain: Option<String>,
        manifest_key: Option<[u8; 32]>) -> Self;
}
```

### Integration plan

1. **`assets.rs` (NEW)**: `PlatformAssetManager` + `AssetResolverFs` +
   `ReadOnlyVfsFile`/`ReadOnlyVfsDirectory`. ~200 lines total.
   Depends on `foundation_nativeapis` (vfs feature) + `tauri`.

2. **`session.rs`**: Add `asset_manager: RwLock<Option<Arc<PlatformAssetManager>>>`.
   `bundle_root()` delegates to `asset_manager.base_root()` when set, falling
   back to `resource_root` (which every existing test constructs directly).
   `resource_root` field stays for backward compat.

3. **`builder.rs`**: Replace `resolve_resource_root()` with
   `PlatformAssetManager::initialize(&app, &bundle_version, manifest_key)`.
   `bundle_version` from `app.package_info().version.to_string()`.
   Add `ota_manifest_domain(domain) -> Self` and `ota_manifest_key(key) -> Self`.

4. **`responder.rs`**: One **additive** change — `MobileApp::mounted_at(assets,
   prefix)`. `serve_response()` derives its target from the request path, which
   includes the app segment (`/app/index.html` → `app/index.html`). With
   app-first version directories the responder's root already *is* the app
   directory (`base_root/app/v0.1.0/`), so the leading segment must be stripped
   or every lookup doubles it. `MobileApp::new()` keeps the old
   no-prefix behaviour; `MobileDirectory` and `MobileDisk` are untouched.

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
        //    Desktop: NativeFs::new(resource_dir())              → Flat
        //    Android: OverlayFileSystem::new(                    → Versioned
        //        AssetResolverFs::new(app.asset_resolver()),   // APK → lazy base
        //        DirectoryDelta::new(app_data),                // OTA → delta
        //    )
        //    Audit: cfg(feature = "vfs-audit") → ObservableFs::new(fs)
        let manager = PlatformAssetManager::initialize(&app, &bundle_version, manifest_key);

        // 3. Create session rooted at base_root — the directory that holds
        //    the per-app directories.
        let session = PlatformSession::new(manager.base_root().to_path_buf(), injector);
        session.set_asset_manager(manager);

        // 4. MobileDirectory responders mount at their app's version dir:
        //      MobileApp::mounted_at(AppAssets { root: mgr.app_root("app") }, "app")
        //    - Desktop/iOS: base_root/app/            (Flat)
        //    - Android:     base_root/app/v0.1.0/     (Versioned)
        // 5. OTA writes go to the delta layer (AppData via DirectoryDelta)
      })
```

#### Where the active version comes from at startup

The binary's `bundle_version` is the default active version for **every**
app — a freshly installed APK serves what it shipped with. `activate()` is a
runtime action within a session (OTA finishing a download, a user hitting
rollback), not a persisted preference: an APK upgrade must not be silently
overridden by whatever an earlier OTA left on disk.

Two pieces of state *are* persisted, because they are worthless otherwise:

- `base_root/.ewe_manifest_seq` — the last accepted manifest sequence.
  Anti-replay that resets on restart does not protect against replay.
- `base_root/.ewe_rollback_state` — the rollback loop breaker (M3). Counting
  rollbacks only within one session would never reach the 3-strike limit.

### Android VFS overlay details

The overlay pattern replaces extraction entirely. On Android init:

```rust
#[cfg(target_os = "android")]
fn build_android_vfs<R: Runtime>(app: &App<R>, delta_root: &Path) -> VfsResult<DynFs> {
    use foundation_nativeapis::native::vfs::dir_delta::DirectoryDelta;
    use foundation_nativeapis::shared::vfs::{DynFs, OverlayFileSystem};

    // Base: APK assets via AssetResolver — lazy, no extraction.
    let base = AssetResolverFs::new(app.asset_resolver());

    // Delta: AppData + whiteout tracking (OTA writes, rollback support).
    // DirectoryDelta creates the directory if missing and is a DeltaStore,
    // which is what OverlayFileSystem's second type parameter requires.
    let delta = DirectoryDelta::new(delta_root)?;

    // Combine: base + delta with CoW and whiteout semantics.
    let overlay = OverlayFileSystem::new(base, delta);

    // Audit: only when the feature is enabled.
    #[cfg(feature = "vfs-audit")]
    { Ok(DynFs::new(Arc::new(ObservableFs::new(overlay)))) }
    #[cfg(not(feature = "vfs-audit"))]
    { Ok(DynFs::new(Arc::new(overlay))) }
}
```

The delta root is `app_data` itself, not a version subdirectory: version
directories live *inside* the VFS namespace (`/{app_id}/v{version}/...`) so
that one overlay covers every app and every version. A per-version delta root
would need one overlay per app per version and could not express a whiteout
that spans them.

**No files are extracted at startup.** The overlay reads from the APK base
on demand. Only when an OTA update writes a file does anything land on disk
in the delta layer.

## Requirements

### 1. `PlatformAssetManager` struct (NEW)
- File: `backends/foundation_platform/src/assets.rs`
- Fields: `fs: DynFs`, `base_root`, `bundle_version`,
  `active: RwLock<HashMap<String, String>>`, `layout`, `manifest_domain`,
  `manifest_key`, `last_manifest_seq`, `ops_lock`
- `initialize(app, bundle_version, manifest_key) -> Self`: builds platform VFS backend
  - Desktop/iOS: `NativeFs::new(resource_dir())`, `AssetLayout::Flat`
  - Android: `OverlayFileSystem::new(AssetResolverFs::new(app.asset_resolver()), DirectoryDelta::new(app_data))`, `AssetLayout::Versioned`
  - Audit: under `cfg(feature = "vfs-audit")`, wraps in `ObservableFs`
  - Prunes old version directories per app on Android (keep 2)
- `app_root(app_id) -> PathBuf`: the directory MobileDirectory responders mount at
- `base_root() -> &Path`: parent of the per-app directories
- `bundle_version() -> &str`: the version the running binary shipped with
- `active_version(app_id) -> String`: that app's currently active version
- `read(path) -> VfsResult<Vec<u8>>`: delegates to `self.fs.read_file(path)`
- `write(path, data) -> VfsResult<()>`: delegates to `self.fs.write_file(path, data)`
- `exists(path) -> VfsResult<bool>`: delegates to `self.fs.exists(path)`
- `list_apps() -> Vec<String>`: app directories under `base_root`, sorted
- `list_versions(app_id) -> Vec<String>`: that app's version dirs, semver-sorted, newest last
- `activate(app_id, version) -> Result<(), String>`: move one app's active version
- `delete_version(app_id, version, rollback_target)`: validated deletion
- `prune(app_id, keep)`: keep the newest N versions of one app
- `process_manifest(json)`: manifest verification + processing (all red team fixes applied)
- `from_vfs(...)`: test constructor taking an explicit VFS backend + layout
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
- Add `asset_manager: RwLock<Option<Arc<PlatformAssetManager>>>` field
- `set_asset_manager(manager)` — called once by `PlatformBuilder` at setup
- `bundle_root()` returns `asset_manager.base_root()` when set, else `resource_root`
- `resource_root` field stays for backward compat; `new()` is unchanged so the
  existing `new_test(root)` suites keep working
- `asset_manager() -> Option<Arc<PlatformAssetManager>>` accessor

### 4. `builder.rs`
- **Delete** `resolve_resource_root()` — the APK-extraction loop it contained
  is what F40 replaces. Call
  `PlatformAssetManager::initialize(app, &bundle_version, domain, key)` instead
- `bundle_version` from `app.package_info().version.to_string()`
- Add `ota_manifest_domain(domain) -> Self` (compile-time constant, immutable in APK)
- Add `ota_manifest_key(key: [u8; 32]) -> Self` (Ed25519 public key)
- Add `ota_manifest_key_from_str(contents) -> Self` — base64-decodes an
  `include_str!`'d key file; panics on anything that is not 32 bytes, because
  a malformed key would leave the binary with no trust anchor and OTA
  silently off
- `session.set_asset_manager(manager)` runs **before** the setup callbacks, so
  `session.app_root(id)` already resolves version directories while routes
  are being registered

### 5. `responder.rs` — one additive change, `MobileDirectory`/`MobileDisk` untouched

`MobileApp::mounted_at(assets, app_id)` — the responder learns which app it
serves. `MobileApp::new(assets)` keeps the old, unmounted behaviour.

Two problems make this necessary, and the earlier draft of this requirement
("no changes, `std::fs::read()` keeps working") was wrong about both:

1. **Path doubling.** `serve_response()` derives its target from the request
   URL, which includes the app segment (`/app/index.html` → `app/index.html`).
   With app-first version directories the responder's root already *is* the
   app directory, so an unstripped target resolves to
   `base_root/app/v0.1.0/app/index.html` and 404s.

2. **Android APK assets are not on disk.** This is the entire premise of F40.
   `std::fs::read(app_root/index.html)` cannot see a file that lives inside
   the APK — only `AssetResolverFs` can, and only through the VFS. A responder
   that reads the filesystem directly serves nothing on a fresh install, which
   is exactly the bug this feature exists to fix.

So `MobileApp::respond()` reads through the session's asset manager when one
is installed — `respond()` already receives `&PlatformSession`, so no new
plumbing is needed:

```rust
// Resolution order inside MobileApp::respond():
//   1. session.asset_manager() is Some → VFS read of
//      "/{app_id}/v{active_version}/{rel}"  (Versioned)
//      "/{app_id}/{rel}"                    (Flat)
//      → delta (OTA'd, on disk) first, then base (APK), then NotFound
//   2. no manager (tests, embedded use) → assets.read_utf8_for(rel)
//      → plain std::fs against the responder's own root
```

`MobileDirectory` (in `foundation_nostd`) and the `MobileDisk` extension are
not modified — the VFS path is added alongside them, not in place of them.

### 6. `ota.rs` — manifest security + version-aware writes

`OtaManifest` / `OtaAppEntry` / `OtaFileEntry` become aliases of the
`manifest` module's types. One schema describes a bundle; two that could
drift apart is one too many. `LocalVersion` / `.ewe_version.json` is deleted
— `.ewe_manifest.json` supersedes it and says strictly more.

`PackageDirectorate::new(assets)` takes the asset manager and reads the baked
domain off it. The domain is deliberately **not** a parameter: a runtime
argument is precisely the redirection vector that baking it exists to prevent.

- OTA writes through `PlatformAssetManager::write()` (VFS-backed)
- Manifest requires `sha256`, `sequence`, `created_at`, `signature` (Ed25519)
- `base_url` validated against the baked domain; file URLs derived, never read
- `path` / `app_id` / `bundle_version` validated before any fetch
- Anti-replay via `sequence` > stored, and the store is on disk
- Per-file max 50 MB, per-manifest max 500 files
- `rollback_to` + `delete_after` with validation
- Rollback loop breaker: 3 rollbacks without 30s uptime → lock the version,
  state persisted to `.ewe_rollback_state`; `mark_launch_healthy()` clears it

**Ordering in `apply_update`** — not incidental:

1. Every file is fetched, size- and hash-checked, staged as `.part`, then
   renamed. A file that fails verification is never renamed, so a version
   directory never holds bytes we did not authenticate.
2. Each app's `.ewe_manifest.json` is written only after all of that app's
   files landed — the manifest is a claim about a *complete* directory.
3. The sequence watermark is committed **last**. A run that dies partway
   leaves the manifest replayable, so a transient network error does not
   permanently block an update.
4. `rollback_to` / `delete_after` run after installation, so a rollback can
   target a version this very run staged.
5. Pruning runs last and its failure is logged, not fatal — a failed prune
   wastes disk; it does not invalidate an update that is already live.

**Manifest provenance**: the signed manifest JSON is stored as
`.ewe_manifest.json` in the version directory, staged `.tmp` → rename.
Every version directory has one, APK-bundled included (requirement 12
generates those at build time), and it is removed with its version directory
— no orphans.

### 7. Android example cleanup
- `embedded_apps.rs` removed (already done — commit `eea842a16`)
- No per-app workarounds; responders come from codegen
- `setup_routes` mounts each app at `asset_manager().app_root(id)` via
  `MobileApp::mounted_at`, so the version directory is resolved at runtime
  rather than baked into the generated module

### 8. Desktop and iOS unaffected
- Desktop `resource_dir()` returns a real path; iOS returns `{exe_dir}/assets`
- Both select `NativeFs` + `AssetLayout::Flat` in `initialize()`
- `app_root(id)` = `resource_dir()/{id}` — the same path responders already
  resolved, so every existing desktop behaviour is preserved
- `activate()` / `prune()` / `list_versions()` are no-ops returning empty
- Verify the iOS branch compiles under a `#[cfg(target_os = "ios")]` guard

### 9. OTA manifest domain — baked at build time, not a runtime parameter
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

### 10. OTA manifest signing — Ed25519, baked public key

The OTA server (CDN) signs every manifest with an Ed25519 private key.
The corresponding public key (32 bytes) is baked into the APK at compile
time. Only the public key ships — the private key lives in CI/CD secrets
and never touches a device.

See requirement 12 for key management details.

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
| M8 | In-place OTA inconsistency | OTA writes to `{app_id}/v{version}.ota{N}/` staging. After all files are written + verified, `activate(app_id, ..)` switches that app only. The prior `{app_id}/v{version}/` becomes its rollback target; other apps are untouched. |

### Low fixes

| ID | Finding | Resolution |
|----|---------|-----------|
| L1 | Manifest `version` field undocumented | Documented: manifest schema version. App rejects unsupported schema versions. |
| L2 | `list_versions(app_id)` exposes history | Not exposed to WebView content. Only available through native API (Rust callers, not JS). |
| L3 | `remove_dir_all` follows symlinks | Resolved by VFS (NativeFs canonicalization). |
| L4 | `v` prefix convention | Internalized: APIs accept bare version, `v` prefixed internally. |

### 11. Tests — in `tests/`, not inline, VFS-backed

Per project standards, public-API tests go in `{crate}/tests/` — never in
`src/` files. Inline tests (`#[cfg(test)] mod tests {}`) are only for
private functions, proc-macro output, and WASM targets that can't use the
test harness.

| File | Tests |
|------|-------|
| `backends/foundation_platform/tests/assets_suite.rs` | **NEW** — layout selection, VFS I/O, overlay semantics over `AssetResolverFs` + `MemoryDelta`, per-app version management, pruning, concurrency |
| `backends/foundation_platform/tests/ota_suite.rs` | **NEW** — manifest signing, domain validation, replay detection, entry validation, provenance, install verification + atomicity, rollback directives + loop breaker |
| `backends/foundation_platform/tests/assets_security_suite.rs` | **NEW** — path traversal through every entry point, version/app-id sanitization, read-only base integrity, audit events under the feature flag |
| `backends/foundation_platform/tests/manifest_suite.rs` | **NEW** — 15+ tests: key generation, manifest generation, sha256 hashing, signing, verification, build.rs integration |

#### assets_suite.rs test matrix

```
// ── Layout selection ──
flat_layout_app_root_has_no_version_segment
flat_layout_list_versions_is_empty
flat_layout_activate_is_noop
versioned_layout_app_root_has_v_prefix_segment
versioned_layout_app_root_tracks_active_version

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

// ── Version management (per app) ──
list_versions_returns_sorted_semver_dirs
list_versions_handles_multi_digit_components
list_versions_v0_10_after_v0_2
list_versions_empty_when_no_dirs
list_versions_ignores_non_version_entries
list_apps_returns_every_app_directory
activate_switches_app_root_for_that_app_only
activate_errors_on_nonexistent_version
activate_errors_on_invalid_semver
prune_keeps_exact_count
prune_handles_no_dirs_gracefully
prune_semver_sort_not_lexical
prune_never_deletes_active_version
prune_is_per_app_not_global
version_directory_name_is_v_prefix
parse_semver_rejects_invalid
parse_semver_handles_v_prefix
delete_version_rejects_active_version
delete_version_rejects_rollback_target
delete_version_rejects_last_remaining_version
delete_version_missing_is_noop

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
manifest_signed_by_another_key_is_rejected
manifest_unsigned_is_rejected
manifest_tampered_body_is_rejected
manifest_source_apk_is_not_installable_as_an_update
no_baked_key_disables_ota_rather_than_accepting_unsigned
no_baked_domain_disables_ota
manifest_signature_key_derived_from_environment_seed
manifest_sequence_equal_to_stored_is_rejected_as_replay
manifest_sequence_less_than_stored_is_rejected_as_replay
manifest_sequence_greater_than_stored_is_accepted
manifest_with_path_traversal_in_file_path_rejected
manifest_with_path_traversal_in_app_id_rejected
manifest_file_count_exceeds_max_is_rejected
manifest_file_size_exceeds_max_is_rejected
download_urls_are_derived_never_taken_from_the_manifest
a_version_directory_keeps_the_manifest_that_produced_it
the_stored_manifest_still_verifies_against_the_baked_key
no_staging_file_survives_writing_a_manifest
reading_a_manifest_from_a_version_that_has_none_returns_nothing

// ── Install: verification and atomicity ──
// PackageDirectorate derives https://{baked_domain}/… URLs, so an
// end-to-end apply_update would need TLS + a DNS override, or a base-URL
// seam — the exact redirection vector the baked domain closes. Everything
// that touches disk is driven through install_verified() instead.
install_writes_the_file_when_the_hash_matches
install_refuses_bytes_whose_hash_does_not_match
install_refuses_bytes_of_the_wrong_length
a_failed_install_leaves_no_staging_file_to_be_mistaken_for_content
a_successful_install_renames_its_staging_file_away
install_overwrites_a_stale_staging_file_from_an_interrupted_run

// ── Directives: rollback and the loop breaker ──
rollback_to_activates_the_named_version
rollback_to_a_version_we_never_shipped_leaves_the_update_intact
delete_after_removes_the_bad_release_but_never_the_rollback_target
delete_after_naming_the_rollback_target_is_refused_without_failing_the_update
three_rollbacks_without_a_healthy_launch_lock_the_version
marking_a_launch_healthy_clears_the_strike_count
the_rollback_strike_count_survives_a_restart
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

#### manifest_suite.rs test matrix (NEW)

```
// ── Key generation ──
ensure_keys_creates_public_key_on_first_call
ensure_keys_creates_private_key_on_first_call
ensure_keys_creates_gitignore_on_first_call
ensure_keys_is_idempotent_does_not_regenerate
ensure_keys_reads_existing_keys_without_modification
ensure_keys_prints_warning_when_private_key_missing
ensure_keys_loads_private_key_from_env_var
ensure_keys_env_var_overrides_file

// ── Manifest generation ──
generate_app_manifest_scans_all_files_recursively
generate_app_manifest_computes_sha256_for_each_file
generate_app_manifest_computes_size_for_each_file
generate_app_manifest_writes_dot_ewe_manifest_json
generate_app_manifest_skips_dot_files_except_manifest
generate_app_manifest_sets_source_to_apk
generate_app_manifest_sets_sequence_to_zero
generate_app_manifest_signs_when_private_key_available
generate_app_manifest_skips_signature_when_no_private_key
generate_all_manifests_processes_every_app_subdirectory
generate_all_manifests_skips_directories_without_index_html

// ── Signing ──
sign_manifest_produces_valid_ed25519_signature
verify_manifest_accepts_valid_signature
verify_manifest_rejects_invalid_signature
verify_manifest_rejects_tampered_json
verify_manifest_rejects_wrong_public_key
sign_then_verify_roundtrip

// ── File hash verification ──
verify_file_hash_matches_declared_sha256
verify_file_hash_rejects_mismatched_bytes
verify_file_hash_returns_false_for_unknown_file
verify_file_hash_returns_false_for_unknown_app

// ── build.rs integration ──
build_rs_calls_ensure_keys_before_manifest_generation
build_rs_calls_generate_all_manifests_with_correct_params
build_rs_uses_cargo_pkg_version_from_env
build_rs_uses_ewe_manifest_domain_from_env_or_default

// ── CLI (integration) ──
cli_init_keys_idempotent
cli_generate_manifests_creates_manifest_files
cli_sign_manifest_outputs_valid_signature
cli_verify_manifest_reports_valid_or_invalid
```


### 12. Manifest machinery: library API, CLI wrapper, and build.rs integration

The manifest generation, signing, and key management functions live as a
**library API** in `foundation_platform` behind `#[cfg(not(wasm32))]`. Both
the CLI and every project's `build.rs` call the same API — the CLI is just a
thin wrapper. This means `cargo build` automatically generates manifests,
hashes, and key pairs without any manual steps.

#### Library API (`backends/foundation_platform/src/manifest.rs`)

```rust
/// Target-gated: available in build scripts and CLI, not in wasm32 or mobile.
#[cfg(not(wasm32))]
pub mod manifest {
    use std::path::Path;

    /// Ensure the OTA key pair exists at `keys/`. If the public key is missing,
    /// generate a fresh Ed25519 key pair, write both files, create .gitignore.
    /// If the public key exists, read it and derive the private key (if present).
    ///
    /// A seed supplied through EWE_OTA_PRIVATE_KEY always wins over the file
    /// and is never written to the working tree — it belongs to CI's secret
    /// store. Warns if no private key is found at all: that build can verify
    /// but not sign, which is fine for `key-info` and fatal for generation.
    pub fn ensure_keys(project_root: &Path) -> Result<KeyPair, ManifestError>;

    /// Generate and SIGN a manifest for an app directory. Scans all files
    /// recursively, computes sha256 and size for each, and writes
    /// `{app_dir}/.ewe_manifest.json`.
    ///
    /// `keypair` is required and must be able to sign — `ensure_keys` mints a
    /// pair when none exists, so an unsigned manifest is never the right
    /// output. Fails with `NoPrivateKey` before doing any work, leaving
    /// nothing behind: a half-written manifest is worse than none, because
    /// tooling treats presence as a promise.
    pub fn generate_app_manifest(
        app_dir: &Path,
        app_id: &str,
        bundle_version: &str,
        manifest_domain: &str,
        keypair: &KeyPair,
    ) -> Result<Manifest, ManifestError>;

    /// Generate manifests for ALL apps in `public/` (one per subdirectory).
    /// Calls `generate_app_manifest` for each directory that contains an
    /// index.html. This is what build.rs calls before the Tauri build.
    pub fn generate_all_manifests(
        public_dir: &Path,
        bundle_version: &str,
        manifest_domain: &str,
        keypair: &KeyPair,
    ) -> Result<Vec<Manifest>, ManifestError>;

    /// Verify a manifest's Ed25519 signature against a public key.
    pub fn verify_manifest(
        manifest_json: &str,
        public_key: &[u8; 32],
    ) -> Result<Manifest, ManifestError>;

    /// Sign a manifest JSON string with an Ed25519 private key seed.
    /// Returns the signature as a base64 string.
    pub fn sign_manifest(
        manifest_json: &str,
        private_key_seed: &[u8; 32],
    ) -> String;

    /// Verify a single file's sha256 against the manifest's declared hash.
    pub fn verify_file_hash(manifest: &Manifest, app_id: &str, path: &str, bytes: &[u8]) -> bool;
}
```

#### `build.rs` integration — automatic, zero-config, nothing to opt into

Projects already call `foundation_platform::codegen::generate_platform_code()`
from `build.rs`, so the key and manifest steps go *inside* it rather than
into a template every project must remember to update:

```rust
// The project's build.rs is unchanged — one line, as before:
fn main() {
    foundation_platform::codegen::generate_platform_code();
}
```

`generate_platform_code()` now, in order:

1. `ensure_keys(CARGO_MANIFEST_DIR)` — **unconditionally, and first**.
   Projects bake the public key with
   `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/keys/ota_public.key"))`,
   so the file must exist by the time the crate itself compiles — whether or
   not this project happens to have any WASM apps. Panics on failure: every
   manifest is signed, so a build that cannot sign cannot produce a shippable
   bundle, and saying so here beats shipping artefacts every device rejects.
2. Builds the WASM app bundles into `public/`.
3. `generate_all_manifests(public/, CARGO_PKG_VERSION, domain, &keypair)` —
   after the bundles exist, so the manifest describes exactly the bytes that
   ship. Panics if signing is unavailable or generation fails.
4. `tauri_build::build()`.

`cargo:rerun-if-env-changed` is emitted for `EWE_MANIFEST_DOMAIN` and
`EWE_OTA_PRIVATE_KEY`, both of which change manifest contents.

On first build: keys are minted, manifests computed. On subsequent builds:
keys are read, never regenerated — regenerating would invalidate every binary
that already baked the old public key.

**Manifest generation is content-idempotent.** `generate_app_manifest` reads
any existing `.ewe_manifest.json` first and leaves it untouched when the file
list, hashes, sizes, app id, version, and domain all match. Manifests are
committed next to the bundle they describe, and `created_at` is wall-clock —
rewriting unconditionally would mint a new timestamp and therefore a new
signature on every build, so an unchanged bundle would still show up as a
diff. Idempotence keys off content only: change one byte of one file and the
manifest is regenerated, because a stale manifest declares hashes that no
longer match and every device would reject the bundle.

#### Variables

| Variable | Where | Purpose |
|----------|-------|---------|
| `EWE_MANIFEST_DOMAIN` | env (build time) | CDN domain; defaults to `"cdn.ewe.studio"` |
| `EWE_OTA_PRIVATE_KEY` | env (CI/CD or local) | Overrides `keys/ota_private.key` |
| `CARGO_PKG_VERSION` | Cargo built-in | Bundle version (from `Cargo.toml`) |

#### Key directory layout

```
keys/
  ota_public.key     — Ed25519 public key (32 bytes, base64). Checked into git.
  ota_private.key    — Ed25519 private key seed (32 bytes, base64). .gitignore'd.
  .gitignore          — Generated: contains "ota_private.key"
```

`ota_public.key` is **committed to git** — it's baked into every APK build.
`ota_private.key` is **never committed** — auto-added to `.gitignore`.
In CI/CD, the private key comes from `EWE_OTA_PRIVATE_KEY` env var.

#### CLI: thin wrapper around the library API

A small CLI binary in `foundation_platform` (target-gated `#[cfg(not(wasm32))]`)
wraps the same library functions. Useful for manual operations and debugging:

```
$ cargo run -p foundation_platform --bin ewe-manifest -- init-keys
  keys/ota_public.key — OK
  keys/ota_private.key — OK
  Keys are ready.

$ cargo run -p foundation_platform --bin ewe-manifest -- key-info
  Public key:  abc123... (32 bytes, base64)
  Private key: keys/ota_private.key (present)

$ cargo run -p foundation_platform --bin ewe-manifest -- sign-manifest manifest.json
  iG18bBqR...
  (uses EWE_OTA_PRIVATE_KEY, else keys/ota_private.key)

$ cargo run -p foundation_platform --bin ewe-manifest -- verify-manifest manifest.json
  Signature:  VALID
  Public key: keys/ota_public.key
  Source:     ota
  Sequence:   42
  Domain:     cdn.ewe.studio

$ cargo run -p foundation_platform --bin ewe-manifest -- generate-manifests public/ --version 0.1.0
  public/app/.ewe_manifest.json — 4 files, signed
  public/app-hello/.ewe_manifest.json — 3 files, signed
```

`verify-manifest` exits non-zero and prints the reason on an invalid
signature — a verification tool that reports success either way verifies
nothing. `generate-manifests` refuses to run without a private key rather
than emitting something unsigned.

The CLI lives in `backends/foundation_platform/src/cli.rs`, entrypoint in
`src/main.rs`, binary name `ewe-manifest`. Both target-gated
`#[cfg(not(target_family = "wasm"))]`.

#### PlatformBuilder: bakes the domain and public key at compile time

```rust
// In the app's lib.rs:
platform_run!(PlatformBuilder::new()
    .inject_platform_runtimes()
    .ota_manifest_domain("cdn.ewe.studio")
    .ota_manifest_key_from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"), "/keys/ota_public.key"
    )))
    //     ↑ include_str! reads the file at compile time; the builder
    //       base64-decodes it and bakes the 32 bytes into the binary.
    //       build.rs has already created it by this point.
    .setup(setup_routes));
```

`ota_manifest_key_from_str` panics on a key that is not exactly 32
base64-decoded bytes. A malformed key would otherwise leave the binary with
no trust anchor and OTA silently disabled, which is a worse failure than
not starting.

### 13. Version lifecycle

**Each app crate carries its own version** — read from `{crate}/Cargo.toml`
by `codegen::crate_version()`, not from `tauri.conf.json` (which is the
*platform*'s version). `app/` at v0.2.0, `app-hello/` at v0.1.0, and
`app-shell/` at v0.3.0 naturally land at different directories:

```
public/
  app/v0.2.0/       ← Surface 2, bundled as "app/v0.2.0/"
  app-hello/v0.1.0/ ← different version, no collision
  app-shell/v0.3.0/ ← Surface 3, bundled in the same shape
```

- The startup active version for each app is the binary's `bundle_version`
  (from `tauri.conf.json` → `PackageInfo::version`) — the default for apps
  that have not received an OTA. `activate()` is a session-scoped override,
  not a persisted preference.
- New APK → delta dirs created per app (empty), old versions pruned per app
  (keep 2)
- OTA same version → writes into that app's existing dir, no new directory
- OTA new version → creates that app's new dir, activates it, prunes old
- Rollback → `activate(app_id, previous_version)`, instant
- Other apps are never touched by any of the above
- **Surface 3 apps use the same bundle path.** They differ only in target
  (wasm32-wasip1) and loader (wasmtime), not in how they are bundled or
  updated. `sync_bundle_resources()` discovers them from `public/` exactly as
  it does every other app.

## Platform behavior matrix

| Method | Desktop | iOS | Android |
|--------|---------|-----|---------|
| `initialize(app, version)` | `NativeFs(resource_dir())`, Flat | same | `OverlayFileSystem(AssetResolverFs, DirectoryDelta(app_data))`, Versioned, + per-app prune |
| `base_root()` | `resource_dir()` | `resource_dir()` | `app_data/` |
| `app_root(id)` | `resource_dir()/{id}` | same | `app_data/{id}/v{active}/` |
| `active_version(id)` | `bundle_version` | same | that app's own active version |
| `read(path)` | `fs.read_file(path)` → NativeFs | same | Overlay: delta first, then base (APK) |
| `write(path, data)` | `fs.write_file(path, data)` → NativeFs | same | `fs.write_file(path, data)` → delta layer (DirectoryDelta) |
| `exists(path)` | `fs.exists(path)` → NativeFs | same | Overlay: delta first, then base |
| `list_apps()` | dirs under `resource_dir()` | same | dirs under `app_data` |
| `list_versions(id)` | empty vec | empty vec | that app's semver-sorted version dirs |
| `activate(id, version)` | no-op | no-op | moves one app's active version |
| `prune(id, keep: 2)` | no-op | no-op | deletes that app's oldest version dirs |
| Startup extraction | None | None | None — APK served lazily via AssetResolverFs |
| Audit logging | `cfg(feature = "vfs-audit")` | same | same — wraps in ObservableFs |

## Verification

```bash
# Unit/Integration tests (in tests/ directory)
cargo test -p foundation_platform --profile uat --test assets_suite
cargo test -p foundation_platform --profile uat --test ota_suite
cargo test -p foundation_platform --profile uat --test assets_security_suite
cargo test -p foundation_platform --profile uat --test manifest_suite

# Audit events only exist under the feature flag
cargo test -p foundation_platform --profile uat --features vfs-audit --test assets_security_suite

# Desktop — unaffected
cargo test -p foundation_platform --profile uat

# CLI round trip
cargo run -p foundation_platform --bin ewe-manifest -- init-keys
cargo run -p foundation_platform --bin ewe-manifest -- generate-manifests public/ --version 0.1.0
cargo run -p foundation_platform --bin ewe-manifest -- verify-manifest public/app/.ewe_manifest.json

# Android — APK, no extraction
cd examples/platform_android
cargo tauri android build --debug --target x86_64
adb install -r gen/android/app/build/outputs/apk/universal/debug/app-universal-debug.apk

# First launch — bundle version 0.1.0, nothing extracted
adb shell monkey -p com.ewe.platform 1
adb logcat -d | grep -E "asset_manager|platform_dashboard"
# Expected:
# asset manager ready: layout=Versioned base=/data/.../files version=0.1.0
# [platform] platform_dashboard: WASM active
# AppData holds only the delta dirs — APK assets are never copied out:
adb shell "ls /data/data/com.ewe.platform/files/app/"
# v0.1.0   (empty until an OTA writes into it)

# OTA in-place — write into the delta for ONE app
adb shell "echo 'console.log(\"OTA v2\")' > /data/data/com.ewe.platform/files/app/v0.1.0/platform_dashboard.js"
adb shell am force-stop com.ewe.platform
adb shell monkey -p com.ewe.platform 1
# Expected: overlay serves the delta copy, APK copy is shadowed.
# app-hello/ is untouched — its own version lifecycle is independent.

# OTA new version for one app — v0.1.1 of "app" only
adb shell "mkdir -p /data/data/com.ewe.platform/files/app/v0.1.1"
adb shell "echo '<html>OTA 0.1.1</html>' > /data/data/com.ewe.platform/files/app/v0.1.1/index.html"
# App calls manager.activate("app", "0.1.1")
# app_root("app")       -> files/app/v0.1.1/
# app_root("app-hello") -> files/app-hello/v0.1.0/   (unchanged)

# Pruning — per app, keep 2
adb shell "mkdir -p /data/data/com.ewe.platform/files/app/v0.1.2"
adb shell "ls /data/data/com.ewe.platform/files/app/"
# Before prune: v0.1.0  v0.1.1  v0.1.2
# After prune:  v0.1.1  v0.1.2   (v0.1.0 deleted; app-hello/ untouched)

# Rollback — activate a previous version of one app
# manager.activate("app", "0.1.0") — instant, files still on disk

# OTA-forced rollback — manifest contains rollback_to
# Manifest: { "rollback_to": "0.1.0", "delete_after": "0.1.2", "apps": [...] }
# App downloads v0.1.3 files, then: activate("0.1.0"), delete v0.1.2/
# Result: v0.1.0 (active) + v0.1.1 + v0.1.3 on disk, v0.1.2 deleted
```

## Files

| File | Action |
|------|--------|
| `backends/foundation_platform/src/assets.rs` | **NEW** — `PlatformAssetManager` + `AssetLayout` + `AssetResolverFs` + `ReadOnlyVfsFile`/`ReadOnlyVfsDirectory` |
| `backends/foundation_platform/Cargo.toml` | Add `foundation_nativeapis` (`default-features = false`, `features = ["vfs-native"]`), `foundation_errstacks`, `ed25519-dalek`, `sha2`, `base64`, `derive_more`; add the `vfs-audit` feature and the `ewe-manifest` bin |
| `backends/foundation_nativeapis/src/shared/api.rs` | Gate the reactor-backend lookup on `feature = "poll"` so `--no-default-features --features vfs-native` compiles (pre-existing break) |
| `backends/foundation_platform/src/lib.rs` | Add `pub mod assets;` + `pub mod manifest;` + re-exports |
| `backends/foundation_platform/src/session.rs` | Add `asset_manager` field + `set_asset_manager()`; `bundle_root()` prefers it |
| `backends/foundation_platform/src/responder.rs` | Add `MobileApp::mounted_at(assets, app_id)`; route reads through the session's asset manager when present |
| `backends/foundation_platform/src/builder.rs` | Replace `resolve_resource_root()` with `PlatformAssetManager::initialize()` + `ota_manifest_domain()` + `ota_manifest_key()` |
| `backends/foundation_platform/src/ota.rs` | Add `rollback_to`/`delete_after`/`signature`/`sequence`/`sha256` fields; VFS-backed writes; domain-locked init |
| `backends/foundation_platform/tests/assets_suite.rs` | **NEW** — layout selection, VFS I/O, overlay semantics over `AssetResolverFs` + `MemoryDelta`, per-app version management, pruning, concurrency |
| `backends/foundation_platform/tests/ota_suite.rs` | **NEW** — manifest signing, domain validation, replay detection, entry validation, provenance, install verification + atomicity, rollback directives + loop breaker |
| `backends/foundation_platform/tests/assets_security_suite.rs` | **NEW** — path traversal through every entry point, version/app-id sanitization, read-only base integrity, audit events under the feature flag |
| `backends/foundation_platform/tests/manifest_suite.rs` | **NEW** — key generation, manifest generation + idempotence, sha256 hashing, signing, verification |
| `backends/foundation_platform/tests/responder_mount_suite.rs` | **NEW** — `MobileApp::mounted_at` path resolution: segment stripping, SPA fallback, sibling-prefix confusion, content types, version tracking |
| `examples/platform_android/src-tauri/src/lib.rs` | Mount responders at `asset_manager().app_root(id)` via `MobileApp::mounted_at` |
| `examples/platform_android/src-tauri/src/generated/*.rs` | Regenerated by codegen to emit the mounted form |
| `backends/foundation_platform/src/manifest.rs` | **NEW** — library API: `ensure_keys`, `generate_app_manifest`, `generate_all_manifests`, `verify_manifest`, `sign_manifest`, `verify_file_hash` (target-gated) |
| `backends/foundation_platform/src/codegen.rs` | Generate `.ewe_manifest.json` per app via `manifest::generate_all_manifests()`; generate `build.rs` template calling manifest API |
| `backends/foundation_platform/src/cli.rs` | **NEW** — thin CLI wrapper around `manifest` library API: `init-keys`, `key-info`, `sign-manifest`, `verify-manifest`, `generate-manifests` |
| `backends/foundation_platform/src/main.rs` | **NEW** — CLI entrypoint (target-gated) |
| `keys/ota_public.key` | **NEW** — auto-generated Ed25519 public key (committed to git) |
| `keys/ota_private.key` | **NEW** — auto-generated Ed25519 private key seed (.gitignore'd) |
| `keys/.gitignore` | **NEW** — auto-generated, ignores ota_private.key |

## Non-goals

- We are NOT writing a general-purpose virtual filesystem — `foundation_nativeapis` provides that
- We are NOT wrapping every `std::fs` call — the VFS trait is the abstraction boundary
- We are NOT adding `tauri-plugin-fs` as a dependency — Tauri core `AssetResolver` is sufficient
- We are NOT changing the `MobileDirectory` trait or the `MobileDisk` extension — the VFS read path is added to `MobileApp` alongside them, not in place of them
- We are NOT enabling VFS audit in release builds by default — gated behind `cfg(feature = "vfs-audit")`
- We are NOT persisting the active version across launches — a new binary serves what it shipped with; only the manifest sequence and rollback loop-breaker state survive a restart
