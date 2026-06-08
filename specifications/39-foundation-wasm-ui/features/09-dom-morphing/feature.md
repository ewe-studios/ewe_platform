# Feature 09: DOM Morphing

## Description

`MorphDom` class in the JS runtime for server/WASM HTML patches with form preservation, element identity tracking, and move detection. Based on Datastar's approach (morphdom + idiomorph). Invoked by Arrow MORPH_NODE (ID 16).

**Decisions:** 027

## Module

`crates/foundation_wasm_ui/assets/foundation-wasm-ui.js` (MorphDom class)

## Algorithm: Four phases

```
Phase 1: Build persistent ID set  → IDs in both old & new with same tag
Phase 2: Populate ID maps          → Map each ancestor to persistent IDs in subtree
Phase 3: Morph children             → Walk new content, match old, reconcile
Phase 4: Cleanup                    → Remove pantry, clear maps
```

## Class

```javascript
class MorphDom {
    static idMap = new Map()         // Node → Set<persistent IDs>
    static persistentIds = new Set() // IDs in both old & new
    static oldIdTagMap = new Map()   // ID → tagName
    static duplicates = new Set()    // Duplicate IDs
    static pantry = createPantry()   // Hidden element for parked nodes
    static scripts = new WeakSet()   // Track executed scripts

    static morph(target, newContent) {
        this._computePersistentIds(target, newContent)
        this._populateIdMaps(target)
        this._morphChildren(target, newContent)
        this._executeNewScripts(target)
        this._cleanup()
    }

    static _findBestMatch(cursor, newChild) {
        // Priority 1: ID set match (same type + tag + overlapping subtree IDs)
        // Priority 2: Soft match (same type + tag, old has no conflicting ID)
        // Anti-churn: block soft match if 2+ future siblings would match
        // Displacement limit: abort if displaced IDs > new node's ID count
    }

    static _removeNode(node) {
        if (this.idMap.has(node)) {
            moveBefore(this.pantry, node, null);  // park (still in document)
        } else {
            node.parentNode?.removeChild(node);   // actually remove
        }
    }

    static _preserveFormState(oldEl, newEl) {
        const preserve = { value, checked, selectedIndex };
        for (const key of preserve) {
            if (oldEl[key] !== undefined && oldEl[key] !== newEl[key]) {
                newEl[key] = oldEl[key];
            }
        }
    }
}
```

## Features

| Feature | Purpose |
|---------|---------|
| Persistent ID set | Anchors — IDs in both old & new with same tag are preserved |
| Ancestor ID map | Enables move detection (reorder lists without morphing) |
| Pantry pattern | Park nodes that might be needed later (still in DOM) |
| Anti-churn protection | Block soft match if 2+ future siblings would match |
| Displacement limit | Bounds forward scan — abort if too many IDs displaced |
| `isEqualNode` skip | Skip entire subtree if structurally identical |
| `moveBefore()` API | Preserves iframe state, animations, web component lifecycle |
| Script execution tracking | `WeakSet` — new scripts cloned (execute), old scripts not re-run |
| Form state preservation | input/textarea/select/option values preserved |
| `data-ignore-morph` | Both old & new must carry it — escape hatch |
| `data-preserve-attr` | List of attribute names the morph won't overwrite |

## Arrow integration

```javascript
// ArrowDomApplicator handles MORPH_NODE (ID 16)
case 16:
    const target = nodeRegistry.get(nodeId);
    const html = textVals[i];  // new HTML content
    const fragment = document.createRange().createContextualFragment(html);
    MorphDom.morph(target, fragment);
    break;
```

## Dependencies

- Feature 06 (ArrowParser)
- Feature 07 (NodeRegistry, ArrowDomApplicator)

## Testing

- Simple morph: old → new HTML → correct DOM state
- Form preservation: input value preserved during morph
- Move detection: reordered list items → moved, not recreated
- Anti-churn: prepend doesn't cause full re-render
- Pantry: parked nodes survive getElementById
- Script execution: new scripts run, old scripts don't re-run
- data-ignore-morph: subtree skipped
- data-preserve-attr: listed attrs not overwritten
