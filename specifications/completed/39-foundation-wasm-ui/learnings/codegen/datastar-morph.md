# Datastar DOM Morphing — Learnings

## Source

Datastar's `patchElements.ts` — a single-file, ~710-line DOM morphing implementation built directly into the SSE watcher plugin. Draws from morphdom and idiomorph but adds Datastar-specific features: pantry pattern, `data-preserve-attr`, `data-ignore-morph`, `data-scope-children`, view transitions.

---

## 1. Algorithm Overview — Four Phases

```
Phase 1: Build Persistent ID Set   → IDs that exist in both old & new with same tag
Phase 2: Populate ID Maps          → Map each ancestor to persistent IDs in its subtree
Phase 3: Morph Children            → Walk new content, match old content, reconcile
Phase 4: Cleanup                   → Remove pantry, clear maps
```

The key innovation is the **persistent ID set** — IDs that exist in both the old and new content with the same tag name. These are the anchors the algorithm uses to preserve DOM state.

---

## 2. Persistent ID Computation

```
Step 1: Collect all IDs from old content
  - Track ID → tagName mapping
  - If duplicate ID in old → mark as duplicate (exclude)

Step 2: Intersect with new content
  - If ID exists in old AND new AND same tagName → persistent
  - If duplicate in new → mark as duplicate (exclude)

Step 3: Remove duplicates from persistent set
```

**Why tag name matters**: An ID that maps to `<div>` in old content and `<span>` in new can't be morphed — the element type changed. Excluding it means the old element is removed and the new one created fresh.

**Why duplicates are excluded**: The algorithm can't reason about which duplicate is "the right one," so all copies are treated as non-persistent.

---

## 3. ID Map — Ancestor Annotation

For each persistent ID, walk up the ancestor chain and annotate each ancestor:

```typescript
ctxIdMap: Map<Node, Set<string>>
// Each node maps to the set of persistent IDs in its subtree
```

Example:
```
<ul>                    ← idSet: {"item-1", "item-2", "item-3"}
  <li id="item-1">     ← idSet: {"item-1"}
  <li id="item-2">     ← idSet: {"item-2"}
</ul>
```

This map enables the algorithm to detect **moves** (item-2 moved up) and **removals** (item-3 disappeared), rather than treating every position as changed.

---

## 4. The Core Loop — `morphChildren()`

Walks new content's children left to right. For each `newChild`, tries to find the best matching old child starting from `insertionPoint`:

### Case 1: Match Found in Forward Scan

`findBestMatch()` scans forward. If found, nodes between `insertionPoint` and the match are removed, then the match is morphed in place.

### Case 2: Persistent ID Element Not in Scan Range

If `newChild` has a persistent ID but wasn't found by the scan, use `getElementById()` to find it (it might be elsewhere in the DOM or in the pantry). Clean ancestor ID maps, then use `moveBefore()` to relocate it.

```typescript
const movedChild = document.getElementById(newChild.id)
// Clean ancestor ID maps
let current = movedChild
while ((current = current.parentNode)) {
  const idSet = ctxIdMap.get(current)
  if (idSet) {
    idSet.delete(newChild.id)
    if (!idSet.size) ctxIdMap.delete(current)
  }
}
moveBefore(oldParent, movedChild, insertionPoint)
morphNode(movedChild, newChild)
```

### Case 3: New Element with Persistent ID Children (Dummy Pattern)

If `newChild` doesn't have a persistent ID but its subtree contains persistent IDs, create an empty element of the same type, insert it, then recursively morph it. This recursively applies the matching algorithm, which eventually finds and moves the persistent children.

```typescript
const newEmptyChild = document.createElement(tagName)
oldParent.insertBefore(newEmptyChild, insertionPoint)
morphNode(newEmptyChild, newChild)  // fills the empty shell
```

### Case 4: Pure New Element (Fast Path)

If neither the element nor any descendants have persistent IDs, there's no state to preserve. Deep-clone and insert directly — no morphing.

```typescript
const newClonedChild = document.importNode(newChild, true)
oldParent.insertBefore(newClonedChild, insertionPoint)
```

---

## 5. Matching Algorithm — Priority System

```
Priority 1 (highest): ID Set Match
  - Same type + tag + compatible ID
  - AND their subtree ID sets overlap (share a persistent ID)
  → Return immediately

Priority 2: Soft Match (fallback)
  - Same nodeType + tagName
  - Old node has no ID, or old ID matches new ID
  - Old node has no pending ID set matches
  → Saved as bestMatch, returned if no ID set match found

Not a match:
  - Different nodeType or tagName
  - Old node has an ID that differs from new node's ID
```

### Soft Match

