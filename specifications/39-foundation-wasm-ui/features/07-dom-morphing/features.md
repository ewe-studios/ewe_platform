# Feature 07: DOM Morphing

`MorphDom` class in the JS runtime for server/WASM HTML patches with form preservation,
element identity tracking, and move detection. Based on Datastar's approach (morphdom +
idiomorph). Invoked by Arrow MORPH_NODE (op 16).

**Decision:** 027

---

## 1. Types and Static Fields

```javascript
class MorphDom {
    static idMap         = new Map()         // Map<Node, Set<string>>  — node to persistent IDs in subtree
    static persistentIds = new Set()         // Set<string>            — IDs in both old & new with same tag
    static oldIdTagMap   = new Map()         // Map<string, string>    — ID to tagName in old tree
    static duplicates    = new Set()         // Set<string>            — IDs appearing more than once
    static pantry        = createPantry()    // HTMLElement            — hidden div for parked nodes
    static scripts       = new WeakSet()     // WeakSet<HTMLScriptElement> — executed scripts
}
```

`createPantry()` appends a hidden `<div style="display:none">` to `document.body`.
Parked nodes stay in the document (queryable via `getElementById`) but invisible.

**Module:** `crates/foundation_wasm_ui/assets/foundation-wasm-ui.js`

---

## 2. Algorithm — Four Phases

Entry: `MorphDom.morph(target, newContent)` calls phases 1-4 then `_cleanup()`.

### Phase 1 — `_computePersistentIds(oldRoot, newRoot)`

```
1. Clear persistentIds, oldIdTagMap, duplicates.
2. Walk old tree (TreeWalker, SHOW_ELEMENT):
   For each element with an `id`:
     If oldIdTagMap already has this ID -> add to duplicates.
     Else -> oldIdTagMap.set(id, element.tagName).
3. Walk new tree (TreeWalker, SHOW_ELEMENT):
   For each element with an `id`:
     If duplicates.has(id) -> skip.
     If oldIdTagMap.has(id) AND oldIdTagMap.get(id) === element.tagName:
       -> persistentIds.add(id).
```

### Phase 2 — `_populateIdMaps(root)`

Bottom-up map: each ancestor to the set of persistent IDs in its subtree.

```
1. Clear idMap. Collect all elements via TreeWalker into array.
2. Iterate in reverse (leaves first):
   a. Let ids = new Set().
   b. If element has id AND persistentIds.has(id) -> ids.add(id).
   c. For each child: if idMap.has(child) -> merge child's set into ids.
   d. If ids.size > 0 -> idMap.set(element, ids).
```

### Phase 3 — `_morphChildren(oldParent, newParent)`

```
 1. Let oldCursor = oldParent.firstChild.
 2. For each newChild in newParent.childNodes:
    a. IGNORE CHECK: if both old match and newChild have data-ignore-morph,
       advance oldCursor past it. Continue.
    b. Let match = _findBestMatch(oldCursor, newChild).
    c. IF match:
       - Remove/park all old nodes between oldCursor and match (_removeNode each).
       - If match !== oldCursor: moveBefore(oldParent, match, oldCursor).
       - _morphNode(match, newChild).
       - oldCursor = match.nextSibling.
    d. IF no match:
       - Clone newChild (deep), insertBefore(clone, oldCursor).
 3. Remove/park all remaining old children from oldCursor to end.
```

### `_morphNode(oldNode, newNode)`

```
1. Text/comment nodes: update nodeValue if different. Return.
2. Elements:
   a. isEqualNode(newNode) -> return (skip subtree).
   b. _syncAttributes(oldNode, newNode).
   c. _preserveFormState(oldNode, newNode).
   d. If INPUT|TEXTAREA|SELECT -> skip children (form elements are leaves).
   e. Else -> _morphChildren(oldNode, newNode) (recurse).
```

### `_syncAttributes(oldEl, newEl)`

