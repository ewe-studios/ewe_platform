# Spec 79 Progress: Manifest Installer

_Last updated: 2026-11-14._

## Status at a glance

| Feature | State | Source |
|---|---|---|
| 00 — manifest installer | ⏳ 0/10, design complete | Moved from spec 65 feature 02 |

## What moved

- The installer feature moved from `specifications/65-agent-rpc-serving-system/features/02-manifest-installer`
  to `features/00-manifest-installer`.
- The manifest format (§4.1) now lives in spec 78. The installer feature keeps its own Rust
  types and points to 78 for the rules and fixtures.
- The installer's systemd unit work is task T5 in the feature.

## Next steps

1. Finish the move: remove the duplicated manifest tables from the feature and add the
   nine-section story wrapper from `.agents/skills/story-driven-specification`.
2. Add the feature to `ci/packages.toml` when the crate exists.
3. Implement T1 (manifest types and validation) against the 78 fixtures.

## Open questions

1. Keep `manifest_version`? Yes (78).
2. Prune entries that leave the manifest? The feature's open question 2.
3. Linux only? Yes for version 1.
