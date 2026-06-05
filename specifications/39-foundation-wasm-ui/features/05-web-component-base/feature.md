# Feature 05: Web Component Base

## Description

Implement the web component integration layer — custom element registration, shadow DOM management, lifecycle callbacks, and attribute observation. Components are registered with the browser's custom elements registry and bridge between WASM logic and the DOM.

## Module

`backends/foundation_wasm_ui/src/wasm/custom_elements.rs`

## API Surface

```rust
/// Register a component type with the browser's customElements registry.
pub fn register_component<C: Component>() -> Result<(), RegistrationError>;

/// Component definition — metadata for custom element registration.
pub struct ComponentDef {
    pub tag_name: String,
    pub observed_attributes: Vec<String>,
    pub use_shadow_dom: bool,
    pub shadow_mode: ShadowMode,  // Open or Closed
}

pub enum ShadowMode {
    Open,
    Closed,
}

/// Internal component instance stored per DOM element.
struct ComponentInstance {
    component: Box<dyn Component>,
    host: ComponentHost,
    bindings: Vec<DomSignalBinding>,
    event_bindings: Vec<EventSignalBinding>,
    template: Option<TemplateResult>,
}
```

## Lifecycle Flow

```
1. Browser calls connectedCallback() on custom element
   → WASM: Component::connected(&host)
   → WASM: component.render() → TemplateResult
   → WASM: Apply template via Arrow batch
   → DOM: Shadow root populated

2. Browser calls disconnectedCallback()
   → WASM: Component::disconnected()
   → WASM: Unbind all signals, event listeners
   → DOM: Shadow root cleared

3. Browser calls attributeChangedCallback(name, old, new)
   → WASM: Component::attribute_changed(name, old, new)
   → WASM: component.render() → re-render
   → DOM: Update via Arrow batch
```

## Implementation Details

### Custom element registration
The JS runtime provides a `registerElement` function:

```javascript
// Called from WASM via host_invoke_function
function registerElement(tagName, observedAttributes, useShadow) {
    class WasmElement extends HTMLElement {
        constructor() {
            super();
            this._componentId = allocateComponentId();
            if (useShadow) {
                this._shadow = this.attachShadow({ mode: 'open' });
            }
        }

        connectedCallback() {
            wasm_exports.on_connected(this._componentId);
        }

        disconnectedCallback() {
            wasm_exports.on_disconnected(this._componentId);
        }

        attributeChangedCallback(name, oldVal, newVal) {
            wasm_exports.on_attribute_changed(this._componentId, name, oldVal, newVal);
        }
    }

    WasmElement.observedAttributes = observedAttributes;
    customElements.define(tagName, WasmElement);
}
```

### Shadow DOM
- Default: `mode: 'open'` — accessible for testing and debugging
- Optional: `mode: 'closed'` — encapsulation
- Component's `render()` populates the shadow root via Arrow batch
- Slots supported — `<slot>` elements in template accept light DOM content

### Attribute observation
- `observed_attributes()` returns list of attribute names
- Browser calls `attributeChangedCallback` only for observed attributes
- Component can react to attribute changes and re-render

### Node ID scoping
- Each component gets a unique node ID prefix
- `component_id * 10000 + local_id` → globally unique node IDs
- Prevents conflicts between multiple component instances

### Style encapsulation
- Shadow DOM provides style encapsulation by default
- Adopted stylesheets supported: `shadowRoot.adoptedStyleSheets = [sheet]`
- Component can define a CSS string that's applied as a `<style>` in shadow root

## Dependencies

- `foundation_wasm` (for host_invoke, ExternalPointer, callbacks)
- Foundation_wasm_ui core (Component trait, TemplateResult, signals)

## Testing

- Register component → customElements.get(tagName) returns constructor
- connectedCallback → shadow root populated with rendered content
- disconnectedCallback → shadow root cleared, bindings removed
- attributeChangedCallback → component re-renders
- Multiple instances → each has independent state
- Shadow mode closed → shadowRoot not accessible from outside