```
1. Let preserved = (oldEl.dataset.preserveAttr || "").split(",").map(trim).
2. For each attr in newEl.attributes:
   If !preserved.includes(attr.name) -> oldEl.setAttribute(attr.name, attr.value).
3. For each attr in oldEl.attributes:
   If !preserved.includes(attr.name) AND !newEl.hasAttribute(attr.name)
     -> oldEl.removeAttribute(attr.name).
```

---

## 3. Matching Priority System — `_findBestMatch`

```
_findBestMatch(oldCursor, newChild):
    If newChild is not Element -> return softMatch(oldCursor, newChild).
    Let newIds = idMap.get(newChild) || EMPTY_SET.
    Let displacedCount = 0, candidate = oldCursor.

    // --- Priority 1: ID set match (scan forward) ---
    While candidate !== null:
        If candidate is Element AND candidate.tagName === newChild.tagName
           AND idMap.has(candidate)
           AND intersection(idMap.get(candidate), newIds).size > 0:
            Return candidate.
        If idMap.has(candidate):
            displacedCount += idMap.get(candidate).size.
        If displacedCount > newIds.size: Break.  // displacement limit
        candidate = candidate.nextSibling.

    // --- Priority 2: Soft match ---
    If oldCursor !== null AND oldCursor.nodeType === newChild.nodeType
       AND oldCursor.tagName === newChild.tagName
       AND !_hasConflictingId(oldCursor):
        // Anti-churn: count future siblings that also soft-match
        Let futureMatches = 0, sib = oldCursor.nextSibling.
        While sib !== null:
            If sib.nodeType === newChild.nodeType AND sib.tagName === newChild.tagName
               AND !_hasConflictingId(sib): futureMatches++.
            If futureMatches >= 2: Break.
            sib = sib.nextSibling.
        If futureMatches >= 2: Return null.  // block soft match
        Return oldCursor.

    Return null.
```

`_hasConflictingId(node)`: true if node has `id` not in `persistentIds`.

### Anti-churn example (prepend)

Old: `[B, C, D]` (no IDs). New: `[A, B, C, D]`. Without anti-churn, A soft-matches B,
B soft-matches C, C soft-matches D. Every node morphs into its successor. With
anti-churn: matching A against old-B, future siblings C and D also soft-match
(`futureMatches >= 2`), so soft match is blocked. A created fresh. B, C, D match themselves.

### Displacement limit example

Old: `[X#1, Y#2, Z#3, W]`. New: `[W, X#1]`. Matching W: scan forward, X#1 displaces 1,
Y#2 displaces 1, Z#3 displaces 1. `displacedCount=3 > newIds.size(W)=0`. Scan aborts.
W created fresh rather than pulling past three anchored nodes.

---

## 4. Pantry Pattern

**Create:** `createPantry()` returns hidden `<div>` appended to `document.body`.

**Park (`_removeNode`):**
```
If idMap.has(node):
    moveBefore(pantry, node, null)     // park — still in document
Else:
    node.parentNode?.removeChild(node) // truly remove
```

**Retrieve:** `_findBestMatch` can match parked nodes. When matched, `moveBefore`
repositions from pantry back into the live tree.

**Cleanup (`_cleanup`):**
```
While pantry.firstChild: pantry.removeChild(pantry.firstChild)  // remove unclaimed
idMap.clear(); persistentIds.clear(); oldIdTagMap.clear(); duplicates.clear()
```

---

## 5. Form State Preservation

| Element | Property | Behavior |
|---------|----------|----------|
| `<input type="text\|number\|email\|password\|search\|url\|tel">` | `value` | Copy oldEl.value to newEl.value |
| `<input type="checkbox">` | `checked` | Copy oldEl.checked to newEl.checked |
| `<input type="radio">` | `checked` | Copy oldEl.checked to newEl.checked |
| `<input type="file">` | (none) | Cannot set programmatically. Element reused via matching. |
| `<textarea>` | `value` | Copy oldEl.value (reflects user edits, not textContent) |
| `<select>` | `selectedIndex` | Copy oldEl.selectedIndex to newEl.selectedIndex |

