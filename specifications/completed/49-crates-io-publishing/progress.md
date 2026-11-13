# Spec 49 progress

_Written in ASD-STE100 Simplified Technical English (STE)._

_Newest first._

Glossary:

- **release-plz**: the tool that bumps crate versions, writes changelogs and publishes
  crates. The workflow `.github/workflows/release-plz.yaml` runs it.
- **release PR**: the "chore: release" pull request that release-plz opens with the version
  bumps.
- **publish cycle**: crate A cannot publish before crate B, and B cannot publish before A.

## 2026-11-09: the spec is complete and archived

The owner marked the spec complete. The spec moved to
`specifications/completed/49-crates-io-publishing`. `LEARNINGS.md` is now `learnings.md`,
as in the other specs.

What landed over the life of the spec:

| Item | Where |
| --- | --- |
| Crate names checked and descriptions added (feature 00) | `learnings.md`, 2026-06-16 |
| Automatic publishing in dependency order (features 02 to 07) | `.github/workflows/release-plz.yaml`, `release-plz.toml`, `docs/releasing.md` |
| A packaging check on every push to master | `scripts/ci/package_workspace.py` (the "Publish dry-run" job in `check.yaml`) |

The last release run, 42516594075 (2026-11-08), failed in both jobs. Branch
`agent/release-fixes` fixes both:

| Job | Cause | Fix | Issue |
| --- | --- | --- | --- |
| Publish to crates.io | `foundation_nostd` had a dev-dependency on `foundation_core` with a `path` and a `version`, which made a publish cycle | Remove the `version`. `package_workspace.py` now fails on this kind of cycle | #442 |
| Open or update the release PR | Stable cargo 1.98 cannot load root manifests from before `df86d7e6`, which have the nightly-only `codegen-backend` key | The release-PR job uses the pinned nightly; the publish job stays on stable | #443 |

State of crates.io on 2026-11-09, read from the crates.io API against
`cargo metadata --no-deps` (104 publishable crates):

- 22 crates are on crates.io at the workspace version.
- 38 crates are on crates.io at an older version. The publish job publishes the new
  versions on its next run, because the publish cycle no longer stops it.
- 44 crates are not on crates.io yet: `foundation_boringtun` and the 43
  `foundation_cloudflare_*` group crates. The publish job publishes each crate whose version
  is not on crates.io, so these also go out on its next run.

### Next

- After the parent session pushes `agent/release-fixes`, watch the next "Publish crates to
  crates.io" run. Both jobs must pass.
- Check that the 44 new crates publish. crates.io limits how fast one account can create new
  crates. If the run stops on a rate limit, run it again: release-plz skips the versions that
  are already on crates.io.
