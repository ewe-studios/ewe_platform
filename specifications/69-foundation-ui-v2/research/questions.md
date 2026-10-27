# Open questions for the owner

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); hash-based message authentication code
(HMAC); authenticated encryption with associated data (AEAD); WebAssembly (WASM); foreign
function interface (FFI); application binary interface (ABI); Unified Foreign Function
Interface generator (UniFFI); Remote Procedure Call (RPC); time to live (TTL); key-value
(KV); over the air (OTA); pull request (PR); AeroSpace and Defence Industries Association of
Europe (ASD), the owner of the STE standard ASD-STE100.

Answer in the PR. Each question gives options and a recommendation (**Rec.**). The file
named after each question has the detail.

## Wire format

**Q1. Where do the schema and the generated code live?** ([`wire-envelope.md`](wire-envelope.md))

The generator is `flatc`, because it is the one tool that generates Rust, TypeScript, Swift and
Kotlin from one schema (D7). `planus` generates only Rust.

- (a) A new `foundation_ui_wire` crate holds the `.fbs` files and the generated Rust code.
  A build script (or a checked-in generation step, like our other generators) writes the
  TypeScript, Swift and Kotlin packages next to it.
- (b) The `.fbs` files in `foundation_ui_traits` (adds a dependency to a zero-dependency
  `no_std` crate).

**Rec.** (a), with the generated code checked in and a CI check that regenerates and compares,
so a build does not need `flatc` installed.

**Q2. Drop MessagePack from the body union?** ([`wire-envelope.md`](wire-envelope.md))
- (a) Drop. It has no schema, so it cannot be generated for every language (D7).
- (b) Keep as a union member for schema-less data.

**Rec.** (a).

**Q3. State format on the wire.** ([`wire-envelope.md`](wire-envelope.md))
- (a) The document's eight-way `StateFormatPayload` union.
- (b) Opaque bytes plus a token table. The server picks one encoding (a generated
  FlatBuffers table per component).

**Rec.** (b). The client never reads state, so the format is a server concern.

**Q4. When do we retire the hand-written columnar code?** ([`columnar-zero-copy.md`](columnar-zero-copy.md))
- (a) When the generated FlatBuffers columnar table matches `ColumnarBatch` and the JavaScript
  `ColumnarParser` in a benchmark (1, 10, 100 and 10 000 operations).
- (b) At once, with no benchmark.

**Rec.** (a). The schema type is built first for native; the web switches after the benchmark.

## Native rendering

**Q5. Where is the layout decoded on device?** ([`native-mobile-rendering.md`](native-mobile-rendering.md))
- (a) In Rust. Rust calls Swift or Kotlin view builders through callbacks.
- (b) In Swift and Kotlin, with generated FlatBuffers readers. Rust is not on the render path.

**Rec.** (b) for the render path. Rust on device holds only local actions and signals, if
we need them (see Q6).

**Q6. Binding layer, and do we need Rust on device for the first version?**
- (a) UniFFI.
- (b) Plain C ABI (pointer plus length) with hand-written glue.
- (c) No Rust on device for the first version. Server actions only.

**Rec.** Spike (a) with one screen and one action round trip. Consider (c) for the first
version if local actions are not needed yet.

**Q7. What is the primitive set?** The document says "8 primitive layout families". The
repository has none for native.
- (a) Fix eight now (for example: stack, text, image, button, text input, toggle, list,
  scroll).
- (b) Derive the set from the first two or three real screens.

**Rec.** (b), then freeze it as a decision. Keep it semantic, not HTML-shaped.

## Signed state

**Q8. What does decision D5 mean for WASM in the browser?** ([`stateless-signed-state.md`](stateless-signed-state.md))
A key in browser WASM is readable by the user.
- (a) Browser WASM signs only for its own local loop. A remote server never accepts a
  browser-made tag.
- (b) Browser WASM never signs. Only remote servers sign.

**Rec.** (a). Please confirm this is what D5 means.

**Q9. One token per element or per component?**
- (a) Per element (the document).
- (b) Per component root, with the node id inside the signed data (Livewire).

