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
| Pull request | not open yet |

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

## Open findings

| Issue | Severity | Summary |
|---|---|---|
| #591 | low | The plan's mime-db counts were wrong. Fixed in this branch. |
| #592 | medium | `foundation_platform/src/responder.rs` keeps a second extension map. |
| #593 | medium | Rule R7 maps `mp4` to `application/mp4`, not `video/mp4`. |

## Next steps

1. The Main Agent reviews against "Done when", then opens the pull request.
2. The full continuous integration (CI) run proves Linux, macOS and Windows.
3. The review of spec 88 feature 00 continues in its own track (spec 88 `progress.md`).

---

_Created: 2026-11-16_
