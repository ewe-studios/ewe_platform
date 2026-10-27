# Idea 5: native mobile rendering (SwiftUI, Jetpack Compose) through UniFFI or C bindings

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); server-driven user interface (SDUI);
Document Object Model (DOM); HyperText Markup Language (HTML); Cascading Style Sheets (CSS);
WebAssembly (WASM); JavaScript (JS); foreign function interface (FFI); application binary
interface (ABI); Unified Foreign Function Interface generator (UniFFI); Android Native
Development Kit (NDK); minimum viable product (MVP); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. What the document proposes

Source: `foundation_ui_rearchitecture.md` §3 lines 153-157, §6 line 212.

- Mobile apps do not use a WebView, a JS engine, or downloaded bytecode.
- The core runtime (`foundation_signals`, `foundation_ui_components`) compiles into the app as
  native `aarch64` code, through UniFFI or static C bindings.
- The native client reads a `MobileLayout` payload (a typed vector of layout operations) and
  maps each operation to a native declarative view: SwiftUI on iOS, Jetpack Compose on
  Android.
- The app ships "8 primitive layout families" (buttons, text cells, inputs, and so on) as
  compiled code. The server only arranges and updates them.

Owner decision D4: **native first.** This idea is the first build target.

## 2. What we have now

| Item | Location | State |
|---|---|---|
| `ViewKind::Native` enum variant | `backends/foundation_ui_traits/src/platform_types.rs:175-187` | Type only. Doc comment says "Post-MVP … MVP renders everything in WebViews." |
| Spec 52 `NativeViewRegistry` plan | `specifications/52-tauri-foundation-platform/decisions/01-platform-and-crates.md:181`, `:196` | Plan only, tied to Tauri hooks. Out of scope (D2). |
| UniFFI mentioned for native capability calls | `specifications/52-tauri-foundation-platform/decisions/07-native-capability-contract.md:40` | A comment in a plan. No code. |
| `foundation_signals` — reactive graph for any target | `backends/foundation_signals/src/lib.rs:1-10` | Builds natively. Usable on device. |
| `DomOp` vocabulary | `backends/foundation_ui_traits/src/dom_op.rs:91-168` | HTML-shaped: tags, attributes, CSS classes, styles, JS properties, event listeners, HTML morph. |
| `HtmlTag` / `AttrName` wire ids | `backends/foundation_ui_traits/src/html.rs` | HTML names. |
| Headless components | `backends/foundation_ui_components/src/` (28 component modules, 11 machinery modules) | Return `Html`. Behaviour is in scoped JS: `machinery/mod.rs:46` (`scoped_script`), used by 21 files including focus trap, dismiss, position, gestures. |
| UniFFI, cbindgen, Swift or Kotlin code | none | Not in any `Cargo.toml`. |

## 3. Gap

1. **No native renderer.** No Swift or Kotlin code exists. No binding layer exists.
2. **The vocabulary is HTML.** `DomOp` speaks tags, attributes, CSS classes and HTML strings.
   SwiftUI and Compose do not have these. The document's `MobileLayout` also uses
   `HtmlTagId` (`DIV`, `SPAN`, `INPUT`, `BUTTON`) and `AttrNameId` (`CLASS`, `STYLE`), so the
   document has the same problem. A native renderer needs a **layout vocabulary**: stack,
   text, image, button, input, list, scroll, spacer, and typed props (padding, alignment,
   colour token, font token). Not CSS.
3. **Imperative operations versus declarative views.** SwiftUI and Compose rebuild views from
   state. They do not expose "create element, append child". So the native side must keep a
   small node tree (id to node, with props and children) and publish it as observable state.
   Each operation edits this tree. The framework then recomposes. This is a retained model
   on the native side, not a virtual DOM diff, but it is a tree the client must hold.
4. **Components are web-only.** All 28 component modules return `Html`. Their behaviour (focus trap,
   dismiss, positioning, gestures) is JS. None of it runs natively. The "8 primitive layout
   families" in the document do not exist in the repository. We must define them.
5. **Events.** Today `primal:onX` binds to a local signal setter (`events.rs`). Native needs a
   tap or change callback from SwiftUI or Compose back into Rust (or straight to the server),
   with the node id and the signed state of that node.
6. **Build.** No iOS or Android targets are in the workspace build. Spec 62
   (`specifications/completed/62-macos-native-build`) covers macOS only.

## 4. Open risks

1. **UniFFI versus C ABI.** UniFFI generates Swift and Kotlin bindings from Rust. It is
   mature. It copies data across the boundary for most types, so a zero-copy read of the
   layout buffer needs a byte-buffer type and a FlatBuffers reader on the Swift or Kotlin
   side. A plain C ABI can pass a pointer and a length. It needs hand-written Swift and
   Kotlin glue. Rule 5 prefers generated types, which favours UniFFI plus generated
   FlatBuffers readers.
2. **Where the layout is decoded.** Option A: Rust decodes frames and calls native view
   builders through callbacks. Option B: Swift or Kotlin decodes FlatBuffers directly (code
   generated for each language) and Rust is not on the render path. Option B is simpler for
   the render path. Option A keeps one decoder. This is question Q5.
3. **Rust on device at all.** If Swift and Kotlin decode the layout, the only Rust on device
   may be the action registry and local signals. That must justify the binary size and the
   build cost. The document assumes Rust on device. We must confirm we need it.
4. **Accessibility and platform look.** Native primitives give accessibility and platform
   look for free. A wide generic vocabulary that copies HTML loses this. Keep the vocabulary
   small and semantic.
5. **Testing.** Spec 52 testing plans are Tauri-based (D2 removes them). Native rendering
   needs its own test plan: snapshot tests of the node tree in Rust, plus simulator tests.

## 5. Conflict with earlier decisions

- `ViewKind::Native` is "post-MVP" and "MVP renders everything in WebViews"
  (`platform_types.rs:180-187`). Per D3 and D4 this is reversed: native is first.
- `DomOp::MorphNode` and decision 027 (DOM morphing with HTML strings) cannot serve native.
  Native needs typed subtree replacement.

## 6. Recommendation: **adapt**

- **Adopt** native rendering with SwiftUI and Jetpack Compose and no WebView.
- **Do not reuse `DomOp` for native.** Define a separate, small, semantic layout vocabulary
  (`LayoutOp` over a fixed set of primitives) as FlatBuffers tables. Keep `DomOp` for the
  web, later.
- **Native side keeps a node tree** (id to node), edited by layout operations and published as
  observable state. Say this plainly in the design, so nobody calls it "no client state".
- **Pick the binding** after a short spike: one screen, one button, one action round trip, on
  both platforms, with UniFFI. See Q5 and Q6.
- **Define the primitive set** as a decision before any code. The document says eight; the
  number must come from the screens we need. See Q7.
