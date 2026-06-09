# Feature 07: DOM Morphing

## Description

`MorphDom` class in the JS runtime for server/WASM HTML patches with form preservation, element identity tracking, and move detection. Based on Datastar's approach (morphdom + idiomorph). Invoked by Arrow MORPH_NODE (ID 16).

**Decision:** 027

## Module

`crates/foundation_wasm_ui/assets/foundation-wasm-ui.js` (MorphDom class)

## Algorithm: Four phases

```
Phase 1: Build persistent ID set  → IDs in both old & new with same tag
Phase 2: Populate ID maps          → Map each ancestor to persistent IDs in subtree
Phase 3: Morph children             → Walk new content, match old, reconcile
Phase 4: Cleanup                    → Remove pantry, clear maps
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
case 16:  // MORPH_NODE
    const target = nodeRegistry.get(nodeId);
    const html = textVals[i];
    const fragment = document.createRange().createContextualFragment(html);
    MorphDom.morph(target, fragment);
    break;
```

## Dependencies

- Feature 05 (Arrow encoding)
- Feature 00 (JS runtime core)

## Testing

- Simple morph: old → new HTML → correct DOM state
- Form preservation: input value preserved during morph
- Move detection: reordered list items → moved, not recreated
- Anti-churn: prepend doesn't cause full re-render
- Pantry: parked nodes survive getElementById
- Script execution: new scripts run, old scripts don't re-run