```typescript
const isSoftMatch = (oldNode, newNode): boolean =>
  oldNode.nodeType === newNode.nodeType &&
  oldNode.tagName === newNode.tagName &&
  (!oldNode.id || oldNode.id === newNode.id)
```

**The asymmetry is intentional**: An old node with no ID can match a new node with an ID (the new ID will be added during morphing). But an old node with a *different* ID is never soft-matched — it would lose its identity.

### Anti-Churn Protection

When scanning, the algorithm checks if upcoming new siblings would soft-match the current old node. If 2+ future siblings would match, it blocks the current soft match to prevent unnecessary DOM churn when an element is prepended:

```typescript
if (bestMatch === null && nextSibling && isSoftMatch(cursor, nextSibling)) {
  siblingSoftMatchCount++
  if (siblingSoftMatchCount >= 2) {
    bestMatch = undefined  // block soft matches
  }
}
```

### Displacement Limit

The scan tracks how many persistent IDs would be "displaced" (skipped over) to reach a potential match. If the displacement exceeds the new node's own ID count, the search is aborted:

```typescript
displaceMatchCount += ctxIdMap.get(cursor)?.size || 0
if (displaceMatchCount > nodeMatchCount) break
```

This prevents scanning the entire sibling list looking for a match that would require rearranging too many elements.

---

## 6. The Pantry Pattern

### The Problem

Old nodes that don't match may need to be "removed." But some of those nodes contain persistent IDs that will be needed later when processing subsequent new children. If we `removeChild()` immediately, `getElementById()` won't find them.

### The Solution

```typescript
const ctxPantry = document.createElement('div')
ctxPantry.hidden = true
document.body.insertAdjacentElement('afterend', ctxPantry)

const removeNode = (node: Node): void => {
  ctxIdMap.has(node)
    ? moveBefore(ctxPantry, node, null)   // park it (still in document)
    : node.parentNode?.removeChild(node)   // actually remove it
}
```

Nodes moved to the pantry are **still in the document** (so `getElementById()` finds them), but placed after `<body>` to avoid MutationObserver firing. After morphing completes, the pantry is removed.

### Why After `<body>`

If the pantry were inside `<body>`, the MutationObserver would fire for nodes being moved in/out, potentially triggering attribute processing and cleanup hooks. Placing it as a sibling of `<body>` keeps it in the document tree but outside the observed subtree.

---

## 7. Node Morphing — `morphNode()`

Once two nodes are matched, synchronize the old node to match the new one:

### Element Nodes

1. **Check `data-ignore-morph`** — if both old and new have it, skip entirely
2. **Read `data-preserve-attr`** — attributes listed here won't be overwritten
3. **Handle form element state** — preserve input/textarea/select/option values
4. **Sync attributes** — add/update from new, remove missing from old (respecting preserve-attr)
5. **Dispatch 'change' event** if form state changed
6. **Preserve `data-scope-children`** — re-add if new content removed it
7. **`<template>` elements** — set `innerHTML` directly (no recursive morph)
8. **`isEqualNode` optimization** — if elements are structurally identical, skip entire subtree
9. **`morphChildren`** — recursive child morphing

### Text and Comment Nodes

```typescript
if (oldNode.nodeValue !== newNode.nodeValue) {
  oldNode.nodeValue = newNode.nodeValue
}
```

### The `isEqualNode` Optimization

Before recursing into children, check `oldElt.isEqualNode(newElt)`. If structurally identical (same attributes, same children, same text), the entire subtree is skipped. Significant optimization for large DOMs where only a small region changed.

---

## 8. Script Handling

Scripts inserted via morphing don't execute (browser only runs scripts created via `createElement` + `appendChild`).

**Solution**: Track all existing scripts in a `WeakSet`. On morph, any new script elements are replaced with fresh clones (which triggers execution), then tracked. The `WeakSet` ensures already-executed scripts aren't re-run on subsequent morphs.

```typescript
const scripts = new WeakSet<HTMLScriptElement>()
// Pre-populate
for (const script of document.querySelectorAll('script')) scripts.add(script)

// On morph
for (const old of elScripts) {
  if (!scripts.has(old)) {
    const script = document.createElement('script')
    for (const { name, value } of old.attributes) script.setAttribute(name, value)
    script.text = old.text
    old.replaceWith(script)    // fresh clone → executes
    scripts.add(script)
  }
}
```

---

## 9. The `moveBefore` API

```typescript
const moveBefore = removeNode.call.bind(
  ctxPantry.moveBefore ?? ctxPantry.insertBefore
)
```

`moveBefore()` (Chromium flag at time of writing) moves an element without destroying/recreating it. Unlike `insertBefore` (which triggers disconnect/reconnect lifecycle), `moveBefore` preserves:
- Iframe content and navigation state
- CSS animations and transitions mid-flight
- Web component connected/disconnected callback suppression
- `<video>`/`<audio>` playback state

