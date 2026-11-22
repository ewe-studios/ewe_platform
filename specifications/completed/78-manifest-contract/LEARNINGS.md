# LEARNINGS — Manifest Contract

## Why the contract is its own specification (2026-11-14)

The manifest format lived in two feature files. The installer owned §4.1 and the registrar owned
the `proxy` and `probes` blocks. That split caused three problems:

1. A change to the format changed two feature files and two spec indexes.
2. The installer could drift from the registrar without a shared fixture.
3. The installer spec was large because it held the contract, the adapters, the deploy targets
   and the CLI.

The fix is one owner for the format, one schema and one set of fixtures. The consumers keep
their own Rust types, because the design forbids a shared runtime crate.

## Rules the consumers must not break

- The installer keeps `proxy` and `probes` as raw JSON. It must not reject a field that only the
  registrar understands.
- The registrar reads only `name`, `version`, `deploy.kind`, `proxy` and `probes`. It ignores the
  source block.
- Marks are part of the contract, not an installer detail. The registrar depends on them.
- A new field needs a fixture and a consumer copy in the same change.

## JSON Schema cannot express V5 uniqueness (2026-11-22)

**Problem.** The Done-when line "every invalid fixture fails the schema" cannot hold for
`invalid/v5-duplicate-name.json`. JSON Schema draft 2020-12 has no keyword for uniqueness by
one property. `uniqueItems` compares whole array items, and two entries with the same name
differ in other fields. A standard validator (`check-jsonschema`) accepts the file.

**Decision.** V5 stays a semantic rule. The `entries` schema records it in a `$comment`, the
readers enforce it (section 6, V5), and `scripts/ci/manifest_contract.py --check-schema`
rejects the fixture at `/entries/1/name`. The Done-when line now says "every invalid fixture
fails the contract check". The section 10 verify command validates only the valid fixtures
with `check-jsonschema`, which matches this decision.

**Lesson.** Write a contract check with two layers: a standard schema for structure, and a
small semantic pass for the rules that JSON Schema cannot express. State the split in the
schema itself, so a reader of the schema does not think a standard validator covers V5.

## A non-crate contract needs a synthetic CI package (2026-11-22)

The manifest contract is JSON and Python, with no crate. Two options existed:

1. Add the check to `foundation_core`'s enforced build step, like the other repository-wide
   scans. That step runs only when `foundation_core` runs, and a change to a reader's fixture
   copy does not select `foundation_core`.
2. Add a synthetic package, `manifest-contract`, in `load_workspace()`. Its `build` step runs
   the two checks. `MANIFEST_CONSUMER_PREFIXES` maps a reader copy to the package, and also to
   the reader crate that owns the copy.

Option 2 was chosen. It gives a light job that runs exactly when the contract or a copy
changes. The existing `examples` and `wasm-tests` packages use the same mechanism. A spec
change alone (`.md` files) is ignored by `affected`, so only a schema or fixture change selects
the package.

## A consumer copy resolves under `fixtures/`, then the spec root (2026-11-22)

The copy directory is `tests/fixtures/manifest/`, so a copy of `valid/complete.json` mirrors
`fixtures/valid/complete.json`. The first version of the check resolved only under the spec
root and reported a false "no canonical file" fault for a valid copy. The check now tries
`<spec>/<relative>` first (a copy of `manifest.schema.json`) and then
`<spec>/fixtures/<relative>` (a copy of a fixture).

## Spec 82's view omits `source.kind` (2026-11-22)

V6 and scenario 4 of spec 78 say both readers reject an unknown `source.kind` at
`/entries/0/source/kind`. The `ManifestView` and `EntryView` types in spec 82 feature 01 do not
have a `source` field. With serde's default unknown-field handling, the sidecar would accept
`invalid/v6-unknown-kind.json`. The M4 handshake note records that spec 82 must read
`source.kind` for the V6 check. Issue #697 (severity: medium) tracks this contract gap.

## Test the defaults, not only the fields (2026-11-22)

"No field has an undocumented default" is easy to lose when a description says "Default 10"
but the schema has no `default` keyword. `test_every_documented_default_is_in_the_schema`
lists every default from section 5 and reads the `default` keyword for it. A changed wording
without a schema default now fails a test.
