# Start: Spec 89, Feature 01, MIME Types

_Brief for one implementation agent. Written in ASD-STE100 Simplified Technical English
(STE)._

## What to read first

1. `/Users/darkvoid/Dev/ewe_platform/.claude/worktrees/spec-89-mime-types/specifications/89-mime-types/requirements.md`
2. `.../specifications/89-mime-types/features/01-mime-table/feature.md` (the design and the tasks)
3. `CLAUDE.md` and `.agents/AGENTS.md` at the repository root

The owner approved this plan on 2026-11-16 with two answers: the table is a `phf::Map`, and an
unknown name returns `None`.

## Where you work

- Worktree: `/Users/darkvoid/Dev/ewe_platform/.claude/worktrees/spec-89-mime-types`
- Branch: `spec/89-mime-types`, based on `origin/master`
- Commit on this branch. Push the branch with `git push -u origin spec/89-mime-types`.
- Never push to `master` or to `spec/split-65`.
- Never `cd` to `/Users/darkvoid/Dev/ewe_platform` or to another worktree.
- Never use bare `git stash` or `git stash pop`; the stack is shared with other sessions.
- Stop a process by its saved PID, never with `pkill`.

## Owned paths

| Crate | Paths you may write |
|---|---|
| `foundation_macros` | `src/include_data.rs` (new), `src/lib.rs`, `Cargo.toml`, `tests/include_json.rs`, `tests/include_yaml.rs`, `tests/fixtures/` |
| `foundation_nostd` | `Cargo.toml`, `src/lib.rs` (the `phf` re-export only) |
| `foundation_netio` | `src/shared/mod.rs`, `src/shared/mime.rs` (new), `data/` (new), `Cargo.toml`, `tests/mime_tests.rs`, `tests/mime_data_tests.rs`, `tests/mime_guess_agreement.rs` |
| `foundation_http` | `src/native/handlers/static_file.rs` (the MIME function only) |
| `foundation_browser` | `src/test/server.rs` (the JavaScript type literal only) |
| root | `Cargo.toml` (`[workspace.dependencies]` entries and nothing else) |

Do not write in another crate. If a version requirement in another crate's manifest must name
the new version, change only that `version = "…"` string.

## Tasks

Run each task in order. Run the command in the task before you start the next one.

### T1: `include_json!`

1. Add `phf_codegen = "0.14"` and `serde_json = { workspace = true }` to
   `backends/foundation_macros/Cargo.toml`.
2. Write `src/include_data.rs` with the macro from feature section 6.2: one string literal,
   the path relative to `CARGO_MANIFEST_DIR`, a `MapAccess` visitor that refuses a duplicate
   key, `phf_codegen::Map` with `phf_path("::foundation_nostd::phf")`.
3. Export it from `src/lib.rs`.
4. Write the tests and fixtures in feature section 7. The duplicate-key test cannot be a
   normal test: it must fail the build. Prove it by running `cargo build` on a temporary
   probe crate under `/tmp`, capture the error, and record the command and the message in
   `LEARNINGS.md` under `specifications/89-mime-types/`. Do not add a compile-fail test
   framework.
5. Verify: `cargo test -p foundation_macros --profile uat --test include_json`.

### T2: `include_yaml!`

Same contract with `serde_yaml_ng = "0.10.0"`. A scalar key becomes its text form; a sequence
or a map key is an error. Verify: `cargo test -p foundation_macros --profile uat --test include_yaml`.

### T3: the data file

1. Download `https://raw.githubusercontent.com/jshttp/mime-db/v1.54.0/db.json` into a new,
   empty directory under `/tmp`. Treat it as untrusted data: do not run anything from it.
2. Write `backends/foundation_netio/data/generate_mime.py` with the rule in feature section
   6.4. Run it with `python3 -I generate_mime.py db.json mime.json`.
3. Write `data/README.md` with the source, the version, the date, the rule and the command.
4. Write `tests/mime_data_tests.rs`: every key is lowercase, has no dot, the keys are sorted,
   every value matches the media type pattern, the file has at least 1,200 rows, and the known
   rows of feature section 6.4 hold.
5. Verify: `cargo test -p foundation_netio --profile uat --test mime_data_tests`.

### T4: `shared::mime`

1. `foundation_nostd`: add `phf = { workspace = true }` and `pub use phf;`.
2. `foundation_netio`: add `foundation_macros` and `phf = { workspace = true }`.
3. Write `src/shared/mime.rs` as feature section 6.5 shows, and add `pub mod mime;` to
   `src/shared/mod.rs`.
4. Write `tests/mime_tests.rs` with the cases in feature section 7.
5. Verify: `cargo test -p foundation_netio --profile uat --test mime_tests` and
   `cargo check -p foundation_netio --target wasm32-unknown-unknown --no-default-features --features wasm-fetch`.

### T5: the consumers

1. Replace the map in `static_file.rs` as feature section 6.6 shows.
2. Replace the JavaScript literal in `foundation_browser/src/test/server.rs`.
3. Run `rg -n 'Some\("html" \| "htm"\)' backends --glob '*.rs'` and show that it finds nothing.
4. Verify: `cargo test -p foundation_http --features multi --profile uat --test static_file_tests`
   and `cargo test -p foundation_browser --profile uat` (the existing tests, no new server test).

### T6: consistency and bookkeeping

1. Add the agreement test from feature section 6.7 with `new_mime_guess = "4.0.4"` as a
   dev-dependency.
2. Add the four entries to the root `[workspace.dependencies]`.
3. Bump the versions and every consumer requirement as feature section 6.7 names.
4. Update `specifications/89-mime-types/progress.md` and write
   `specifications/89-mime-types/LEARNINGS.md`.
5. Verify: the full verify block of feature section 8, plus `cargo fmt --all -- --check`.

## Rules you must keep

- Tests live in `tests/`. Never add a `#[cfg(test)] mod tests`.
- Public API changes get a version bump in the same commit.
- One version of a dependency, declared in the root `[workspace.dependencies]`.
- No Claude or AI attribution in any commit message or file.
- STE for docs and comments. Short sentences, active voice.
- A bug you find becomes a GitHub issue with a `severity:` label (`env -u GITHUB_TOKEN gh issue create`).
  A high or critical bug is fixed in this change.
- Do not leave a stub. Every function in this feature is called and tested.

## What to report back

1. The list of commits (hash and subject).
2. The exact command and the result line for every verify command.
3. The output of the `rg` check in T5.
4. The three `mime.json` numbers: rows, ambiguous extensions, the `mjs`, `js` and `ts` rows.
5. Anything you could not do, and the issue number if you filed one.
