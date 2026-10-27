# Idea 9: App Store Guideline 2.5.2 compliance by design

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); server-driven user interface (SDUI);
WebAssembly (WASM); JavaScript (JS); over the air (OTA); AeroSpace and Defence Industries
Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. What the document proposes

Source: `foundation_ui_rearchitecture.md` §6, lines 240-246.

Apple App Store Review Guideline 2.5.2 says an app must be self-contained. It must not
download, install or run code that adds or changes features. Google Play has a similar
policy on code downloaded from outside Play. The document meets this with three rules:

1. **Data, not code.** The server sends declarative layout data. It never sends scripts,
   bytecode or binaries.
2. **Compiled primitives.** All UI primitives are compiled into the app. The server only
   arranges them and sets their values.
3. **Static action registry.** An event names an action by id. The app checks the id against
   a static registry of compiled actions. Nothing is interpreted.

## 2. What we have now

| Item | Location | Comment |
|---|---|---|
| Spec 52 deployment surfaces 1 and 3: "WASM hot-swap, no review" | `specifications/52-tauri-foundation-platform/decisions/04-deployment-surfaces.md:32-34`, `:715` | Downloaded WASM that changes app logic. This is the case 2.5.2 is about. |
| Spec 52 signed manifests before WASM hot-swap | `specifications/52-tauri-foundation-platform/decisions/08-security-model.md:149` | Integrity, not policy compliance. |
| OTA module in `foundation_platform` | `backends/foundation_platform/src/ota.rs` | Part of the Tauri platform crate (out of scope, D2). |
| Components with behaviour in scoped JS | `backends/foundation_ui_components/src/machinery/mod.rs:46` | JS that the server can send inside markup. On the web this is normal. In a native app it would be downloaded code. |
| `DomOp::MorphNode` content is an HTML string | `backends/foundation_ui_traits/src/dom_op.rs:159` | HTML can hold `<script>`. Web only. |

There is no on-device action registry and no native primitive set.

## 3. Gap

1. The current plan (spec 52) depends on code hot-swap. The new direction removes it for
   native apps.
2. Our payloads can carry HTML and JS today. The native layout vocabulary must not have any
   field that holds code, markup or an expression language.
3. No registry of compiled actions exists. See [`action-macro.md`](action-macro.md).

## 4. Open risks

1. **Grey zone.** Apple allows server-driven layout and content. Apple reviewers judge
   "changes features" case by case. A payload that can build any screen from primitives can
   still be seen as a way to ship new features without review. Mitigation: keep the
   vocabulary semantic and limited. Do not add conditionals, loops, or an expression
   language to the layout format.
2. **Local logic.** If the app needs offline logic, it must be compiled into the app (Rust
   through the binding layer). It cannot be a downloaded WASM module.
3. **Version skew.** An old app gets a payload that names a primitive or action it does not
   have. The client must show a fallback (for example "update the app") and must not crash.
   FlatBuffers unions give unknown members a safe "NONE" read.
4. **Web is different.** On the web, downloaded JS and WASM are normal. The rules here are for
   native app stores only.

## 5. Conflict with earlier decisions

Per owner decision D3: spec 52 surfaces 1 and 3 ("WASM hot-swap, no review") do not apply to
the native-first direction. OTA updates of data and assets (images, strings, layout) stay
allowed. OTA updates of logic do not.

## 6. Recommendation: **adopt**

- **Adopt** all three rules: data only, compiled primitives, static action registry.
- **The action registry** is generated from the same macro as server actions, so ids match.
  An action can be "server" (send to server), "local" (compiled Rust on device) or both.
- **Add a capability handshake.** The client sends its layout-vocabulary version and its
  action registry version at connect. The server sends only what the client supports.
- **Write a one-page compliance note** in the design spec that lists every payload field and
  shows that none holds code.

See Q16 in [`questions.md`](questions.md).
