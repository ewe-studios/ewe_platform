# Datastar Form Preservation — Learnings

## Source

Datastar's `patchElements.ts` (morphNode form handling) and `bind.ts` (two-way data binding plugin).

---

## 1. The Attribute vs. Property Problem

The root of all form preservation complexity:

```html
<input type="text" value="initial">
```

After page load → after user types "hello":

| | Before typing | After typing |
|---|---|---|
| `el.getAttribute('value')` | `"initial"` | `"initial"` (unchanged) |
| `el.value` | `"initial"` | `"hello"` (live state) |

**Key insight**: Any DOM update strategy must sync **properties** (not just attributes) when morphing form elements. The server sends HTML with `value="..."` attributes, but the morph must write to `.value` (the DOM property) on the old element.

---

## 2. Form State Handling in `morphNode()`

### 2.1 `data-preserve-attr` Mechanism

Before form handling, read `data-preserve-attr` from the **new** element:

```typescript
const preserveAttrs = (newNode.getAttribute('data-preserve-attr') ?? '').split(' ')
```

Space-separated list of attributes that should NOT be overwritten. If `value` is in the list, the user's current input is preserved even if the server sends a different value.

```html
<!-- Server sends this, but user's typed input is preserved -->
<input id="search" data-preserve-attr="value" value="server-default">
```

### 2.2 `HTMLInputElement` Handling

```typescript
if (oldElt instanceof HTMLInputElement && newElt instanceof HTMLInputElement && newElt.type !== 'file') {
  // Value sync: read from new attribute, write to old property
  const newValue = newElt.getAttribute('value')
  if (oldElt.getAttribute('value') !== newValue && !preserveAttrs.includes('value')) {
    oldElt.value = newValue ?? ''      // PROPERTY, not attribute
    shouldDispatchChangeEvent = true
  }

  // Checked state sync
  shouldDispatchChangeEvent = updateElementProp(oldElt, newElt, 'checked') || shouldDispatchChangeEvent

  // Disabled state sync
  updateElementProp(oldElt, newElt, 'disabled')
}
```

**Critical detail**: Value sync reads from `getAttribute('value')` on the new element but writes to `oldElt.value` on the old element. If the server's attribute matches the old attribute, nothing happens — preserving user edits.

**File inputs are excluded** (`type !== 'file'`) because file inputs are read-only. Setting `.value` on a file input throws a security error.

### 2.3 `updateElementProp` — Boolean Attribute Bridge

```typescript
const updateElementProp = (oldElt, newElt, name): boolean => {
  const newEltHasAttr = newElt.hasAttribute(name)
  if (oldElt.hasAttribute(name) !== newEltHasAttr && !preserveAttrs.includes(name)) {
    oldElt[name] = newEltHasAttr   // boolean PROPERTY from attribute presence
    return true
  }
  return false
}
```

For `checked`: if new element has `checked` attribute and old doesn't → `oldElt.checked = true`. If both agree → do nothing. Leverages the fact that boolean HTML attributes are present/absent (not true/false).

### 2.4 `HTMLTextAreaElement` Handling

```typescript
if (oldElt instanceof HTMLTextAreaElement && newElt instanceof HTMLTextAreaElement) {
  if (oldElt.defaultValue !== newValue) {
    oldElt.value = newValue
    shouldDispatchChangeEvent = true
  }
}
```

Uses `defaultValue` (markup content) not `value` (user input). The morph only overwrites user text when the **server's default** actually changed.

### 2.5 `HTMLOptionElement` Handling

```typescript
if (oldElt instanceof HTMLOptionElement && newElt instanceof HTMLOptionElement) {
  updateElementProp(oldElt, newElt, 'selected')
}
```

### 2.6 Synthetic `change` Event

When any form state is modified by the morph, a synthetic `change` event is dispatched:

```typescript
if (shouldDispatchChangeEvent) {
  const dispatchElt = oldElt instanceof HTMLOptionElement ? oldElt.closest('select') : oldElt
  dispatchElt?.dispatchEvent(new Event('change', { bubbles: true }))
}
```

This ensures:
1. `data-on:change` handlers fire
2. `data-bind` listeners pick up the change
3. Third-party form libraries stay in sync
4. Event delegation patterns work (`bubbles: true`)

For `<option>` elements, the event is dispatched on the parent `<select>` (standard browser convention).

---

## 3. Two-Way Data Binding: The `bind` Plugin

### 3.1 Architecture

```
Signal Store ←── effect(() => set(getPath('email'))) ──→ DOM Element
     ↑                                                    │
     └──────── 'input'/'change' event → syncSignal() ─────┘
```

