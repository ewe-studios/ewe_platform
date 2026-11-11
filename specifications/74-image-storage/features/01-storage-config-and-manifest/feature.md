---
feature: "Storage configuration, manifest and retention rules"
status: "planned"
priority: "medium"
depends_on: []
crates: []
issues: []
model: "sonnet"
branch: "spec74/f01-storage-config"
---

# Feature 01: Storage configuration, manifest and retention rules

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Cloudflare R2 (R2), continuous integration (CI), gibibyte
(GiB), JavaScript Object Notation (JSON), Open Container Initiative (OCI), SHA-256, virtual
machine (VM), AeroSpace and Defence Industries Association of Europe (ASD), the owner of the
STE standard ASD-STE100.

## 1. Goal

Give every storage feature one place to read configuration and one typed manifest format. No
other feature invents a key, a default or a retention rule.

## 2. Measured state

1. `ci/platform-images/scripts/images.nu:38` `load-config` requires `registry`, `base` and
   `image`. It knows nothing about storage.
2. `ci/platform-images/scripts/vm.nu` takes a `--storage` path. It has no concept of an
   archive, a manifest or a checksum.
3. `ci/ewe-platform-image/config.toml` has `[registry]`, `[base]`, `[image]` and `[vm.*]`.
   It has no `[storage]`.
4. There is no Nushell test runner in the repository. `mise.toml` has no `test:ci-storage`
   task.

## 3. Requirements

1. `[storage]` in `ci/ewe-platform-image/config.toml` selects the registry backend and the
   object store. Section 7.1 of `requirements.md` gives the schema.
2. Every default in section 7.1 has a comment with its source. The loader rejects a value
   that is out of range: part size below 5 MiB or above 5 GiB, concurrency below 1 or above
   32, retries below 0 or above 10, presign expiry below 60 s or above 604,800 s, retention
   below 1.
3. The manifest type and its validation live in one module. A manifest that breaks any rule
   in `requirements.md` section 6.2 is a typed error with the field name and the reason.
4. The key layout is a function, not a string built at each call site. Given an OS, a base
   hash and a SHA-256, it returns the three object keys (body, checksum file, manifest).
5. Retention is a pure function. Given the manifests, it returns the current version, the
   versions to keep and the versions to delete. It never deletes the current version.
6. The Nushell test runner exists and `mise run test:ci-storage` runs it.
7. The code is in `ci/platform-images/scripts/storage.nu` and is loaded by `images.nu` and
   `vm.nu`. Do not duplicate the loader.

## 4. Design

### 4.1 Module surface

```nu
# storage.nu

# Load [storage] from a config file. Merge defaults. Validate every value.
export def load-storage [config: path]: nothing -> record

# Build the object keys for a disk version.
export def disk-keys [
    os: string,          # "windows" | "macos"
    base_hash: string,   # the hash-<hash> value from images.nu
    sha256: string,
]: nothing -> record   # { body, checksum, manifest, prefix }

# Validate a manifest record. Return it, or raise a typed error.
export def manifest-validate [manifest: record]: nothing -> record

# Build a manifest from measured values. Sets schema = 1.
export def manifest-new [
    os: string,
    base_hash: string,
    base_image: string,
    raw_bytes: int,
    compressed_bytes: int,
    sha256: string,
    license: string,
    previous: string = "",
]: nothing -> record

# Decide which versions to keep.
export def retain-plan [
    manifests: list<record>,
    keep: int,
]: nothing -> record   # { current, keep, delete }
```

Error shape:

```nu
error make {
    msg: $"manifest field `sha256` is not 64 lowercase hex characters"
    label: { text: "sha256", span: (metadata $manifest).span }
}
```

### 4.2 Key layout

```nu
disk-keys "windows" "a1b2c3d4e5f6" "9f2c..." == {
    prefix: "disks/windows/a1b2c3d4e5f6/"
    body: "disks/windows/a1b2c3d4e5f6/9f2c....tar.zst"
    checksum: "disks/windows/a1b2c3d4e5f6/9f2c....tar.zst.sha256"
    manifest: "disks/windows/a1b2c3d4e5f6/manifest.json"
}
```

