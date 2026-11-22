---
feature: "MIME Types"
description: "One compile-time media type table for the platform: a JSON data file from mime-db, an include_json and include_yaml macro in foundation_macros, and a shared module in foundation_netio that every crate uses."
status: "done"
priority: "medium"
depends_on: []
estimated_effort: "medium"
created: 2026-11-16
last_updated: 2026-11-16
author: "Main Agent"
model: "sonnet"
tasks:
  completed: 6
  uncompleted: 0
  total: 6
  completion_percentage: 100%
---

# MIME Types

_Status: **done 2026-11-16**. Written in ASD-STE100 Simplified Technical English (STE).
Code and identifiers do not change._

**Abbreviations in this file.** Multipurpose Internet Mail Extensions (MIME); Internet Assigned
Numbers Authority (IANA); JavaScript Object Notation (JSON); Yet Another Markup Language
(YAML); perfect hash function (phf); continuous integration (CI); Uniform Resource Identifier
(URI).

## 1. Goal

A crate asks `foundation_netio::shared::mime` for the media type of a file name. The answer
comes from one table. The table is built at compile time from a JSON file that the repository
owns. No crate keeps its own extension map.

## 2. Story

A contributor adds a `.mjs` file to a directory that `StaticFileHandler` serves. Chrome refuses
the file, because the server sends `application/octet-stream`. The contributor opens one data
file, sees no `mjs` row, and adds it. No Rust code changes. A test proves the row works.

## 3. Bounded context

### 3.1 Inside

- `foundation_macros`: the `include_json!` and `include_yaml!` proc macros.
- `foundation_nostd`: the `phf` re-export.
- `foundation_netio`: the `shared::mime` module and the vendored data.
- `foundation_http`: the static file handler that calls the module.
- `foundation_browser`: the test server that names a JavaScript type.
- `backends/foundation_netio/data/`: the data file, the generator and the README.

### 3.2 Outside

- Content-Type parsing, negotiation and compression (other modules).
- The embedded-asset pipeline in `foundation_macros/src/embedders.rs`. It keeps
  `new_mime_guess`. T6 proves the tables agree on common types.

### 3.3 Verbs

| Verb | Meaning |
|---|---|
| include | read a data file at compile time and emit a Rust value |
| lookup | turn an extension or a path into a media type |
| generate | rebuild the vendored data file from mime-db |

## 4. Rules

### 4.1 Laws worksheet

| Law | Rule |
|---|---|
| R1 | One table, in `foundation_netio::shared::mime`. |
| R2 | The map is a compile-time value. No file read at run time. |
| R3 | The module returns `Option<&'static str>`; the caller picks the fallback. |
| R4 | Lookup ignores case and a leading dot; `from_path` reads the last extension. |
| R5 | The data file names its source, version and selection rule; a script regenerates it. |
| R6 | A table value is a bare media type, never a parameter. |
| R7 | The generator resolves an ambiguous extension by a written rule, not by file order. |

### 4.2 Invariants

- **I1.** For every row in the data file, `from_extension(key)` returns its value.
- **I2.** `from_path("/a/file.KEY")` equals `from_extension("KEY")` for every row.
- **I3.** `from_extension` returns `None` for an unknown name; it never panics.
- **I4.** The build fails when the data file is missing, malformed, has a duplicate key or a
  non-string value. The error names the file and the key.
- **I5.** The module compiles for `wasm32-unknown-unknown` with the `wasm-fetch` feature set.
- **I6.** A crate that serves a file names `foundation_netio` and no other table.

### 4.3 Must have

- `include_json!` and `include_yaml!` with tests.
- `foundation_netio::data/mime.json`, generated from mime-db 1.54.0, with a README and a script.
- `foundation_netio::shared::mime` with `from_extension` and `from_path`.
- The static file handler and the browser test server use the module.
- `phf` re-exported by `foundation_nostd`.
- Version bumps for the three crates that gain public surface.

### 4.4 Must do

- Write the tests in `tests/`, never inline.
- Record every new dependency in the root `[workspace.dependencies]` with its version.
- Update the consumers' version requirements in the same change.

### 4.5 Must not

