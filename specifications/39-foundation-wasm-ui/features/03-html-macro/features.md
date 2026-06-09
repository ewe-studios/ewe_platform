# Feature 03: html! Macro

## Description

Implement the `html!` macro — compile-time DOM tree builder that parses HTML-like syntax via `TokenTree::Group`, assigns `primal-id` to every element, generates `Html` struct with `Part` descriptors for dynamic `{}` slots. Zero runtime parsing.

**Decisions:** 001, 005, 006

## Module

`crates/foundation_wasm_ui/src/html_macro/` (proc macro + parser)

## API Surface

```rust
#[proc_macro] pub fn html(input: TokenStream) -> TokenStream { ... }
```

### Parsing

- Uses `TokenTree::Group` for brace balancing
- Reuses `foundation_html`'s parser for tag recognition, self-closing rules, attribute parsing
- `{}` delimits a Rust expression that evaluates to anything `IntoHtml`

### Generated output

```rust
html! { <div class="foo">{name}</div> }
// generates:
Html {
    tag: Some("div"),
    attributes: vec![("class".into(), "foo".into()), ("primal-id".into(), "0".into())],
    children: vec![],
    text: None,
    parts: vec![Part::Text(TextPart { node_id: 0 })],
}
```

### primal-id assignment

- Compile-time: macro assigns sequential IDs (0, 1, 2...) scoped to the template
- Runtime: component instance gets a unique prefix (e.g., `42`)
- Final DOM attribute: `primal-id="42:0"` — no cross-component collisions
- User `id` attribute preserved as-is, coexists with `primal-id`

### No template directives

- Discards `@for`, `@if`, `on:click=`, `.value:bind=`, `...{spread}`, `x-directive`
- Control flow is plain Rust: `let items: Vec<_> = users.iter().map(|u| html! { <li>{u.name}</li> }).collect();`

## Dependencies

- `foundation_ui_traits` (Html, Part)
- `proc-macro2`, `quote`, `syn` (for proc macro)
- `foundation_html` parser (tag recognition only)

## Testing

- Simple element → correct Html struct
- Nested elements → correct tree structure
- `{}` slot with string → TextPart generated
- Loop → Vec<Html> renders all items
- Conditional → Option<Html> works
- primal-id → sequential, no gaps for conditionals
- User id attribute → preserved alongside primal-id