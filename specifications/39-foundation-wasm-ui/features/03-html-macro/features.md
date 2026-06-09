# Feature 03: html! Macro

**Crate:** `crates/foundation_wasm_ui_macro/` (proc macro crate)
**Runtime support:** `crates/foundation_wasm_ui/src/html_macro/`
**Decisions:** 001 (pure Rust, no template syntax), 005 (compile-time element ID + Part-level effects), 006 (runtime-prefixed element IDs), 029 (signal getter/setter and two-way bindings)

Compile-time proc macro that parses HTML-like syntax via `TokenTree` walking, assigns `primal-id` to every element, and produces Rust code constructing an `Html` struct with `Part` descriptors for dynamic `{}` slots. Zero runtime HTML parsing. No template directives (`@for`, `@if`, `on:click=`, `.value:bind=`, `...{spread}`). Control flow is plain Rust.

## 1. Types (from F01 foundation_ui_traits)

```rust
pub struct Html {
    pub tag: Option<String>,              // None = text node, Some = element
    pub attributes: Vec<(String, String)>,
    pub children: Vec<Html>,
    pub text: Option<String>,
    pub parts: Vec<Part>,                 // Reactive binding descriptors
}
pub enum Part { Text(TextPart), Attribute(AttrPart), Event(EventPart), Children(ChildPart) }
pub struct TextPart   { pub node_id: u32 }
pub struct AttrPart   { pub node_id: u32, pub attr_name: String }
pub struct EventPart  { pub node_id: u32, pub event_name: String }
pub struct ChildPart  { pub parent_id: u32 }
```

## 2. Proc Macro Entry Point

```rust
#[proc_macro]
pub fn html(input: TokenStream) -> TokenStream { ... }
```

Standalone proc macro crate. Depends on `proc-macro2`, `quote`, `syn`. Embeds void element tables as static data (does NOT link `foundation_html`).

## 3. Parsing Algorithm

### 3.1 Internal Parse Structures

```rust
enum ParsedNode {
    Element { tag: String, attributes: Vec<ParsedAttr>, children: Vec<ParsedNode>, self_closing: bool },
    TextLiteral(String),
    DynamicSlot(proc_macro2::TokenStream),   // captured verbatim from {}
}
enum ParsedAttr {
    Static  { name: String, value: String },
    Dynamic { name: String, tokens: proc_macro2::TokenStream },
    Event   { event_name: String, tokens: proc_macro2::TokenStream },
}
```

### 3.2 Token Walking

```
fn parse_nodes(tokens) -> Vec<ParsedNode>:
    nodes = []
    loop:
        CASE peek is Punct('<'):
            advance '<'
            if peek is Punct('/'): return nodes          // closing tag — caller handles
            tag = parse_ident(tokens)                    // "div", "my-component"
            attrs = parse_attributes(tokens)             // until '>' or '/>'
            self_closing = consume_close(tokens)         // true if '/>', false if '>'
            if self_closing OR is_void_element(tag):
                push Element { tag, attrs, children: [], self_closing: true }
            else:
                children = parse_nodes(tokens)           // recurse
                consume_closing_tag(tokens, tag)         // '<' '/' tag '>'
                push Element { tag, attrs, children, self_closing: false }
        CASE peek is Group(Brace):
            push DynamicSlot(token.stream()); advance
        CASE peek is Literal(string):
            push TextLiteral(unquoted value); advance
        DEFAULT:
            accumulate consecutive non-tag tokens as TextLiteral
    return nodes
```

### 3.3 Attribute Parsing

```
fn parse_attributes(tokens) -> Vec<ParsedAttr>:
    loop until peek is '>' or '/>':
        name = parse_attr_name(tokens)       // handles "primal:onclick", "data-id"
        if name starts with "primal:on":
            event = name.strip_prefix("primal:on")   // "primal:onclick" -> "click"
            consume '='; value_tokens = parse_attr_value_tokens(tokens)
            push Event { event_name: event, tokens: value_tokens }
        else if peek is '=':
            advance '='
            if peek is Group(Brace): push Dynamic { name, tokens: inner }
            else: push Static { name, value: parse_string_literal(tokens) }
        else:
            push Static { name, value: "true" }      // boolean attribute
```

