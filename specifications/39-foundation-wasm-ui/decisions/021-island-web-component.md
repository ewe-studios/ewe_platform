# 021 — `<island>` Web Component: scoped content container

**Date:** 2026-06-08
**Status:** Resolved

### Decision

A central `<island>` custom element handles scoped scripts, styles, and event bindings. The browser instantiates it automatically — no MutationObserver scanning needed for scoped content.

### How it works

```html
<island>
  <div id="menu-tabs">
    <style primal:style>
      .title { font-size: 20px; }
      .subtitle { color: gray; }
    </style>
    <script primal:script>
      function(scope) {
        let targets = scope.targets();
        primal.addEvent(targets, "click", () => { ... });
      }
    </script>
    <button primal:onclick="controller.delete">Delete</button>
  </div>
</island>
```

### `IslandComponent` lifecycle

```js
class IslandComponent extends HTMLElement {
    connectedCallback() {
        // 1. Process scoped styles
        this._processStyles();
        
        // 2. Execute scoped scripts
        this._processScripts();
        
        // 3. Wire up event bindings
        this._wireEvents();
    }
    
    disconnectedCallback() {
        // Cleanup: remove injected styles, unbind events
        this._cleanup();
    }
    
    _processStyles() {
        const styles = this.querySelectorAll('style[primal\\:style]');
        for (const style of styles) {
            const css = style.textContent;
            // Wrap all CSS in a parent selector for scoping
            const scoped = `:host > * { ${css} }`;
            // Or if using light DOM, prepend with the island's id/class
            const sheet = new CSSStyleSheet();
            sheet.replaceSync(scoped);
            this.shadowRoot?.adoptedStyleSheets?.push(sheet);
        }
    }
    
    _processScripts() {
        const scripts = this.querySelectorAll('script[primal\\:script]');
        for (const script of scripts) {
            const body = script.textContent;
            const scope = this._createScope(script.parentElement);
            const fn = new Function('scope', body);
            fn(scope);
        }
    }
    
    _wireEvents() {
        // Walk all descendants, wire up primal:on* attributes
        const elements = this.querySelectorAll('[primal\\:on]');
        for (const el of elements) {
            // ... wire events
        }
    }
    
    _createScope(targetElement) {
        return {
            targets: () => [targetElement],
            parent: () => this,
            querySelector: (sel) => this.querySelector(sel),
            addEvent: (parent, eventType, handlerRef) => {
                // ... delegate event binding
            },
        };
    }
}

customElements.define('island', IslandComponent);
```

### CSS scoping strategy

**Simple approach**: Wrap all CSS in a parent selector:

```css
/* Original: */
.title { font-size: 20px; }

/* Transformed: */
#island-1 .title { font-size: 20px; }
```

**Alternative**: Use `:host` for shadow DOM scoping:

```css
/* Transformed: */
:host .title { font-size: 20px; }
```

If users nest CSS, the parent selector is prepended to the outermost selector:

```css
/* Original: */
.menu {
  & .item { padding: 8px; }
}

/* Transformed: */
#island-1 .menu .item { padding: 8px; }
```

### Why this design

- **No MutationObserver for scoped content** — browser handles discovery via custom elements
- **Automatic lifecycle** — `connectedCallback` / `disconnectedCallback` for setup/cleanup
- **Isolated scope** — each island manages its own scripts, styles, events
- **Nesting supported** — islands can contain other islands
- **Server-compatible** — server sends `<island>` content, browser handles the rest
- **Clean teardown** — when island is removed, all its scoped resources are cleaned up
