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