### 3.4 Void Elements

`VOID_ELEMENTS`: `area`, `base`, `br`, `col`, `embed`, `hr`, `img`, `input`, `link`, `meta`, `param`, `source`, `track`, `wbr`. Unknown tags are non-void (require closing tag or `/>`).

## 4. primal-id Assignment

Every element gets `primal-id`. Text nodes and dynamic slots reference parent ID via Parts.

### 4.1 Compile-Time (depth-first, sequential)

```
fn assign_ids(nodes, counter: &mut u32):
    for node in nodes:
        if Element: attrs.insert(0, "primal-id" = counter.to_string()); counter += 1; recurse children
        if TextLiteral | DynamicSlot: skip
```

IDs start at 0, scoped to the macro invocation, increment per element regardless of nesting.

### 4.2 Runtime Prefixing

At mount, the component gets a unique prefix from Runtime's atomic counter. `prefix_ids` rewrites `"0"` to `"42:0"`. User `id` preserved alongside `primal-id`.

```rust
fn prefix_ids(html: &mut Html, prefix: u64) {
    for attr in &mut html.attributes {
        if attr.0 == "primal-id" { attr.1 = format!("{}:{}", prefix, attr.1); }
    }
    for child in &mut html.children { prefix_ids(child, prefix); }
}
```

### 4.3 Loops and Conditionals

Each `html!` in a `.map()` closure is a separate expansion (IDs start at 0). At mount, each iteration gets a distinct prefix: `"43:0"`, `"44:0"`. Conditional branches assign IDs at compile time; unused IDs absent from DOM. Gaps acceptable.

## 5. Part Determination

### 5.1 Classification Rules

| Template position | Part generated |
|---|---|
| `<div>{expr}</div>` child content | `Part::Text(TextPart { node_id: parent_primal_id })` |
| `<div class={expr}>` dynamic attr | `Part::Attribute(AttrPart { node_id, attr_name })` |
| `<button primal:onclick={expr}>` event | `Part::Event(EventPart { node_id, event_name })` |

All dynamic slots in child position produce `Part::Text` at compile time. Runtime `IntoHtml` handles `Html`/`Vec<Html>` promotion.

### 5.2 Collection Walk

```
fn collect_parts(nodes, parent_id: Option<u32>, parts: &mut Vec<Part>):
    for node in nodes:
        Element: id = extract_primal_id(attrs)
            Dynamic attr -> push Attribute(id, name); Event -> push Event(id, event_name)
            recurse children with parent_id = id
        DynamicSlot: push Text(TextPart { node_id: parent_id.unwrap() })
        TextLiteral: skip
```

Parts flatten into root `Html.parts` for `mount(ctx)` to iterate.

## 6. Setter Detection and Callback Codegen

The macro cannot resolve types. All `primal:on*` handler expressions generate `MaybeCallback` trait dispatch. At Rust compile time, the trait resolves to `Some(id)` for `SignalSetter` or `None` for closures.

### 6.1 MaybeCallback Trait (in foundation_wasm_ui)

```rust
pub trait MaybeCallback { fn maybe_callback_id(&self) -> Option<u64>; }
impl<T> MaybeCallback for SignalSetter<T> {
    fn maybe_callback_id(&self) -> Option<u64> { Some(self.callback_id()) }
}
impl<F: Fn(web_sys::Event)> MaybeCallback for F {
    fn maybe_callback_id(&self) -> Option<u64> { None }
}
```

### 6.2 JS Wiring

JS event runtime (F08) reads `primal:setter` attribute during wiring. On event fire: `invoke_callback(42, event.target.value)` -> WASM -> callback registry -> setter -> signal update -> stabilize -> effects.

