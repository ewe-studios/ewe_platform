# Progress: Spec 89, MIME Types

_Updated 2026-11-16. Written in ASD-STE100 Simplified Technical English (STE)._

## Where this specification is

| Stage | State |
|---|---|
| The owner's request | given 2026-11-16 during the review of spec 88 feature 00 |
| Survey and measured facts | done (requirements section 2) |
| Decisions | drafted (requirements section 6); four questions wait for the owner |
| Feature design | done: `features/01-mime-table/feature.md` |
| Owner approval of the plan | **approved 2026-11-16**: spec 89; the table is a `phf::Map`; an unknown name returns `None` |
| Implementation | **done 2026-11-16** on branch `spec/89-mime-types` |
| The Main Agent's review | **done 2026-11-16**: pass, with four findings recorded below |
| Pull request | draft #656 opened 2026-11-21; merge after the review fixes |

## The feature

| Feature | State | Model | Blocked by |
|---|---|---|---|
| 01-mime-table | `done` | sonnet | nothing: approved 2026-11-16 |

## What the plan depends on

| Assumption | State | Where it is measured |
|---|---|---|
| mime-db 1.54.0 is the standard source and holds 2,522 types | measured 2026-11-16 | requirements section 2 |
| mime-db gives 1,239 extensions, 48 of them ambiguous | measured 2026-11-16 | requirements section 2 |
| The rule in requirements R7 gives `mjs` and `js` the type `text/javascript` | measured | feature section 6.4 |
| `phf` 0.14 builds in a `no_std` crate | measured in the crate sources: `#![cfg_attr(not(feature = "std"), no_std)]`; `phf_shared` and `siphasher` are `no_std` | requirements section 2 |
| `serde_yaml_ng` 0.10.0 is the maintained YAML parser | read from the crate metadata | feature section 6.3 |

## The approval (2026-11-16)

| Question | The owner's answer | What the plan does |
|---|---|---|
| Where the plan lives | "Approve, spec 89" | One spec, one branch `spec/89-mime-types` off `master`, one pull request |
| The compile-time type | `phf::Map` | `foundation_nostd` re-exports `phf` 0.14 with `default-features = false`; the macro sets `phf_path` to `::foundation_nostd::phf` |
| An unknown name | `None` | `from_extension` and `from_path` return `Option`; `static_file.rs` uses the module's `OCTET_STREAM` constant |

The `no_std` question was measured before the approval: `phf` 0.14.0 signals
`#![cfg_attr(not(feature = "std"), no_std)]`, and `phf_shared` and `siphasher` match.

## The cost

- Three crates gain a dependency: `foundation_nostd` (phf), `foundation_netio` (phf and the
  macro), `foundation_macros` (phf_codegen and serde_yaml_ng).
- Three versions move: `foundation_nostd`, `foundation_netio`, `foundation_macros`.
- The vendored `mime.json` is about 1,250 rows, near 40 kilobytes.

## What landed in feature 01

| Task | Result |
|---|---|
| T1 `include_json!` | done; the duplicate-key and bad-input errors work; the proof is in `LEARNINGS.md` |
| T2 `include_yaml!` | done; a scalar key becomes its text; a duplicate key and a nested value fail |
| T3 the data file | done; `mime.json` holds 1,239 rows; the README and the generator are in `data/` |
| T4 `shared::mime` | done; `from_extension` and `from_path` pass; the wasm check passes |
| T5 the consumers | done; `static_file.rs` and the browser test server call the module |
| T6 consistency | done; the agreement test passes; the three versions moved |

The `cargo` verification block of feature section 8 is green, and so is
`cargo test -p foundation_browser --profile uat`.

## The review (2026-11-16)

The Main Agent read every new file and ran the suites again. The work matches the
"Done when" list of `features/01-mime-table/feature.md` section 9.

| Check | Command | Result |
|---|---|---|
| The macros | `cargo test -p foundation_macros --profile uat --test include_json --test include_yaml` | 2 and 3 cases pass |
| The module | `cargo test -p foundation_netio --profile uat --test mime_tests --test mime_data_tests --test mime_guess_agreement` | 8, 4 and 1 cases pass |
| Formatting | `cargo fmt --all -- --check` | pass |
| Clippy and the wasm check | `cargo clippy` on the five crates, then `cargo check -p foundation_netio --target wasm32-unknown-unknown --no-default-features --features wasm-fetch` | pass; no lint, and the crate checks for `wasm32` |
| No inline tests | `grep -rn '#\[cfg(test)\]'` on the two new files | none |
| The counts | `generate_mime.py` on a fresh v1.54.0 download | 2,522 types, 1,239 extensions, 48 ambiguous; the documents match |

The review measured the `mp4` question again. A rule that prefers a non-application
type inside one rank changes four rows: `mp4` and `mpg4` become `video/mp4`, `xml`
becomes `text/xml`, and `rtf` becomes `text/rtf`. The `rtf` row would go wrong,
because the Internet Assigned Numbers Authority (IANA) marks `text/rtf` as
deprecated. The review therefore recommends an override list instead of a new
tie-break. The record is in `LEARNINGS.md`.

## Open findings

| Issue | Severity | Summary |
|---|---|---|
| #591 | low | The plan's mime-db counts were wrong. Fixed in this branch. |
| #592 | medium | fixed 2026-11-21: `foundation_platform` now calls the shared table, with a test. |
| #593 | medium | fixed 2026-11-21: a reviewed override maps `mp4` and `mpg4` to `video/mp4`. |
| #595 | low | open: 22 manifests carry stale version requirements. It waits for its own change. |

## Next steps

1. Merge pull request #656 after its tests and the full continuous integration (CI) run.
2. Fix #595 in its own change, because both touch the same manifests.
3. The review of spec 88 feature 00 continues in its own track (spec 88 `progress.md`).

---

_Created: 2026-11-16_