Two connections:
1. **Signal → DOM**: `effect()` reads signal, calls element-specific `set()`
2. **DOM → Signal**: Event listeners read element, call `mergePaths()`

### 3.2 Element-Specific Getters/Setters

| Element Type | Get | Set |
|-------------|-----|-----|
| Text inputs | `el.value` (coerced to signal type) | `el.value = String(value)` |
| Number/range | `+el.value` (unless signal is string) | `el.value = String(value)` |
| Checkbox | `el.checked` (boolean) or `el.value` (string) | `el.checked = value === el.value` or `value` |
| Radio | `el.checked ? el.value : empty` (Symbol sentinel) | `el.checked = value === el.value` |
| File | `FileReader.readAsDataURL()` → `{name, contents, mime}` | N/A (write-only) |
| Multi-select | `[...el.selectedOptions].map(o => o.value)` | Set `option.selected` by matching values |
| Web component | `el.value` property or `el.getAttribute('value')` | `el.value = value` or `el.setAttribute` |

**Radio sentinel**: Unchecked radios return `empty` (a unique Symbol) so they don't write empty/false values to the signal. Only the checked button writes.

**Auto-naming**: Radio buttons without a `name` attribute get the signal name as their name, so all radios bound to the same signal form a group.

### 3.3 Signal Initialization with `ifMissing`

```typescript
mergePaths([[path, get(el, type)]], { ifMissing: true })
```

- If signal exists → element reads from it
- If signal doesn't exist → create with element's current value
- Prevents server-driven signal patches from being overwritten by initial DOM state

### 3.4 Avoiding Infinite Loops

DOM → Signal → DOM loop naturally stops because:
- `syncSignal` calls `mergePaths()` → signal changes
- Signal change triggers `effect()` → calls `set()` on DOM
- Programmatic `.value` assignment doesn't fire `input`/`change` events
- Loop stops

The synthetic `change` event from `morphNode` is the exception — but morphing and binding operate on different elements, so no cycles on the same element.

---

## 4. Morph + Bind Interaction Sequence

```
1. Morph updates the property → existing change listener fires → signal updates
2. Morph changes/adds data-bind attributes → MutationObserver fires
3. Old bind cleanup runs → removes effect + event listeners
4. New bind initializes with ifMissing → reads current signal value
```

This means a server can:
- **Update form values**: Send new `value` attributes → morph syncs → change event → signal updates
- **Preserve user input**: Use `data-preserve-attr="value"` → morph skips value sync → user's typing preserved
- **Change form structure**: Add/remove inputs → morph handles DOM, MutationObserver handles binding lifecycle

---

## 5. Edge Cases

### `defaultValue` vs `value` for Textareas

The morph compares against `defaultValue`, not `value`. Only overwrite user text if the server is sending a **new default**. Same default → user edits preserved.

### Attribute removal uses `Array.from()`

The removal pass iterates `Array.from(oldElt.attributes)`, not the live `NamedNodeMap`. Removing attributes during iteration shifts indices, causing skipped entries.

### Change event on `<option>` goes to `<select>`

`<option>` elements don't fire their own `change` events. Dispatch on the parent `<select>` so bind listeners work correctly.

---

## 6. Relevance to Spec-39

### What We Should Incorporate

1. **Attribute-to-property bridge** — Read from new element's attributes, write to old element's properties. This is the critical insight for form preservation during morphing.

2. **`data-preserve-attr` escape hatch** — Allow server to specify which attributes the morph should not overwrite. Essential for preserving user input during server-driven updates.

3. **Synthetic `change` event** — After morphing form state, dispatch a synthetic `change` event (on the correct element — `<select>` for `<option>` changes). This keeps signal bindings in sync.

4. **`defaultValue` comparison for textareas** — Compare against `defaultValue` (markup), not `value` (user input). Only overwrite when the default actually changed.

5. **File input exclusion** — Never attempt to set `.value` on file inputs. Security error.

6. **Boolean attribute bridge** — `updateElementProp` pattern for `checked`/`disabled`/`selected`. Presence/absence → boolean property.

7. **`Array.from(attributes)` for removal** — Avoid live `NamedNodeMap` iteration issues.

8. **Two-way binding loop prevention** — Programmatic `.value` assignment doesn't trigger DOM events, so the loop naturally stops.

9. **Signal initialization with `ifMissing`** — Read existing signal before writing element value. Prevents server state from being overwritten by DOM initial values.

10. **Radio sentinel pattern** — Use a unique Symbol for unchecked radio values. Prevents unchecked radios from overwriting the signal with empty values.