## 7. Full Codegen Example

**Input:** `html! { <div class="greeting"><span>{name.get()}</span><input primal:onchange={set_name} value={name.get()} /><button primal:onclick={move |_| set_count.update(|c| *c += 1)}>Count: {count.get()}</button></div> }`

**Generated output (conceptual):**
```rust
{
    let __h1 = {set_name};
    let __cb1: Option<u64> = MaybeCallback::maybe_callback_id(&__h1);
    let __h2 = {move |_| set_count.update(|c| *c += 1)};
    let __cb2: Option<u64> = MaybeCallback::maybe_callback_id(&__h2);
    let mut __ia = vec![("primal-id".into(),"2".into()),("primal:onchange".into(),"true".into()),
        ("value".into(), ({name.get()}).into_html().text.unwrap_or_default())];
    if let Some(id) = __cb1 { __ia.push(("primal:setter".into(), id.to_string())); }
    let mut __ba = vec![("primal-id".into(),"3".into()),("primal:onclick".into(),"true".into())];
    if let Some(id) = __cb2 { __ba.push(("primal:setter".into(), id.to_string())); }
    Html {
        tag: Some("div".into()),
        attributes: vec![("primal-id".into(),"0".into()),("class".into(),"greeting".into())],
        children: vec![
            Html { tag: Some("span".into()), attributes: vec![("primal-id".into(),"1".into())],
                children: vec![({name.get()}).into_html()], text: None, parts: vec![] },
            Html { tag: Some("input".into()), attributes: __ia,
                children: vec![], text: None, parts: vec![] },
            Html { tag: Some("button".into()), attributes: __ba,
                children: vec![({count.get()}).into_html()], text: None, parts: vec![] },
        ],
        text: None,
        parts: vec![
            Part::Text(TextPart { node_id: 1 }),
            Part::Attribute(AttrPart { node_id: 2, attr_name: "value".into() }),
            Part::Event(EventPart { node_id: 2, event_name: "change".into() }),
            Part::Event(EventPart { node_id: 3, event_name: "click".into() }),
            Part::Text(TextPart { node_id: 3 }),
        ],
    }
}
```

IDs: div=0, span=1, input=2, button=3. `__h1` captures setter -> `MaybeCallback` resolves `Some(callback_id)` -> `primal:setter` injected. `__h2` captures closure -> resolves `None` -> no `primal:setter`.

## 8. mount(ctx) Integration

Mount (in `foundation_wasm_ui`) walks `Html.parts`, creates one effect per Part (decision 005). Effects run immediately (decision 008). On signal change, only affected effects re-run. `Text` -> effect queues `SetText`. `Attribute` -> effect queues `SetAttribute`. `Event` -> queues `AddEventListener` once (no effect). `Children` -> effect diffs and queues child ops.

## 9. Error Cases and Edge Cases

### Compile-Time Errors (all include `proc_macro2::Span` for IDE integration)

| Condition | Message |
|---|---|
| Unclosed tag | `html!: unclosed tag '<{tag}>'` |
| Mismatched close | `html!: expected closing '</{expected}>', found '</{found}>'` |
| Missing `>` | `html!: missing '>' after attributes for '<{tag}>'` |
| `=` without value | `html!: attribute '{name}' has '=' but no value` |
| Empty invocation | `html!: empty input` |
| Multiple roots | `html!: multiple root elements — wrap in a parent` |
| Unknown `primal:` | `html!: unknown primal attribute '{name}'` |
| Unterminated string | `html!: unterminated string in attr '{attr}' of '<{tag}>'` |

### Edge Cases

