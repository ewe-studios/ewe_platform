# 027 — DOM Morphing: `MorphDom` class based on Datastar's approach

**Date:** 2026-06-08
**Status:** Resolved

### Decision

A `MorphDom` class in the JS runtime handles server/WASM HTML patches with form preservation, element identity tracking, and move detection. Based on Datastar's approach (which combines morphdom + idiomorph).

### Algorithm: Four phases

```
Phase 1: Build persistent ID set  → IDs in both old & new with same tag
Phase 2: Populate ID maps          → Map each ancestor to persistent IDs in subtree
Phase 3: Morph children             → Walk new content, match old, reconcile
Phase 4: Cleanup                    → Remove pantry, clear maps
```

### `MorphDom` class

```js
class MorphDom {
    // Module-level singleton state, reused across calls
    static idMap = new Map()         // Node → Set<persistent IDs>
    static persistentIds = new Set() // IDs in both old & new
    static oldIdTagMap = new Map()   // ID → tagName
    static duplicates = new Set()    // Duplicate IDs
    static pantry = createPantry()   // Hidden element for parked nodes
    static scripts = new WeakSet()   // Track executed scripts

    // Public API
    static morph(target, newContent) {
        this._computePersistentIds(target, newContent)
        this._populateIdMaps(target)
        this._morphChildren(target, newContent)
        this._executeNewScripts(target)
        this._cleanup()
    }

    // Matching priority system
    static _findBestMatch(cursor, newChild) {
        // Priority 1: ID set match (same type + tag + overlapping subtree IDs)
        // Priority 2: Soft match (same type + tag, old has no conflicting ID)
        // Anti-churn: block soft match if 2+ future siblings would match
        // Displacement limit: abort if displaced IDs > new node's ID count
    }

    // The pantry pattern
    static _removeNode(node) {
        if (this.idMap.has(node)) {
            moveBefore(this.pantry, node, null)  // park (still in document)
        } else {
            node.parentNode?.removeChild(node)   // actually remove
        }
    }

    // Form state preservation
    static _preserveFormState(oldEl, newEl) {
        const preserve = { value, checked, selectedIndex }
        for (const key of preserve) {
            if (oldEl[key] !== undefined && oldEl[key] !== newEl[key]) {
                newEl[key] = oldEl[key]
            }
        }
    }
}
```

### Features inherited from Datastar

| Feature | Purpose |
|---------|---------|
| **Persistent ID set** | Anchors — IDs in both old & new with same tag are preserved |
| **Ancestor ID map** | Enables move detection (reorder lists without morphing) |
| **Pantry pattern** | Park nodes that might be needed later (still in DOM for getElementById) |
| **Anti-churn protection** | Block soft match if 2+ future siblings would match (prevents prepend churn) |
| **Displacement limit** | Bounds forward scan — abort if too many IDs would be displaced |
| **`isEqualNode` skip** | Skip entire subtree if structurally identical |
| **`moveBefore()` API** | Preserves iframe state, animations, web component lifecycle |
| **Script execution tracking** | `WeakSet` — new scripts cloned (execute), old scripts not re-run |
| **Form state preservation** | input/textarea/select/option values preserved during morph |
| **`data-ignore-morph`** | Both old & new must carry it — escape hatch for third-party widgets |
| **`data-preserve-attr`** | List of attribute names the morph won't overwrite |

### Form preservation operations

Additional form-specific operations built on top of the morph:

- **Input value preservation** — text, number, email, password, etc.
- **Checkbox/radio checked state** — preserved even if `checked` attribute changes
- **Select dropdown selectedIndex** — user's selection preserved
- **Textarea content** — user edits preserved
- **File input** — cannot be preserved programmatically (browser security), but morph doesn't reset it

### Integration with Arrow batch

When the server sends an Arrow batch with `MORPH_NODE` (ID 16) or `REPLACE_NODE` (ID 12) operations (decision 010):

```js
// ArrowDomApplicator handles morph ops
case 16:  // MORPH_NODE
    const target = nodeRegistry.get(nodeId)
    const html = textVals[i]  // new HTML content
    const fragment = document.createRange().createContextualFragment(html)
    MorphDom.morph(target, fragment)
    break

case 12:  // REPLACE_NODE
    const oldEl = nodeRegistry.get(nodeId)
    const newEl = valueToElement(vals[i])  // reconstruct from serialized new_id
    oldEl.parentNode.replaceChild(newEl, oldEl)
    nodeRegistry.unregister(oldEl)
    nodeRegistry.register(newEl)
    break
```

### Why this design

- **Proven algorithm** — morphdom + idiomorph battle-tested in production
- **Datastar improvements** — persistent ID set, pantry, anti-churn are real additions
- **Form preservation** — critical for UX, morphing shouldn't reset user input
- **Module-level state** — reduces GC pressure for frequent morphing
- **Clean integration** — fits into Arrow batch pipeline as an operation type
