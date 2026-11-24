# Spec 79 Progress: Manifest Installer

_Last updated: 2026-11-23._

## Status at a glance

| Feature | State | Source |
|---|---|---|
| 00 — manifest installer | ✅ 10/10 tasks implemented, verified and merged as `d804580d5` | `backends/foundation_installer` |

## What landed

The new crate `backends/foundation_installer` holds the whole feature:

- `manifest.rs` parses and validates the spec 78 contract (rules V3 to V9)
  against the copied canonical fixtures.
- `source/` has the `SourceAdapter` trait and the six adapters:
  `github-release`, `url`, `cargo`, `apt`, `mise` and `docker-image`.
  Downloads stream to a temporary file and are renamed after the SHA-256
  matches.
- `deploy/` has the `DeployTargetAdapter` trait and the three adapters:
  `systemd`, `docker` and `none`. Docker calls go through the injected
  `DockerApi`, whose host implementation uses `foundation_deployment_docker`.
- `record.rs` keeps the install record and the exclusive lock.
- `installer.rs` runs or plans the entries, upgrades one entry and reports a
  rolled-back upgrade with `kept`.
- `bootstrap.rs` renders the bootstrap script from its template.
- `src/bin/ewe-installer.rs` is the `apply`, `plan` and `render-bootstrap`
  command-line interface.
- `tests/` holds the six test targets and the shared fakes.

## Verification

| Command | Result |
|---|---|
| `cargo test -p foundation_installer --test manifest_tests` | 15 passed |
| `cargo test -p foundation_installer --test source_adapter_tests` | 9 passed |
| `cargo test -p foundation_installer --test deploy_adapter_tests` | 7 passed |
| `cargo test -p foundation_installer --test installer_tests` | 7 passed |
| `cargo test -p foundation_installer --test bootstrap_tests` | 11 passed |
| `cargo test -p foundation_installer --features docker-tests --test install_e2e_tests` | 1 passed (real Docker daemon) |
| `cargo test -p foundation_installer` | all targets passed |
| `python3 scripts/ci/manifest_contract.py --check-copies` | every consumer copy matches |
| `cargo fmt --all -- --check` | clean |
| `cargo clippy -p foundation_installer --all-targets --no-deps -- -D warnings` | clean |
| `ewe-installer plan` on `valid/complete.json` | exit code 0, three install steps |
| `ewe-installer apply` on `valid/empty.json` | exit code 0, no entries |

The systemd end-to-end case in `install_e2e_tests.rs` runs only on a Linux
host with root and systemd. It did not run on the macOS development host; the
Linux CI job runs it.

## Deviations from the feature design

1. `Installer::run` returns `Result<RunReport, InstallError>`. The design gives
   `RunReport`, but the run needs to report `Locked` and a record-read failure
   to the command line, which must exit with code 2 for them.
2. `Deployed::Systemd` and `Deployed::Container` carry a `version`. A failed
   upgrade needs the old version to render the old unit again.
3. `InstallContext` holds a `DockerApi` seam, not `DockerClient` directly. The
   host implementation `DockerEngine` uses `DockerClient` and its generated
   types; the seam lets the deploy tests run with no daemon.
4. The `cargo` source gives cargo an explicit `--root` under the artifact
   directory, so the program path is known and no environment variable decides
   it.
5. `InstallError` adds `ManifestIo`, `ManifestTooLarge`, `UnknownKind` and
   `MissingSideEffect`. The first two are spec 78 rules V1; `UnknownKind` is
   the V6 error with its JSON path; the last names a missing injected effect.
6. The bootstrap script renders with minijinja directly because
   `foundation_packager` has no render-to-string function. The systemd unit
   goes through `foundation_packager` as designed.

## What is next

1. Review and commit the change. Add the Linux and Windows CI results.
2. Run the systemd end-to-end case on the Linux CI image.
3. Spec 82 feature 01 consumes the same manifest. The marks are in place.

## Follow-up from the registrar end-to-end test (2026-11-23)

The spec 82 feature 01 end-to-end test runs the installer against a real Docker
daemon. It found two defects and both are fixed:

1. **The image pull sent the wrong fields.** `DockerEngine::image_pull` passed
   `repo` and `tag` to `POST /images/create`. The daemon read `repo` as a source
   URL and failed with "no Host in request URL". It now sends `fromImage` and
   `tag`, as the Docker API requires for a registry pull.
2. **The old container stopped too early.** The Docker target created and
   started `{name}-next`, then stopped the old container at once. The registrar
   sidecar had no time to discover the new address, so the proxy sent requests
   to a dead address and answered `502`. `InstallerConfig::old_container_grace`
   (default 15 s, longer than the sidecar's default 10 s report interval) keeps
   the old container up until the sidecar registers the new address.

The end-to-end test now passes with no failed request during an upgrade.