Falls back to `insertBefore` when not available.

---

## 10. Module-Level State (Singleton Optimization)

The algorithm uses module-level (singleton) data structures, reused across calls:

```typescript
const ctxIdMap = new Map<Node, Set<string>>()
const ctxPersistentIds = new Set<string>()
const oldIdTagNameMap = new Map<string, string>()
const duplicateIds = new Set<string>()
const ctxPantry = document.createElement('div')
```

These are cleared at the start of each `morph()` call. Using module-level state avoids allocation churn — these structures would otherwise be created and GC'd on every morph. Datastar expects frequent morphing (every SSE event), so reducing GC pressure matters.

---

## 11. Special Features

### `data-ignore-morph`

Both old and new must carry the attribute — this prevents accidental preservation when the server intentionally wants to replace content. Both old node and ancestors are checked.

Use cases: third-party widgets, video players, canvases with complex internal state.

### `data-preserve-attr`

List of attribute names to preserve during morphing. The server's new content won't overwrite these attributes.

### `data-scope-children`

When present, the morph algorithm preserves this marker even if incoming markup doesn't carry it. After morphing, a non-bubbling `datastar:scope-children` event is dispatched, allowing parent components to react to child content changes.

---

## 12. Algorithmic Complexity

| Operation | Complexity | Notes |
|-----------|-----------|-------|
| Persistent ID computation | O(n) | Two querySelectorAll + set intersection |
| ID map population | O(n × d) | n = persistent IDs, d = tree depth |
| `morphChildren` | O(n × m) worst case | n = new children, m = old children |
| `findBestMatch` | O(m) per call | Forward scan with early termination |
| `morphNode` | O(a) | a = number of attributes |
| Overall | O(n × m + a) | Dominated by child matching |

**In practice, it's fast because**:
1. ID set matches terminate immediately
2. Soft matches without IDs return immediately
3. Displacement limit bounds the forward scan
4. `isEqualNode` skips unchanged subtrees
5. Fast path (clone + insert) avoids recursion for new elements without persistent IDs

---

## 13. Relevance to Spec-39

### What We Learn

1. **Persistent ID set is the anchor** — unlike R3's signal-based approach, DOM morphing needs to know which elements to preserve. The persistent ID set (intersection of old + new IDs with same tag) is the most efficient way to determine this.

2. **The pantry pattern is critical** — without it, elements that need to be "moved" (not removed) become unreachable. Our WASM morph module should implement the same pattern: a hidden container for temporarily parked elements.

3. **Ancestor ID map enables move detection** — the `ctxIdMap` (node → persistent IDs in subtree) is what makes the algorithm smarter than positional diff. Without it, reordering a list would morph every element instead of moving them.

4. **Anti-churn protection matters** — the sibling soft-match count prevents unnecessary morphing when items are prepended. Our morph should include this.

5. **Displacement limit bounds worst case** — without it, a single new element could trigger a full scan of all old siblings. The limit (displaced IDs > new node's ID count) is a good heuristic.

6. **`isEqualNode` as subtree skip** — before recursing into children, check if the subtrees are identical. This is a cheap optimization for large DOMs with small changes.

7. **Module-level singleton state** — reuse the ID map and sets across morph calls. This reduces GC pressure for frequent morphing. In Rust, we'd use `thread_local!` or a `OnceLock` for this.

8. **Form state preservation** — the algorithm preserves input/textarea/select/option values during morphing. Our morph should do the same, or better yet, our signal system should handle form state separately.

9. **Script execution tracking** — the `WeakSet` approach for script execution is simple and effective. Our morph should handle scripts the same way.

10. **`moveBefore` API** — when available, use `moveBefore()` instead of `insertBefore()` for element relocation. This preserves iframe state, animations, and web component lifecycle.

### What Datastar Doesn't Do

- No height-based ordering (unlike R3/Svelte) — morphing is purely DOM-based, no dependency graph
- No lazy evaluation — the entire morph runs synchronously
- No signal integration — morphing is triggered by SSE events, not by reactive state changes

### What Our Spec-39 Should Incorporate

- Persistent ID set computation (same as Datastar — intersection of old/new IDs)
- Ancestor ID map for move detection
- Pantry pattern for temporarily parked elements
- Anti-churn protection (sibling soft-match count ≥ 2)
- Displacement limit for forward scan
- `isEqualNode` subtree skip optimization
- Module-level singleton state (reused across morph calls)
- `moveBefore()` API with `insertBefore()` fallback
- Script execution tracking via WeakSet
- `data-ignore-morph` escape hatch
- Form state preservation during morphing
