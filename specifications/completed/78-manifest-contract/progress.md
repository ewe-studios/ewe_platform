# Spec 78 Progress: Manifest Contract

_Last updated: 2026-11-23._

**State.** Merged into master as `0aeffcc74`. The drift checks pass. Open: #697 (medium, spec 82).

## Status at a glance

| Task | State |
|---|---|
| M1 — JSON Schema from sections 5 and 6 | ✅ Done |
| M2 — Canonical fixtures and examples | ✅ Done |
| M3 — Drift check in CI | ✅ Done |
| M4 — Consumer handshake in 79 and 82 | ✅ Done |

The work is merged into master. Issue #697 belongs to spec 82 and stays open.

## What landed

### M1 — The schema

- `specifications/completed/78-manifest-contract/manifest.schema.json` is a JSON Schema draft 2020-12
  document. It covers the top level, `Entry`, every `source.kind`, every `deploy.kind`, the
  `proxy` block, the `probes` block and every documented default.
- The `entries` schema records V5 (unique names) in a `$comment`. JSON Schema draft 2020-12 has
  no keyword for uniqueness by one property, so the readers and `--check-schema` enforce V5.

### M2 — The fixtures and examples

- `fixtures/valid/complete.json` is the example of section 5.1, with full SHA-256 values.
- `fixtures/valid/empty.json` has an empty `entries` array.
- `fixtures/valid/unknown-field.json` has unknown fields at the top level and on one entry.
- `fixtures/valid/marks.json` is the `agent-rpc` entry for the marks scenario.
- `fixtures/valid/deploy-systemd.json`, `deploy-docker.json` and `deploy-none.json` show one
  example for each deploy kind.
- `fixtures/invalid/` holds one fault per file: `v3-version-2.json`, `v5-duplicate-name.json`,
  `v6-unknown-kind.json`, `v7-http-url.json` and `v7-bad-sha256.json`.
- `fixtures/README.md` gives the fixture table, the expected JSON path of each invalid fixture
  and the mark values for the marks fixture.

### M3 — The drift check and the CI step

- `scripts/ci/manifest_contract.py` adds `--check-schema` and `--check-copies`.
  - `--check-schema` checks the schema document, then validates every fixture with a small
    JSON Schema subset validator (standard library only, no network). It also applies the V5
    and V6 semantic rules. Each invalid fixture must fail at its recorded JSON path.
  - `--check-copies` compares each reader copy with the canonical file by SHA-256. A copy
    resolves first under the spec root (for `manifest.schema.json`) and then under
    `fixtures/` (for the fixtures). A missing consumer directory is not a fault: the readers
    have not added their copies yet.
- `scripts/ci/ci.py` gains the synthetic `manifest-contract` package. It owns
  `specifications/completed/78-manifest-contract` and runs the two checks in its `build` step.
  `MANIFEST_CONSUMER_PREFIXES` selects the package when a reader copy changes, and also
  selects the reader crate that owns the copy.
- `ci/packages.toml` sets the package to `build = "enforce"` and turns the other steps off.
- `scripts/ci/tests/test_manifest_contract.py` tests the schema, the fixtures, the semantic
  rules, the documented defaults, the copy drift (a one-byte change fails with the copy path),
  the command line and the CI selection.

### M4 — The consumer handshake

- `specifications/completed/79-manifest-installer/features/00-manifest-installer/feature.md` gains a
  "Fixture handshake with spec 78" section. It lists the 12 fixtures that the installer copies
  and the test `tests/manifest_tests.rs` that consumes them.
- `specifications/82-proxy-registration-and-registrar/features/01-registrar-sidecar/feature.md`
  gains the same section. It lists the 7 fixtures that the sidecar copies and
  `tests/manifest_tests.rs`. It records that V6 and scenario 4 of spec 78 make the sidecar read
  `source.kind`, even though it ignores the other source fields.

## Verify results

| Command | Result |
|---|---|
| `python3 -I scripts/ci/manifest_contract.py --check-schema` | 0; all fixtures match |
| `python3 -I scripts/ci/manifest_contract.py --check-copies` | 0; no consumer copies yet |
| `check-jsonschema --schemafile ... fixtures/valid/*.json` | 0; `ok -- validation done` |
| `python3 -m unittest discover -s scripts/ci/tests` | 143 tests, OK |
| `python3 scripts/ci/ci.py plan --packages manifest-contract` | 0; the package is in the matrix |

A raw `check-jsonschema` run accepts `invalid/v5-duplicate-name.json`. This is expected: JSON
Schema cannot express uniqueness by one property. The readers and `--check-schema` reject it.
This is recorded in `fixtures/README.md` and in LEARNINGS.

## Next steps

1. Merge the branch into master.
2. Spec 79 T1 adds `foundation_installer` and its `tests/fixtures/manifest/` copy.
3. Spec 82 T2 adds `foundation_registrar` and its copy. It must add `source.kind` to its view
   for the V6 check (issue #697).
4. After a reader adds its copy, `--check-copies` compares it. No change to the script is
   needed.

## Open questions

1. Keep `manifest_version`? Yes.
2. Prune removed entries? The installer decides (spec 79).
3. Linux only? Yes for version 1.
