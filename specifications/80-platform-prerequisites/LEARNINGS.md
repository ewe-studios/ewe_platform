# LEARNINGS — Platform Prerequisites

## Why the seams left spec 83 (2026-11-14)

Spec 83 bundled four missions. The Env bundle, the transport facts and the migration groups
change shared crates that the proxy also uses. Keeping them in the agent RPC spec made the proxy
spec depend on the agent RPC spec, which is the coupling the design forbids.

The features now have one owner. The agent RPC server (83), the proxy (82), the auth library
(81) and mutual TLS (87) all depend on this spec.

## Why the `Env` bundle left this specification again (2026-11-15)

The owner reviewed six conflicts on 2026-11-15. One of them was the `Env` design here: it
described a second clock, an `IdSource` with two meanings and a bundle that did not match the
`Ctx` that spec 76 had already landed. A consumer that read both designs could not tell which
one was true.

The owner direction: "Ctx should be the main means of all of such injection... lets create a
small spec (featureless) that adds the bits into foundation_context then others get it by
default."

So the bundle is [spec 84](../84-context-injection/requirements.md) now. It is featureless,
it extends `Ctx`, and it owns the one `Timer`, `EntropySource` and `IdSource`. This spec keeps
only what is about the transport and the database.

The lesson: a bundle of injected effects belongs next to the type that carries it, not in a
specification about platforms. The two designs drifted while they had different owners.

## Do not repeat spec 76 or spec 84

Spec 76 already built `foundation_context::Ctx` with the clock, config and a seeded RNG, and
moved about 600 sites. Spec 84 adds the `Env` entry.

This spec adds the remaining delta only:

- connectrpc request extensions, and the deadline clock from `Ctx`.
- foundation_http TLS peer facts.
- Migration groups and `FleetRunner`.

A second context type is a defect. A second clock is a defect. Verify the baseline before
writing code.

## Moved design files

- `features/01-migration-groups/migrations.md` — the migration-group design.

The Env design and the deterministic-testing document are in
`specifications/84-context-injection/`.

## A public type keeps one shape on every target (2026-11-15)

`MigrationSource::Directory(PathBuf)` was a `#[cfg(not(target_family = "wasm"))]` variant, so
the public enum changed shape per target. The owner asked for every instance of that pattern to
be checked across the split specifications (spec 77 section 6.0, the part wrappers).

The variant now exists on every target. A `PathBuf` is portable; only reading a directory is
not. `MigrationRunner::plan` reports the new `MigrationProblem::NoFileSystem { group }` when a
wasm host is given a `Directory` group. The readers stay gated, because they are capabilities:
`MigrationLayout::scan`, `from_dir` and `add_from_dir` are native only.

The rule: a `cfg` goes on a private inner type, a method, a module or a dependency. It never
changes the variants of a public enum.
