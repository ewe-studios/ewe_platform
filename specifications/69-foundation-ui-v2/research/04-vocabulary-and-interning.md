# 04. Vocabulary and interning: numeric ids for tags, props and names

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); HyperText Markup Language (HTML);
Cascading Style Sheets (CSS); Document Object Model (DOM); identifier (id); AeroSpace and
Defence Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. The idea, laid out

Source: `foundation_ui_rearchitecture.md` §2 (`HtmlTagId`, `AttrNameId`, `CreateElementOp`,
`SetAttributeOp`) and §6 ("8 primitive layout families").

**Known names travel as small numbers. Unknown names travel as strings, as a fallback.**

```flatbuffers
enum HtmlTagId : ushort { UNKNOWN = 0, DIV = 1, SPAN = 2, INPUT = 3, BUTTON = 4 }
enum AttrNameId : ushort { UNKNOWN = 0, CLASS = 1, ID = 2, STYLE = 3, VALUE = 4 }
table CreateElementOp { node_id: uint; tag_id: HtmlTagId; custom_tag_name: string; class_name: string; }
table SetAttributeOp  { node_id: uint; attr_id: AttrNameId; custom_attr_name: string; value: string; }
```

- A `ushort` costs 2 bytes. `"button"` costs 6 bytes plus a length and an offset.
- A number compares in one instruction. The client maps it to a native constructor with a
  jump table, not a string match.
- `UNKNOWN = 0` plus `custom_*_name` keeps the schema open: a new name works before the enum
  is extended.
- The document also says the app ships a fixed set of **primitive layout families** (it says
  eight: buttons, text cells, inputs, and so on). The server arranges and updates them; it
  never adds new ones.

_North star fit: one vocabulary at the top, with the web and mobile variants under it (D36,
[13](./13-platform-capabilities.md))._

## 2. Current state

| Part | Where |
|---|---|
| Append-only `TAG_NAMES` and `ATTR_NAMES` wire id tables; ids 1 to 4 match the document | `backends/foundation_ui_traits/src/html.rs` |
| `DomOp`, 19 variants, HTML-shaped: tags, attributes, CSS classes, styles, JS properties, listeners, HTML morph | `backends/foundation_ui_traits/src/dom_op.rs:91-168` |
| Decision "no string keys" for signals | `specifications/completed/39-foundation-wasm-ui/decisions/011-no-string-keys.md` |
| 28 headless components that return `Html` | `backends/foundation_ui_components/src/` |
| No native primitive set | — |

## 3. The new way

### 3.1 One vocabulary, with a web mapping

The document's names are HTML names. A native renderer has no `DIV`, no `CLASS` and no
`STYLE`. The owner's review gives the rule (D10): **one vocabulary**. The core names work on
every platform. The web maps each core name to its HTML name. HTML tags and attributes are
custom names for the core names, not a second vocabulary.

| Layer | For | Elements | Properties |
|---|---|---|---|
| Core (every platform) | SwiftUI, Compose, the DOM, WebView | Stack, Text, Image, Button, TextInput, Toggle, List, Scroll, Spacer, Overlay (the primitive families) | Typed: padding, spacing, alignment, colour token, font token, accessibility label, enabled |
| Web-only subset | The DOM, when a screen needs a tag that the core does not have | HTML tags | HTML attributes, plus the `HtmlMorph` and `ScriptNode` ops (D8, D37) |
| Mobile-only subset | SwiftUI and Compose, when a screen needs a platform control that the core does not have | Native controls | Platform properties and effects, as `NativeControl` and `NativeCommand` (D36, [06](./06-native-rendering.md)) |

Mapping rules:

- The core name is the wire name. The web applicator maps it locally: `Stack` becomes a flex
  `div`, `Text` becomes a `span`, and so on.
- A core name is valid on every platform.
- A web-only name is valid on the web only. A native client reports it and shows a safe
  fallback ([11](./11-app-store-and-command-bus.md)). The web supports every name.
- A mobile-only name is valid on iOS and Android only. The two subsets are the platform
  variants of D36; they extend the one vocabulary, and they do not form a second one.
- The old `TAG_NAMES` and `ATTR_NAMES` tables become the web mapping. A known name keeps its
  number. A custom name falls back to a string. The numbers are append-only.

### 3.2 Typed properties, not strings

The document's `SetAttributeOp.value: string` makes the client parse `"16px"` or
`"#ff0000"`. The new way makes each property a typed union member:

```flatbuffers
union PropValue { Length: Length, ColorToken: ColorToken, FontToken: FontToken,
                  Alignment: Alignment, Bool: BoolValue, Text: TextValue }
table SetProp { node_id: uint; prop: PropId; value: PropValue; }
enum PropId : ushort { UNKNOWN = 0, PADDING = 1, SPACING = 2, ALIGN = 3, COLOR = 4,
                       FONT = 5, ENABLED = 6, A11Y_LABEL = 7 }
```

Design tokens (colour, font) are ids into a token table that ships in the app. A theme change
is a token table change, not a payload change.

### 3.3 The interning rule

- Every enum is append-only. An id is never reused or renumbered.
- `0` is always `UNKNOWN`. A client that reads an unknown id shows a safe fallback and reports
  it.
- The custom-name string fallback exists only in the web subset. The core vocabulary has no
  free-form names, because a native app cannot build an element it does not have
  ([11](./11-app-store-and-command-bus.md)).
- A native client that reads a web-only name reports it and shows a safe fallback. It never
  guesses a core name for it.
- The `ClientHello` carries the vocabulary version, so the server never sends ids that the
  client does not know ([02](./02-unified-wire-contract.md)).

## 4. What this changes for us

- `DomOp` does **not** stay as a second vocabulary (D10). Its 19 variants become the web subset
  of the one vocabulary, including the HTML morph. The `DomOp` Rust type is replaced by the
  schema types. The morph is the one web op that carries free-form markup
  ([01](./01-compile-time-targeting.md), [03](./03-payload-tiers.md)).
- Scoped JavaScript (JS) stays, web only. It is the behaviour path for an island. A native
  platform gets a compiled equivalent for the same behaviour
  ([12](./12-scoped-behaviour.md), [11](./11-app-store-and-command-bus.md)).
- The primitive families must be defined, with their properties, as a decision before code.
  `foundation_ui_components` behaviour (focus trap, dismiss, gestures) must be expressed with
  these primitives plus a platform binding, not only scoped JS.

## 5. Open questions

Q9 in [`questions.md`](./questions.md): the exact primitive set. Q10 is answered by D10: one
consistent vocabulary, extended with the web-only parts, with a string fallback at the bottom.
The web maps the core vocabulary to HTML by default.