**Rec.** (b). Smaller frames, and actions act on components.

**Q10. HMAC or AEAD by default?**
- (a) HMAC-SHA256 always. State is readable.
- (b) AEAD (ChaCha20-Poly1305) always. State is secret.
- (c) HMAC by default. AEAD when a component marks its state as private.

**Rec.** (c).

## Replay protection

**Q11. Session binding.** ([`replay-protection.md`](replay-protection.md))
- (a) Bind every token to a hash of the session id. Anonymous users get an anonymous
  session.
- (b) Bind only for signed-in users.
- (c) No binding (Livewire).

**Rec.** (a). It stops cross-user replay at low cost.

**Q12. Single-use actions.**
- (a) No single-use support. Rely on expiry and domain row versions.
- (b) Opt-in single-use nonce per action, with a shared store (which one: `foundation_db`,
  a KV store, other).
- (c) Single-use for every action.

**Rec.** (b). Please name the shared store.

**Q13. Token expiry default.** LiveView uses 14 days for its session token.
- (a) Short (for example 30 minutes), with a silent refresh on each response.
- (b) Long (for example 24 hours or 14 days).

**Rec.** (a). Each response carries a fresh token, so an active page never expires. The value
is configuration (rule 8).

## Actions

**Q14. Are actions ConnectRPC procedures, or a separate router?** ([`action-macro.md`](action-macro.md))
- (a) ConnectRPC procedures generated by an `#[action]` macro on top of the existing
  code-first `#[service]` machinery.
- (b) A separate action router and endpoint.

**Rec.** (a), if the ConnectRPC codecs can carry the FlatBuffers action envelope. It reuses
typed clients, limits and cancellation.

**Q15. Who picks the view vocabulary in a response?**
- (a) The action returns a neutral view and the framework encodes it per client (native
  layout ops or web ops).
- (b) The action returns one vocabulary and the client must support it.

**Rec.** (a), with native layout ops as the only target in the first version.

## Compliance and old plans

**Q16. OTA policy for native apps.** ([`app-store-compliance.md`](app-store-compliance.md))
- (a) OTA for data and assets only. No downloaded logic (no WASM hot-swap on native).
- (b) Keep WASM hot-swap as an option for apps outside the app stores.

**Rec.** (a) for store builds. (b) can come later behind a build flag, if a non-store use
case appears.

**Q17. Stateless actions and stateful streams.** ([`stateless-signed-state.md`](stateless-signed-state.md))
- (a) Two named modes: stateless actions (default) and stateful streams (existing `App` +
  `Broadcaster`).
- (b) Stateless only. Streams also re-sign state on each frame.

**Rec.** (a).

**Q18. What happens to spec 52 and `foundation_platform`?** (D2, D3)
- (a) Freeze spec 52. Mark the decisions this research overrides. Leave the crate as is.
- (b) Archive spec 52 and remove the Tauri parts of `foundation_platform`.

**Rec.** (a) until the spec 69 design is accepted.

## Added after the PR #157 review

**Q19. Does "FlatBuffers is the top format" also apply to stored `foundation_ai` data?** ([`wire-envelope.md`](wire-envelope.md) §5)

Spec 36 decision 10 stores conversation data (messages, tool calls, memory snapshots, events)
as JSON and Arrow. It is not about the UI.
- (a) UI wire only. Decision 10 stays as it is.
- (b) Everywhere: stored conversation data also gets a FlatBuffers schema, with JSON and
  Arrow as derived views. Decision 10 is rewritten.

**Rec.** (a) for this spec, and (b) as a separate decision if you want it, because it changes
`foundation_ai` storage and its tests.

**Q20. What happens to Arrow IPC for UI frames?** ([`wire-envelope.md`](wire-envelope.md) §4 risk 5)
- (a) Drop it from the UI wire. The columnar FlatBuffers table covers UI frames.
- (b) Keep it as a union member that holds Arrow IPC bytes, for data that goes to Arrow tools.

**Rec.** (a) for UI frames. Use (b) only when a real Arrow consumer exists.