| Scenario | Behavior |
|---|---|
| `<br>` (no `/>`) | Void element, no closing tag expected |
| `<my-widget>` custom element | Non-void, requires closing tag or `/>` |
| `Hello {name}!` text+slot | Three nodes: TextLiteral, DynamicSlot, TextLiteral |
| `<div></div>` empty | Empty children, still gets `primal-id` |
| `<input disabled>` boolean | Attribute `("disabled", "true")` |
| `<div class=foo>` unquoted | NOT supported — use quoted string or `{expr}` |
| Nested `html!` in `{}` | Separate expansion, independent `Html` |
| `{None::<Html>}` | Via IntoHtml: empty node |
| `{vec![html!{...}]}` | Via IntoHtml: tagless node with children |

## 10. Integration Points

| Feature | Interaction |
|---|---|
| F01 (foundation_ui_traits) | Generates `Html`, `Part`; calls `.into_html()` on slot expressions |
| F02 (Signal System) | Calls `getter.get()`; reads `setter.callback_id()` for event codegen |
| F04 (InstructionReceiver) | Mount effects queue `DomOp` per Part |
| F06 (Web Components) | Provides instance prefix for `primal-id` |
| F08 (Event Runtime) | JS reads `primal:setter(N)` to wire events to WASM |

## 11. File Ownership

```
crates/foundation_wasm_ui_macro/src/
    lib.rs              # #[proc_macro] pub fn html
    parser.rs           # parse_nodes, parse_attributes, parse_attr_name
    parsed_node.rs      # ParsedNode, ParsedAttr enums
    id_assign.rs        # assign_ids: primal-id counter walk
    part_collect.rs     # collect_parts: Part determination walk
    codegen.rs          # Quote-based Rust code generation
    void_elements.rs    # VOID_ELEMENTS, is_void_element()
crates/foundation_wasm_ui/src/html_macro/
    mod.rs              # Re-exports
    maybe_callback.rs   # MaybeCallback trait + impls
    mount.rs            # mount(ctx, html, receiver): effect-per-Part
    id_prefix.rs        # prefix_ids(html, instance_prefix)
```

## 12. Refactoring Strategy

1. Create `foundation_wasm_ui_macro` with parser + codegen for static HTML only.
2. Add dynamic slot parsing (`{}`), `.into_html()` calls, `Part::Text` generation.
3. Add event handler detection + setter codegen (`MaybeCallback` dispatch).
4. Implement `mount.rs` and `id_prefix.rs` in `foundation_wasm_ui`.
5. Integration test: full component with signals, html!, mount, verify DomOps.

## 13. Dependencies

**Macro crate:** `proc-macro2` (tokens), `quote` (codegen), `syn` (LitStr parsing).
**Generated code:** `foundation_ui_traits` (Html, Part, IntoHtml), `foundation_signals` (SignalGetter, SignalSetter).
**Runtime:** `foundation_wasm_ui` (MaybeCallback, mount, prefix_ids).

## 14. Testing

### Parsing and Codegen (tests 1-12)

| # | Input | Verify |
|---|---|---|
| 1 | `html! { <div></div> }` | `tag == Some("div")`, empty children, `primal-id="0"` |
| 2 | `html! { <div class="foo"></div> }` | attrs contain `("class","foo")` and `("primal-id","0")` |
| 3 | `html! { <br> }` | Void element, no error, `primal-id="0"` |
| 4 | `html! { <div><span></span><p></p></div> }` | IDs: div=0, span=1, p=2 |
| 5 | `html! { <div>{name}</div> }` | `parts` has `Part::Text(TextPart { node_id: 0 })` |
| 6 | `html! { <div class={expr}></div> }` | `Part::Attribute(AttrPart { node_id: 0, attr_name: "class" })` |
| 7 | `html! { <button primal:onclick={h}></button> }` | `Part::Event(EventPart { node_id: 0, event_name: "click" })` |
| 8 | `html! { <input primal:onchange={set_name} /> }` | Codegen includes `MaybeCallback::maybe_callback_id` call |
| 9 | `html! { <div>{a}{b}</div> }` | Two `Part::Text` entries, both `node_id: 0` |
| 10 | `html! { <div><span>{x}</span><p>{y}</p></div> }` | Part node_ids: span=1, p=2 |
| 11 | `html! { <div id="user" class="x"></div> }` | Both `id="user"` and `primal-id="0"` present |
| 12 | `html! { <input disabled /> }` | Boolean attr: `("disabled", "true")` |

