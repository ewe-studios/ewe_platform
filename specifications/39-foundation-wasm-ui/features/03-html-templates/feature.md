# Feature 03: HTML Templates

## Description

---

**TODO**: We have a foundation_html crate, lets review it if it works here e.g it can compile the html and create a compile time structure that saves cpu completely. But we could also support both, where we have two macros: 

1. html_template! -> TemplateResult
2. html! -> produces a compile time representation of the html and how the rust fields fit in which this does not even expressively describe.

Lots of things in here that is not clear, lets make it clearer and more detailed.

---

Implement an `html!` macro for declarative template rendering inspired by lit's tagged template literals. Templates are compiled into a tree of Parts that manage dynamic content bindings. Template caching by identity ensures the same template is only parsed once.

## Module

`backends/foundation_wasm_ui/src/shared/template.rs`

## API Surface

```rust
/// The html! macro produces a TemplateResult.
///
/// Example:
///   html! {
///       <div class="container">
///           <h1>{title}</h1>
///           <p>{description}</p>
///           <button on:click={on_click}>Click me</button>
///           <input type="text" .value:bind={name} />
///           {children}
///       </div>
///   }

/// A compiled template ready for rendering.
pub struct Template {
    /// Static HTML content (cached, shared across all instances).
    html: CachedHtml,
    /// Dynamic parts within the template.
    parts: Vec<PartDescriptor>,
}

/// The result of evaluating an html! macro invocation.
pub struct TemplateResult {
    template: &'static Template,  // cached by identity
    values: Vec<PartValue>,       // dynamic values for this invocation
}

/// Types of dynamic parts in a template.
pub enum PartType {
    /// Content between tags: `<div>{value}</div>`
    Child,
    /// Attribute value: `<div class="{value}">`
    Attribute(String),  // attribute name
    /// Property binding: `<input .value:bind={signal}>`
    Property(String),   // property name
    /// Event handler: `<button on:click={handler}>`
    Event(String),      // event name
    /// Spread attributes: `<div ...{attrs}>`
    Spread,
    /// Directive: `<div x-data={expr}>`
    Directive(String),  // directive name
}

/// A value for a dynamic part.
pub enum PartValue {
    /// A signal binding — will create a DomSignalBinding.
    Signal(WeakSignalRef),
    /// A static string value.
    Text(String),
    /// An event handler function.
    EventHandler(Box<dyn Fn(ExternalPointer, &str) + Send + Sync>),
    /// A nested template result (for composition).
    Template(TemplateResult),
    /// A directive configuration.
    Directive(DirectiveConfig),
}
```

## Macro Syntax

The `html!` macro is a proc macro that parses a quasi-HTML syntax:

```rust
html! {
    // Static HTML
    <div class="container">
        <h1>{title}</h1>                    // ChildPart - dynamic text

        // Attribute binding
        <img src="{image_url}" alt="{alt}" />

        // Property binding (dot prefix)
        <input .value:bind={name_signal} />  // Binds signal to input.value

        // Event handler (on: prefix)
        <button on:click={handle_click}>Submit</button>

        // Conditional rendering
        @if show_message {
            <p>{message}</p>
        }

        // Loop rendering
        @for item in items {
            <li key={item.id}>{item.name}</li>
        }

        // Spread attributes
        <div ...{extra_attrs}></div>

        // Directive
        <div x-intersect={on_visible}>...</div>

        // Nested templates
        {child_component.render()}
    </div>
}
```

## Implementation Details

### Template compilation
1. `html!` macro parses at compile time
2. Extracts static HTML string and identifies dynamic expression positions
3. Generates a `static Template` with Parts metadata
4. At runtime, evaluates expressions into `PartValue`s
5. Combines with cached Template into `TemplateResult`

### Template caching
- Template identity = the static HTML string
- First evaluation: parse HTML, create Parts, cache
- Subsequent evaluations: reuse cached Template, only evaluate dynamic values
- This mirrors lit's `TemplateStringsArray` identity caching

### Part types mapping to DOM operations
- `ChildPart` → `setTextContent(node, value)`
- `AttributePart` → `setAttribute(node, name, value)`
- `PropertyPart` → `setProperty(node, name, value)`
- `EventPart` → `addEventListener(node, name, handler)`
- `SpreadPart` → iterate attrs, set each
- `DirectivePart` → invoke directive logic

### Key-based list rendering
- `@for item in items { <li key={item.id}>` uses idiomorph-style keyed diffing
- Items with same key are preserved (DOM nodes not recreated)
- Items added/removed/moved are identified by key

### Conditional rendering
- `@if condition { ... }` — when false, removes the subtree
- When true, creates the subtree
- Transitions use DOM morphing (Feature 04's morph module)

### Directives
Directives are special attribute handlers that extend template behavior:

```rust
/// A directive that runs when an element enters the viewport.
pub struct IntersectDirective {
    callback: Box<dyn Fn(bool) + Send + Sync>,
}

/// A directive that focuses an element when rendered.
pub struct FocusDirective;

/// Directive registry for custom directives.
pub struct DirectiveRegistry {
    directives: HashMap<String, Box<dyn Directive>>,
}
```

## Dependencies

- `foundation_wasm` (for ExternalPointer, batch operations)
- `foundation_macros` (for html! proc macro)
- `serde_json` (for spread attrs)

## Testing

- html! macro compiles with static content → TemplateResult
- html! with signal binding → PartValue::Signal in parts
- Template caching → same html! text shares Template
- Different html! text → different Template
- Conditional @if → ChildPart added/removed
- Loop @for with key → keyed diff preserves nodes
- Event on:click → PartValue::EventHandler registered
- Property .value:bind → PropertyPart with signal binding
