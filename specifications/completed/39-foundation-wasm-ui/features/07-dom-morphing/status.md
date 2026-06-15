# Feature 07 — Status: COMPLETE (2026-06-12)

## What shipped

`MorphDom` + `MorphContext` in `foundation-wasm-ui.js` (decision 027):

- **Four-phase reconciliation**: persistent-ID computation (tag-mismatch and
  duplicate exclusion), bottom-up ID maps over BOTH trees, child morphing with
  best-match scanning, script re-execution.
- **Matching**: priority 1 — ID-set intersection, scanning the remaining
  siblings AND the pantry (parked nodes are retrievable in the same morph);
  priority 2 — soft match guarded by an EQUALITY-LOOKAHEAD anti-churn rule.
- **Pantry** (G33): lazily created, parked nodes are id-bearing only, cleanup
  in `finally` — never leaks, even when a morph throws.
- **Form preservation**: input value/checked, textarea value, select
  selectedIndex; file inputs untouched; form controls are morph leaves.
- **`moveBefore`** with feature detection (G32) + structural fallback.
- **Escape hatches**: `data-ignore-morph` (both sides required),
  `data-preserve-attr` (comma list shields named attributes).
- **Scripts**: `executeNewScripts` clones-and-replaces with a `WeakSet` guard
  (browser-only; guarded on `querySelectorAll`).
- **Op 16 integration**: `DomOpApplicator`'s `ReplaceChildren` action now
  morphs (via `createRange().createContextualFragment`) when the document can
  parse HTML; innerHTML fallback otherwise (mocks).
- All node access is duck-typed, so the real DOM and the test mock run the
  same algorithm; the mock grew `childNodes`/`firstChild`/`nextSibling`/
  `isEqualNode`.

## Spec corrections (documented deviations)

The spec's §3 heuristics contradict its own test table; implemented to the
TESTS, which encode the intent:

1. **Displacement limit**: as written (`displaced > newIds.size` break) it
   blocks the spec's own reorder test 7 ([#a,#b,#c]→[#c,#a,#b] needs to scan
   past two anchors). ID matches are anchors and always worth moving for —
   the ID scan is unbounded (sibling run + pantry). The spec's
   displacement EXAMPLE (W with no ids) never enters the ID scan and is
   handled by anti-churn instead. Both spec examples pass.
2. **Soft-match anti-churn**: the future-sibling counter (`>= 2 blocks`)
   rejects every element of any homogeneous list ≥3 — including the B/C/D the
   spec's own narrative says must match. Replaced with an equality lookahead:
   if the NEXT new sibling `isEqualNode`s the cursor, the current new child is
   an insertion → create fresh. Prepend (test 13), append (test 2), in-place
   morphs (tests 1/4), and the displacement example (test 15) all behave as
   the spec narrates.

## Verification

16 morph tests on the mock DOM (spec rows 1-4, 7-11, 13-21, 23, 27-29, 31):
identity reuse, attr sync incl. removal, ID reorders/moves with zero creation,
tag-mismatch + duplicate exclusion, pantry park/retrieve, prepend anti-churn,
displacement-by-anti-churn, 100-item move (99 kept + 1 moved, all identical
references), full form preservation, both escape hatches, empty-content clear.
Script tests (24-26) are browser-only by nature — the guard + clone logic ship;
exercised via the web testbed when needed. Full wasm_ui JS suite 48/48.