### primal-id Assignment (tests 13-17)

| # | Input | Expected IDs |
|---|---|---|
| 13 | `<div><span><a></a></span></div>` | div=0, span=1, a=2 |
| 14 | `<ul><li></li><li></li><li></li></ul>` | ul=0, li=1, li=2, li=3 |
| 15 | `<div><img /><br><hr></div>` | div=0, img=1, br=2, hr=3 |
| 16 | `<div><div><div></div></div></div>` | 0, 1, 2 (depth-first) |
| 17 | Two separate `html!` calls in loop | Each starts at 0 independently |

### Runtime Prefixing (tests 18-20)

| # | Scenario | Verify |
|---|---|---|
| 18 | `prefix_ids(html, 42)` on `primal-id="0"` | Becomes `"42:0"` |
| 19 | Tree with IDs 0,1,2 prefixed with 99 | `"99:0"`, `"99:1"`, `"99:2"` |
| 20 | User `id="myid"` after prefixing | Both `id="myid"` and `primal-id="42:3"` present |

### Part Collection (tests 21-25)

| # | Input | Parts |
|---|---|---|
| 21 | `<div>{x}</div>` | `[Text(0)]` |
| 22 | `<div class={c}>{x}</div>` | `[Attribute(0,"class"), Text(0)]` |
| 23 | `<form><input primal:onchange={s}/><button primal:onclick={h}></button></form>` | `[Event(1,"change"), Event(2,"click")]` |
| 24 | `<div class="x">hello</div>` | `[]` (static only) |
| 25 | `<div>{a}<span>{b}</span>{c}</div>` | `[Text(0), Text(1), Text(0)]` |

### Setter Codegen (tests 26-29)

| # | Scenario | Verify |
|---|---|---|
| 26 | `SignalSetter` in `primal:onchange` | `maybe_callback_id` returns `Some(id)` |
| 27 | Closure in `primal:onclick` | `maybe_callback_id` returns `None`, no `primal:setter` attr |
| 28 | Two setters in same template | Distinct `__handler_N` vars, distinct callback IDs |
| 29 | Setter callback_id in output | `primal:setter` attr value matches `setter.callback_id()` |

### IntoHtml Integration (tests 30-34)

| # | Expression | Expected |
|---|---|---|
| 30 | `{42u32}` | Text node: `text == Some("42")`, `tag == None` |
| 31 | `{"hello"}` | Text node: `text == Some("hello")` |
| 32 | `{Some(html! { <b></b> })}` | Unwrapped Html with `tag == Some("b")` |
| 33 | `{None::<Html>}` | Empty Html node |
| 34 | `{vec![html!{<li></li>}, html!{<li></li>}]}` | Tagless node, two `<li>` children |

### Error Diagnostics (tests 35-39)

| # | Input | Expected error substring |
|---|---|---|
| 35 | `html! { <div> }` | `"unclosed tag '<div>'"` |
| 36 | `html! { <div></span> }` | `"expected closing tag '</div>', found '</span>'"` |
| 37 | `html! { }` | `"empty input"` |
| 38 | `html! { <div><p></p> }` | `"unclosed tag '<div>'"` |
| 39 | `html! { <div class= ></div> }` | `"attribute 'class' has '=' but no value"` |

### Mount Integration (tests 40-42)

| # | Scenario | Verify |
|---|---|---|
| 40 | Mount Html with `Part::Text` | One effect created, initial `DomOp::SetText` queued |
| 41 | Mount Html with `Part::Event` | `DomOp::AddEventListener` queued with correct event name |
| 42 | Signal change after mount | Effect re-runs, new `DomOp::SetText` with updated value |