The `prefix` field is what `retain-plan` lists. The manifest is read from the prefix, not
from a fixed "current" key, so two jobs cannot race on one small pointer object. A version
becomes visible only when its manifest object exists.

### 4.3 Retention

1. Sort by `created_at`, newest first.
2. `current` is the newest.
3. `keep` is the newest `retain_versions`.
4. `delete` is every manifest older than `keep`, except an object with the same SHA-256 as
   `current` (a duplicate upload).
5. A manifest with `license` not in the allowed set for the prefix is deleted and reported.
   This is a safety net; feature 04 refuses the upload first.

### 4.4 Test runner

`ci/platform-images/tests/mod.nu` sources `*_tests.nu`, runs every command whose name starts
with `test `, counts passes and failures, prints one line per test and exits 1 on a failure.
The `mise.toml` task `test:ci-storage` calls it. The runner starts and stops MinIO when a
case asks for it; feature 02 adds the MinIO compose file and the cases that need it.

The runner is plain Nushell. Do not add a third-party test framework.

## 5. Files

| File | Change |
| --- | --- |
| `ci/platform-images/scripts/storage.nu` | New module. |
| `ci/platform-images/scripts/images.nu` | Read the storage section after `load-config`. |
| `ci/platform-images/scripts/vm.nu` | Read the storage section in the new disk commands of feature 04. |
| `ci/ewe-platform-image/config.toml` | Add `[storage]` with the defaults and source comments. |
| `ci/platform-images/tests/mod.nu` | New test runner. |
| `ci/platform-images/tests/storage_tests.nu` | New tests. |
| `mise.toml` | Add `test:ci-storage`. |
| `ci/platform-images/README.md` | One paragraph that names the module and the test task. |

## 6. Tasks

- [ ] Add `[storage]` to `config.toml` with the defaults and the source comments.
- [ ] Write `storage.nu` with `load-storage`, `disk-keys`, `manifest-new`,
      `manifest-validate` and `retain-plan`.
- [ ] Add the typed errors and the range checks.
- [ ] Write `tests/mod.nu` and `tests/storage_tests.nu`.
- [ ] Add the `mise run test:ci-storage` task.
- [ ] Load the storage section in `images.nu` and print it in `main prepare` output.
- [ ] Add the README paragraph.

## 7. Tests

Tests use only the public functions. Each test must fail before the function exists.

| Test | Asserts |
| --- | --- |
| `test storage defaults` | An empty `[storage]` gives the defaults in section 7.1. |
| `test storage rejects part size` | 4 MiB and 6 GiB fail with the field name. |
| `test storage rejects concurrency and expiry` | Out-of-range values fail. |
| `test disk keys layout` | The four keys match the example in 4.2 for both OS values. |
| `test manifest round trip` | `manifest-new` then `manifest-validate` returns the same record. |
| `test manifest rejects bad sha256` | Short, upper-case and non-hex values fail. |
| `test manifest rejects unknown license` | A Windows manifest with `internal-apple-hardware` fails. |
| `test retain keeps newest n` | Five manifests with `keep = 2` keep the two newest and delete three. |
| `test retain never deletes current` | A list with one manifest and `keep = 1` keeps it. |
| `test retain removes duplicate sha` | Two manifests with the same SHA-256 keep one. |

## 8. Verification

```bash
mise run test:ci-storage
nu ci/platform-images/scripts/images.nu prepare \
  --config ci/ewe-platform-image/config.toml \
  --layer base --flavor default --out /tmp/pi-base
```

The second command must print the storage record and must not contact a network.

## 9. Done when

- `mise run test:ci-storage` passes every test in section 7.
- No other script builds a disk key or a default.
- The config comments name the source of every default.
- `progress.md` and `learnings.md` record the feature.

## 10. Risks

| Risk | Control |
| --- | --- |
| Nushell has no record type check at load time. | Validate every field in `manifest-validate`; tests cover each field. |
| A default drifts from the source. | The comment names the source; the test asserts the default value. |
| A test runner grows into a framework. | Keep it below 120 lines; one file per feature area. |
