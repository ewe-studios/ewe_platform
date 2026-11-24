# LEARNINGS — Manifest Installer

## Split from spec 83 (2026-11-14)

The installer is one mission: read a manifest, install and start each entry, record the result,
and make a second run safe. It does not own the manifest format. It does not own health. It does
not own Cloudflare.

Three boundaries matter:

1. **The manifest contract is spec 78.** The installer parses it with its own types. A field
   change is a change to 78 and to the consumer's fixtures.
2. **Health is spec 82.** The installer proves that a program started. The registrar proves that
   it is healthy.
3. **The registrar finds entries by marks, not by the install record.** The install record is
   private to the installer. The marks (`EWE_ENTRY`, `EWE_VERSION`, Docker labels) are the
   public contract.

## Reuse

- `foundation_packager` renders the systemd unit and the bootstrap script. Do not hand-build
  strings.
- `foundation_deployment_docker` does all Docker work with generated types. Do not build a
  `docker` command string.
- `InstallContext` injects the HTTP client, command runner, Docker client, clock and state
  directory. No adapter reads a global.

## Implementation findings (2026-11-23)

1. **A Docker seam is necessary for tests.** `InstallContext` cannot hold only
   `DockerClient`: the deploy and image tests would need a live daemon. The
   `DockerApi` trait wraps the small set of calls that the installer uses, and
   `DockerEngine` is the one host implementation over `DockerClient`. The tests
   replace the trait, and every type on it is a generated Docker type.

2. **`foundation_packager` has no render-to-string.** `FileContent::run` writes
   to a path, which suits the unit file. The bootstrap script returns a
   `String`, so it renders with minijinja directly, the engine that
   `foundation_packager` wraps. Do not add a temporary file just to reach the
   helper.

3. **The old version must be in `Deployed`.** A rollback re-renders the old
   unit file. `previous: Option<&Deployed>` is the only old value that `apply`
   receives, so `Deployed::Systemd` and `Deployed::Container` carry `version`.
   The old program path (`exec_path`) points at the version directory, not at
   the `current` link, so the link can move back.

4. **Create the unit directory before rendering.** `FileContent::run` creates a
   file, not its parent. `SystemdDeploy::apply` makes
   `InstallerConfig::systemd_unit_dir` first. A test with a fresh temporary
   directory found this.

5. **`#[valtron_test]` does not make a body async.** A test that uses `.await`
   must be written `#[valtron_test] async fn ...`. The macro accepts an async
   body and drives it with `block_on_future`; a sync body with `.await` is a
   compile error.

6. **Let chains need Rust edition 2024.** This workspace is on edition 2021, so
   `if let ... && let ...` does not compile. Use a `match` that returns an
   `Option` and one `if let`.

7. **A fake Docker must map both the name and the id.** The installer starts a
   container by its id and checks it by name. The `create_container` response
   id must resolve in the fake, or the start gets a false 404.

8. **The canonical fixtures are a copy, not a link.**
   `scripts/ci/manifest_contract.py --check-copies` compares every file under
   `tests/fixtures/manifest/` with the spec 78 file by SHA-256. Copy the
   `README.md` and `manifest.schema.json` too; the script expects a canonical
   file for each copy.

9. **The bootstrap script must make the manifest directory.** `/etc/ewe` does
   not exist on a clean host, and `> "$MANIFEST_PATH"` fails without it. The
   script runs `mkdir -p "$(dirname "$MANIFEST_PATH")"` before the write.

