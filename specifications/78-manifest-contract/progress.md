# Spec 78 Progress: Manifest Contract

_Last updated: 2026-11-14._

## Status at a glance

| Task | State |
|---|---|
| M1 — JSON Schema from sections 5 and 6 | ⬜ Pending |
| M2 — Canonical fixtures and examples | ⬜ Pending |
| M3 — Drift check in CI | ⬜ Pending |
| M4 — Consumer handshake in 79 and 82 | ⬜ Pending |

## Where this specification came from

This spec was extracted from spec 65 (`65-agent-rpc-serving-system`) on 2026-11-14. The manifest
format was split across the old feature 02 (`02-manifest-installer` §4.1) and feature 04
(`04-registrar-sidecar` §4.1). The contract now has one owner. The installer moved to spec 79;
the registrar moved to spec 82 feature 01.

## Next steps

1. Write `manifest.schema.json`.
2. Add the valid and invalid fixtures.
3. Add the drift check and wire it into `ci.py plan`.
4. Add the consumer handshake note to 79 and 82.

## Open questions

1. Keep `manifest_version`? Yes.
2. Prune removed entries? The installer decides (spec 79).
3. Linux only? Yes for version 1.
