# Spec 69 research: questions for the owner

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); WebAssembly (WASM); JavaScript (JS);
TypeScript (TS); Server-Sent Events (SSE); Durable Object (DO); over the air (OTA); identifier
(id); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE
standard ASD-STE100.

Each question has options and a recommendation (**Rec.**). Answer in the PR. The design
specification starts from the answers.

## Element identity ([01](./01-compile-time-targeting.md))

**Q1. How are element ids derived so they stay stable?**
- (a) Hash of component type plus the element path in its template. A moved element gets a new
  id; ids are never reused.
- (b) A counter per template, as today (`primal-id`).

**Rec.** (a). An old client and a new server must agree on ids.

## Wire contract ([02](./02-unified-wire-contract.md))

**Q2. Where do the schema and the generated code live?**
- (a) A new `foundation_ui_wire` crate with the `.fbs` files and generated Rust; generated TS,
  Swift and Kotlin packages beside it; generated code checked in; CI regenerates with a pinned
  `flatc` and fails on a difference.
- (b) In `foundation_ui_traits` (adds a dependency to a zero-dependency `no_std` crate).

**Rec.** (a).

**Q3. Vector alignment.** The zero-copy columns need 8-byte alignment.
- (a) `force_align: 8` on numeric vectors, confirmed by a spike in all four generated languages.
- (b) Put columns after the FlatBuffers table as a trailing aligned block.

**Rec.** (a); fall back to (b) only if a generator ignores the attribute.

**Q4. Transport framing.**
- (a) WebSocket for live streams and actions; HTTP POST for actions where WebSocket is not
  open.
- (b) HTTP/2 or HTTP/3 streams through ConnectRPC for both.

**Rec.** (b) for actions (it reuses the ConnectRPC stack, see Q21), (a) or SSE for browser
push where ConnectRPC streaming is not available.

**Q5. Does "FlatBuffers is the top format" also cover stored `foundation_ai` data?**
- (a) UI wire only; spec 36 decision 10 stays.
- (b) Everywhere; stored conversation data gets a FlatBuffers schema, and decision 10 is
  rewritten.

**Rec.** (a) for this spec, (b) as its own decision if you want it.

## Payload tiers ([03](./03-payload-tiers.md))

**Q6. Default threshold between tier 1 (`OpBatch`) and tier 2 (`ColumnarBatch`)?**
**Rec.** Configuration, default 64 operations, set from the benchmark in Q7.

**Q7. When do we retire the hand-written columnar code?**
- (a) When the generated form matches it in a benchmark (1, 10, 100, 10 000 operations).
- (b) At once.

**Rec.** (a).

**Q8. Arrow IPC on the UI wire?**
- (a) Drop it; `DataTable` covers UI data.
- (b) Keep an `ArrowIpc` union member for data that goes to Arrow tools.

**Rec.** (a) for UI frames; add (b) when a real Arrow consumer exists.

## Vocabulary ([04](./04-vocabulary-and-interning.md))

**Q9. What is the primitive set?** The document says eight families. The research proposes:
Stack, Text, Image, Button, TextInput, Toggle, List, Scroll, Spacer, Overlay.
**Rec.** Decide from the first three real screens, then freeze version 1.

**Q10. Does the web render the layout vocabulary by default?**
- (a) Yes; the web vocabulary (HTML tags) is only for pages that need raw HTML.
- (b) No; the web keeps HTML as its main vocabulary.

**Rec.** (a). One vocabulary for most screens on every platform.

## Web applicator ([05](./05-web-zero-copy-applicator.md))

**Q11. Do frames from local WASM logic use the same `Frame` schema and the same applicator?**
**Rec.** Yes. One format, one applicator; the 14-byte `WasmEnvelope` goes away.

## Native rendering ([06](./06-native-rendering.md))

**Q12. Who reads frames on device?**
- (a) Swift and Kotlin generated readers edit the node store; Rust is for logic only.
- (b) Rust decodes and calls the platform through UniFFI.

**Rec.** (a).

**Q13. Minimum OS versions?** They decide the observation API (iOS 17 `@Observable` or older
`ObservableObject`; Compose version).
**Rec.** iOS 17 and Android API 26, unless a customer needs older.

**Q14. First spike.** One screen, one button, one server action round trip, on both platforms.
**Rec.** Yes, before the design spec is final.

## Element-riding state ([07](./07-element-riding-state.md))

**Q15. Default state size cap per component?**
**Rec.** Configuration, default 4 KiB (state is ids and flags, not data).

**Q16. Private state: opt-in or default?**
- (a) HMAC by default; authenticated encryption when a component marks state private.
- (b) Authenticated encryption always.

**Rec.** (a). Most state is ids and flags; encryption adds cost and hides it from debug tools.

## State security ([08](./08-state-security-and-replay.md))

**Q17. Reading of D5 for browser WASM.** A key in the browser can be read by the user. So
state signed by browser WASM is checked only by that same WASM runtime, and a remote server
never accepts a client-made tag. Is this the intended meaning?
**Rec.** Yes.

**Q18. Default expiry of a state token?**
**Rec.** Configuration, default 24 hours, with a reload on expiry. LiveView uses 14 days for
its session token, which carries no view state.

**Q19. Store for single-use nonces?**
- (a) A `foundation_db` key-value store with expiry (Durable Object or D1 on Cloudflare,
  Turso natively).
- (b) In memory per replica (not safe across replicas).

**Rec.** (a), only for actions marked single-use.

## Stateless execution ([09](./09-stateless-execution.md))

**Q20. Where do live streams run?**
- (a) Any replica, with a sticky connection.
- (b) One Durable Object per stream on Cloudflare; one task per stream natively.

**Rec.** (b) on Cloudflare, (a) natively.

## Actions ([10](./10-action-model.md))

**Q21. Are actions ConnectRPC procedures?**
- (a) Yes: `#[action]` reuses the `#[service]` machinery; one router and one interceptor chain.
- (b) A separate action router.

**Rec.** (a).

**Q22. How are action ids derived?**
**Rec.** Hash of component type and action name, checked for collisions at build time, never
reused.

## App Store and command bus ([11](./11-app-store-and-command-bus.md))

**Q23. Is `LocalThenServer` (optimistic local update, then server confirm) in version 1?**
**Rec.** No. Start with `Local` and `Server`; add optimistic updates when a screen needs them.

**Q24. OTA policy for native apps.**
**Rec.** Data, assets, design tokens and flags may update over the air. Logic never does.

**Q25. What happens to spec 52 and `foundation_platform`?** (D2, D3)
- (a) Freeze them; the native work starts as new crates.
- (b) Fold the useful parts (native capability contract) into the new design later.

**Rec.** (a) now, (b) when Tauri comes back into scope.
