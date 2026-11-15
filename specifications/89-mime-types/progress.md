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
| Implementation | starting: one sonnet agent in its own worktree, branch `spec/89-mime-types` |
| Pull request | none |

Product code changes need the owner's approval of the plan. Nothing is implemented yet.

## The feature

| Feature | State | Model | Blocked by |
|---|---|---|---|
| 01-mime-table | `in progress` | sonnet | nothing: approved 2026-11-16 |

## What the plan depends on

| Assumption | State | Where it is measured |
|---|---|---|
| mime-db 1.54.0 is the standard source and holds 2,601 types | measured | requirements section 2 |
| mime-db gives 1,246 extensions, 56 of them ambiguous | measured | requirements section 2 |
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

## Next steps

1. One sonnet agent does T1 to T6 on branch `spec/89-mime-types`, in its own git worktree.
2. The Main Agent reviews against "Done when", then opens the pull request.
3. The review of spec 88 feature 00 continues in its own track (spec 88 `progress.md`).

---

_Created: 2026-11-16_