- Keep a second extension map in any crate.
- Read the JSON file at run time.
- Put a media type default inside `foundation_netio`.
- Add an inline `#[cfg(test)] mod tests`.

## 5. Acceptance scenarios

| # | Given | When | Then |
|---|---|---|---|
| S1 | a crate calls `include_json!("data/app.json")` | it builds | the map has the file's pairs and a missing key returns `None` |
| S2 | the JSON file has a duplicate key | the crate builds | the build fails and the error names the key |
| S3 | a served directory holds `app.mjs` | a client requests it | the response says `text/javascript` (#578) |
| S4 | a caller asks for an unknown name | it calls `from_extension` | it receives `None` and no panic |
| S5 | a contributor updates mime-db | they run `generate_mime.py` | the diff holds only data rows and the README version |

## 6. Technical breakdown

### 6.1 Tasks

| Task | What | Files | Done when |
|---|---|---|---|
| T1 | `include_json!`: compile-time map from a JSON file | `backends/foundation_macros/src/include_data.rs`, `src/lib.rs`, `Cargo.toml`, `tests/include_json.rs`, `tests/fixtures/*.json` | the tests in 6.2 pass, and a bad file fails with a named error |
| T2 | `include_yaml!`: the same for YAML | the same files, `tests/include_yaml.rs`, `tests/fixtures/*.yaml` | the YAML tests pass |
| T3 | Vendor mime-db 1.54.0 and generate the flat file | `backends/foundation_netio/data/{mime.json,generate_mime.py,README.md}`, `tests/mime_data_tests.rs` | the data tests pass and `mime.json` matches the rule |
| T4 | `shared::mime` with the two functions | `backends/foundation_netio/src/shared/{mod.rs,mime.rs}`, `tests/mime_tests.rs`, `Cargo.toml` | the module tests pass, and the wasm check in section 8 passes |
| T5 | Move the consumers onto the module | `foundation_http/src/native/handlers/static_file.rs`, `foundation_browser/src/test/server.rs` | no extension map remains outside `foundation_netio`; the #578 test stays green |
| T6 | Consistency, phf re-export, versions, CI | `foundation_nostd/{Cargo.toml,src/lib.rs}`, `foundation_netio/tests/mime_guess_agreement.rs`, the manifests of every consumer | the agreement test passes, CI is green on Linux, macOS and Windows |

Order: T1, then T2 and T3 in parallel. T4 needs T1 and T3. T5 needs T4. T6 last.

### 6.2 T1: `include_json!`

The macro takes one string literal. The path is relative to the calling crate's
`CARGO_MANIFEST_DIR` (decision D5).

```rust
// backends/foundation_macros/src/include_data.rs
//! Compile-time data inclusion: `include_json!` and `include_yaml!`.
//!
//! WHY: a crate that needs a fixed table should not read and parse a file at
//! run time, and should not turn the data into a hand-written match arm.
//!
//! WHAT: one proc macro expands to a `phf::Map<&'static str, &'static str>`
//! literal that holds every pair of the file.
//!
//! HOW: the macro reads the file from CARGO_MANIFEST_DIR, parses it with
//! `serde_json` (or `serde_yaml_ng`), refuses a duplicate key or a non-string
//! value, and writes the map with `phf_codegen::Map`.
```

The emission step:

```rust
let mut map = phf_codegen::Map::new();
map.phf_path("::foundation_nostd::phf"); // phf_codegen 0.14 writes a bare `::phf` by default
for (key, value) in pairs {
    map.entry(key.as_str(), format!("{value:?}").as_str());
}
let literal = map.build().to_string(); // "::foundation_nostd::phf::Map { key: …, disps: …, entries: … }"
```

The path makes the caller depend on `foundation_nostd` only. `foundation_nostd` adds `pub use phf;`
and the `phf` dependency with `default-features = false`; the crate signals
`#![cfg_attr(not(feature = "std"), no_std)]`, and `phf_shared` and `siphasher` under it are
`no_std` too (measured in the 0.14.0 sources on 2026-11-16). `Map::get` needs `Eq + PhfHash`,
and `PhfHash for str` is available without `std`. A test compiles a map and reads it; a wrong
path fails at compile time, which is the proof.

Error cases, each a `compile_error!` with the file path and the key:

| Input | Message |
|---|---|
| no literal, or two arguments | "include_json! takes exactly one string literal path" |
| the file is absent | "include_json! cannot read <path> (relative to <manifest dir>)" |
| an object with a repeated key | "include_json! finds a duplicate key \"<key>\" in <path>" |
| a value that is not a string | "include_json! needs a string value for \"<key>\", found <kind>" |
| text that is not an object | "include_json! needs a JSON object of strings in <path>" |

A missing duplicate check is the defect that hides data silently. `serde_json` keeps the last
pair when a map has a repeated key. The macro parses the object through a `MapAccess` visitor,
so it sees every pair and can refuse the repeat.

### 6.3 T2: `include_yaml!`

The same contract, parsed with `serde_yaml_ng` 0.10.0. YAML allows a key that is not a string
(an integer, a boolean). The macro converts a scalar key to its text form and refuses a
sequence or a map key. The tests cover a plain file, a duplicate key and a nested value.

### 6.4 T3: the data file

`generate_mime.py` takes the path of a `db.json` download and writes `mime.json`. It needs
Python 3 and no other package. The rule from R7:

1. collect every `(extension, media type, source)` triple;
2. for an extension with one candidate, keep it;
3. for an extension with several, keep the first by this order: `source == "iana"`, then no
   `source` key, then `source == "apache"`, then any other source; inside one rank, take the
   alphabetically first media type;
4. apply the reviewed overrides: `mp4` and `mpg4` become `video/mp4` (issue #593);
5. write a flat object, sorted by extension, two-space indent, one pair per line.

Measured with this rule on mime-db 1.54.0 (2026-11-16):

| Extension | Result | Reason |
|---|---|---|
| `mjs` | `text/javascript` | the only IANA candidate |
| `js` | `text/javascript` | IANA beats Apache `application/javascript` |
| `wasm` | `application/wasm` | the only IANA candidate |
| `map` | `application/json` | the only IANA candidate |
| `md` | `text/markdown` | the only IANA candidate |
| `ts` | `video/mp2t` | the only IANA candidate; see the out-of-scope note |
| `mp4` | `video/mp4` | reviewed override; browsers expect the video type (issue #593) |
| `mpg4` | `video/mp4` | the same container and ambiguity (issue #593) |

`README.md` holds the source Uniform Resource Identifier (URI), the version, the download date,
the rule and the command:

```bash
curl -sSLO https://raw.githubusercontent.com/jshttp/mime-db/v1.54.0/db.json
python3 -I generate_mime.py db.json mime.json
```

`mime.json` is a vendored copy. CI never downloads it. The data test in section 7 checks the
shape and the known rows.

### 6.5 T4: `shared::mime`

```rust
// backends/foundation_netio/src/shared/mime.rs
//! Media types for a file name, from one compile-time table.
//!
//! WHY: every crate that serves a file needs the same answer, and a hand-written
//! match arm missed `.mjs` once (issue #578).
//!
//! WHAT: `from_extension` and `from_path`, backed by `data/mime.json`, which
//! `generate_mime.py` builds from mime-db 1.54.0.
//!
//! HOW: `include_json!` expands the file into a `phf::Map` at build time.

use std::path::Path;

/// Every media type, by extension. Built at compile time (rule R2).
pub static TYPES: ::foundation_nostd::phf::Map<&'static str, &'static str> =
    ::foundation_macros::include_json!("data/mime.json");

/// The media type for `ext`, without the dot and without case.
pub fn from_extension(ext: &str) -> Option<&'static str> {
    let ext = ext.trim_start_matches('.').to_ascii_lowercase();
    TYPES.get(ext.as_str()).copied()
}

/// The media type for the last extension of `path`.
pub fn from_path(path: impl AsRef<Path>) -> Option<&'static str> {
    path.as_ref().extension()?.to_str().and_then(from_extension)
}
```

`from_extension` allocates only when the text has an upper-case letter or a dot. A caller with
a known-lower extension can use `TYPES.get(ext).copied()` directly; the doc says so.

`foundation_netio` gains `foundation_macros` as a normal dependency. It is a proc-macro crate
that compiles for the host, so the wasm build stays clean.

### 6.6 T5: the consumers

`static_file.rs`:

```rust
fn mime_type(path: &Path) -> &'static str {
    foundation_netio::shared::mime::from_path(path)
        .unwrap_or(foundation_netio::shared::mime::OCTET_STREAM)
}
```

`OCTET_STREAM` is a `pub const &str = "application/octet-stream"` in the module. It is the
caller's default, not a module decision (R3).

`foundation_browser/src/test/server.rs`:

```rust
let js_type = foundation_netio::shared::mime::from_extension("js")
    .expect("mime.json holds the js row");
let js_ct = format!("{js_type}; charset=utf-8");
```

A repository search proves no second map remains:

```bash
rg -n 'Some\("html" \| "htm"\)|"application/javascript"' backends --glob '*.rs'
```

### 6.7 T6: consistency and bookkeeping

- `foundation_nostd/src/lib.rs` gains `pub use phf;`; its manifest gains
  `phf = { workspace = true }`.
- `foundation_netio/tests/mime_guess_agreement.rs` compares 20 common extensions with
  `new_mime_guess::MimeGuess::from_ext` (dev-dependency `new_mime_guess = "4.0.4"`). A mismatch
  fails the test with both values.
- Version bumps, in this change: `foundation_nostd` 0.0.9 to 0.0.10, `foundation_netio` 0.0.14
  to 0.0.15, `foundation_macros` 0.0.21 to 0.0.22, and every `version = "…"` requirement on
  them in the other crates.
- New workspace entries: `phf = { version = "0.14", default-features = false }`, `phf_codegen =
  "0.14"`, `serde_yaml_ng = "0.10.0"`, `new_mime_guess = "4.0.4"` (dev).

## 7. Tests

| Law or scenario | Test | Oracle |
|---|---|---|
| S1, I1 | a map from a fixture file holds every pair | example |
| S2, I4 | a duplicate key fails the build | the error message |
| I4 | a missing file, a non-object and a nested value fail the build | the error message |
| S1 | `include_yaml!` reads a fixture | example |
| I1, I2, S3 | `from_extension` and `from_path` on known rows | example (`mjs`, `js`, `wasm`, `html`, `svg`) |
| R4, I3 | case, leading dot, no extension, unknown name, `.bashrc` | expected outcome |
| I5 | `cargo check -p foundation_netio --target wasm32-unknown-unknown` | the compiler |
| 6.4 | the data file shape and the known rows | the rule and mime-db |
| T6 | 22 extensions agree with `new_mime_guess` | example |

The data test also checks: every key is lowercase ASCII, every value matches
`[a-z0-9.+-]+/[a-z0-9.+-]+`, the file holds at least 1,200 rows, and the keys are sorted.

## 8. Verify

```bash
cargo test -p foundation_macros --test include_json --test include_yaml --profile uat
cargo test -p foundation_netio --profile uat --test mime_tests --test mime_data_tests \
  --test mime_guess_agreement
cargo test -p foundation_http --features multi --profile uat --test static_file_tests
cargo test -p foundation_platform --profile uat --test content_type_tests
cargo check -p foundation_netio --target wasm32-unknown-unknown \
  --no-default-features --features wasm-fetch
cargo clippy -p foundation_netio -p foundation_macros -p foundation_nostd --profile uat --all-targets
cargo fmt --all -- --check
```

The full run is the pull request's GitHub CI: Linux on every push; macOS and Windows on a full
run (rule 16). `scripts/ci/linux.sh foundation_netio` is the local Linux form.

## 9. Done when

- `include_json!` and `include_yaml!` exist and their tests pass; every error case names the
  file and the key.
- `foundation_netio/data/mime.json` is generated from mime-db 1.54.0 by `generate_mime.py`, and
  `README.md` records the source, the version and the rule.
- `foundation_netio::shared::mime::{from_extension, from_path}` passes its tests, including the
  wasm check.
- `static_file.rs` has no extension map; the #578 test passes; the browser test server names its
  type from the table.
- `rg` finds no other extension map in `backends/`.
- The agreement test passes for 20 common extensions.
- The three crates carry the new versions, and every consumer requirement names them.
- CI is green on Linux, macOS and Windows.
- `requirements.md`, this file and `progress.md` are current.