```
_preserveFormState(oldEl, newEl):
    If tags differ -> return.
    Switch oldEl.tagName:
        "INPUT": if checkbox/radio -> newEl.checked = oldEl.checked
                 else if not file  -> newEl.value = oldEl.value
        "TEXTAREA": newEl.value = oldEl.value
        "SELECT":   newEl.selectedIndex = oldEl.selectedIndex
```

---

## 6. `moveBefore()` and Fallback

Moves a node without disconnect/reconnect lifecycle. Preserves iframe state, CSS
animations, web component lifecycle, focus.

```
moveBefore(parent, node, ref):
    If parent.moveBefore: parent.moveBefore(node, ref)
    Else: parent.removeChild(node); parent.insertBefore(node, ref)  // fallback
```

Feature detection at module load. Fallback is correct but loses state guarantees.

---

## 7. Script Execution

```
_executeNewScripts(root):
    For each <script> in root.querySelectorAll("script"):
        If scripts.has(script) -> continue.
        Let clone = document.createElement("script").
        Copy all attributes. clone.textContent = script.textContent.
        script.parentNode.replaceChild(clone, script).
        scripts.add(clone).
```

WeakSet prevents re-execution. GC'd scripts auto-clear from WeakSet.

---

## 8. Escape Hatches

**`data-ignore-morph`:** Both old AND new must carry it. Old element left untouched.
Use case: third-party widgets (maps, editors, video players).

**`data-preserve-attr`:** Comma-separated attribute names on old element. Those
attributes skip sync. Example: `<div data-preserve-attr="style,class">` keeps style
and class unchanged while other attributes morph normally.

---

## 9. Arrow Integration

**MORPH_NODE (op 16):** Intelligent reconciliation with form preservation.
```javascript
case 16: {
    const target = this.nodes.get(nid);
    const fragment = document.createRange().createContextualFragment(textVals[i]);
    MorphDom.morph(target, fragment);
    break;
}
```

**REPLACE_NODE (op 12):** Hard replacement. No form preservation, no identity tracking.
Old node and subtree removed entirely.

---

## 10. Error Cases and Edge Cases

| Scenario | Behavior |
|----------|----------|
| target is null (node_id missing) | `nodes.get()` returns undefined, throws TypeError |
| Empty fragment | All old children removed/parked. Target becomes empty. |
| Duplicate IDs in old tree | Added to duplicates, excluded from persistentIds. Soft matching fallback. |
| Different root tag | Children reconciled; target element itself never replaced. |
| `<template>` elements | Content in `.content` fragment. Morph treats as opaque. |
| `<svg>`/`<math>` namespaced | Existing elements reused via matching. New from fragment have correct namespace. |
| `data-ignore-morph` one side | Ignored. Both sides required. Element morphed normally. |
| Deep nesting (>100 levels) | Recursive _morphChildren may hit stack. Practical HTML rarely exceeds 30. |

---

## 11. Integration Points

| Feature | Relationship |
|---------|-------------|
| F05 (Arrow Encoding) | Op 16 sends HTML in text_val. ArrowDomApplicator dispatches to MorphDom. |
| F06 (Web Components) | Patcher may invoke MorphDom for incremental updates. |
| F00 (JS Runtime) | MorphDom defined in foundation-wasm-ui.js, shares document and nodeRegistry. |
| F08 (Event Runtime) | Listeners on matched elements survive. New elements need Arrow ops 6-7. |

---

## 12. File Ownership

| File | Owns |
|------|------|
| `crates/foundation_wasm_ui/assets/foundation-wasm-ui.js` | MorphDom class, createPantry(), moveBefore() polyfill |

All morphing logic is JS-only. No Rust code.

---

## 13. Refactoring Strategy

1. Implement `createPantry()` and `moveBefore()` with fallback. Unit test both paths.
2. Implement `_computePersistentIds` and `_populateIdMaps`. Test with known ID sets.
3. Implement `_findBestMatch` with priorities, anti-churn, displacement limit.
4. Implement `_morphChildren` and `_morphNode` with `_syncAttributes`.
5. Implement `_preserveFormState` for all form element types.
6. Implement `_executeNewScripts` with WeakSet tracking.
7. Add escape hatches (`data-ignore-morph`, `data-preserve-attr`).
8. Wire into ArrowDomApplicator case 16. End-to-end test.

---

## 14. Testing

### Core Morph (tests 1-6)

| # | Scenario | Verify |
|---|----------|--------|
| 1 | Simple text change: `<p>old</p>` -> `<p>new</p>` | textContent updated, same element reused |
| 2 | Add child to `<ul>` | New `<li>` appended, existing `<li>` unchanged |
| 3 | Remove child from `<ul>` | Child removed from DOM, sibling unchanged |
| 4 | Attribute sync: `class="a"` -> `class="b"` | className updated, same element |
| 5 | isEqualNode skip: identical subtrees | No attribute writes, no child walks |
| 6 | Nested 3-level morph | Correct at all levels, minimal creation |

### Identity and Move Detection (tests 7-12)

| # | Scenario | Verify |
|---|----------|--------|
| 7 | Reorder `[#a,#b,#c]` -> `[#c,#a,#b]` | Same 3 elements reordered, zero creation |
| 8 | Move node with subtree to new position | Parent and children are same elements |
| 9 | ID tag mismatch: `<div id="x">` vs `<span id="x">` | ID excluded, div removed, span created |
| 10 | Duplicate ID in old tree | Excluded from persistentIds, soft match used |
| 11 | Pantry park then retrieve in same morph | Same element reused from pantry |
| 12 | Pantry cleanup: unclaimed parked node | Removed during cleanup phase |

### Anti-churn and Displacement (tests 13-16)

| # | Scenario | Verify |
|---|----------|--------|
| 13 | Prepend no IDs: `[B,C,D]` -> `[A,B,C,D]` | A new, B/C/D same elements |
| 14 | Prepend with IDs: `[#b,#c]` -> `[#a,#b,#c]` | #a created, #b/#c matched by ID |
| 15 | Displacement limit triggers | Scan aborts, node created instead of moved |
| 16 | 100-item list, first moved to last | 99 matched, 1 moved |

### Form Preservation (tests 17-23)

| # | Scenario | Verify |
|---|----------|--------|
| 17 | Text input: user types "hello", morph changes placeholder | value === "hello" |
| 18 | Checkbox: user checks, morph updates label | checked === true |
| 19 | Radio: user selects, morph adds option | selected radio still checked |
| 20 | Select: user picks option 2, morph updates text | selectedIndex === 2 |
| 21 | Textarea: user edits, morph changes wrapper | value preserved |
| 22 | File input: user selects file, morph updates sibling | file not reset |
| 23 | Full form: mixed inputs, morph changes heading | all values preserved |

### Script Execution (tests 24-26)

| # | Scenario | Verify |
|---|----------|--------|
| 24 | New inline script added | Cloned and executed |
| 25 | Existing script on re-morph | In WeakSet, not re-executed |
| 26 | External script src added | Cloned, fetches and executes |

### Escape Hatches (tests 27-29)

| # | Scenario | Verify |
|---|----------|--------|
| 27 | `data-ignore-morph` both sides | Element untouched |
| 28 | `data-ignore-morph` one side only | Element morphed normally |
| 29 | `data-preserve-attr="style"`, new changes style | style unchanged, others synced |

### Arrow Integration (tests 30-32)

| # | Scenario | Verify |
|---|----------|--------|
| 30 | MORPH_NODE end-to-end via Arrow batch | DOM matches new HTML, form state preserved |
| 31 | MORPH_NODE with empty HTML string | Target children removed |
| 32 | REPLACE_NODE vs MORPH_NODE same update | Op 12: hard replace. Op 16: reuse where possible. |
