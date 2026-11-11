# Spec 69 research: questions for the owner

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); WebAssembly (WASM); JavaScript (JS);
TypeScript (TS); Server-Sent Events (SSE); remote procedure call (RPC); identifier (id);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

Each open question has options and a recommendation (**Rec.**). Answer in the PR. The design
specification starts from the answers. An answered question stays here as a short record. It
is not open.

## Element identity ([01](./01-compile-time-targeting.md))

**Q1. How are element ids derived?**
**Answered (2026-11-11): `primal-id`, as today.**
- An element id is not state, so it needs no stability rule.
- The template macro pre-assigns the id in the code. A `View` or a `Div` has one specific set
  id.
- Anything else is a custom element: a string or a user-defined custom id.

## Wire contract ([02](./02-unified-wire-contract.md))

**Q2. Where do the schema and the generated code live?**
**Answered (2026-11-11): a new `foundation_ui_wire` crate.** It holds the `.fbs` files and the
generated Rust. The generated TypeScript, Swift and Kotlin packages sit beside it. Generated
code is checked in, and CI regenerates it with a pinned `flatc` and fails on a difference.

**Q3. Vector alignment.** The zero-copy columns need 8-byte alignment.
**Answered (2026-11-11): enforce 8 bytes.** Use `force_align: 8` on numeric vectors and
confirm it in a spike in all four generated languages. A trailing aligned block is not needed
if every reader enforces the attribute.

**Q4. Transport framing.**
**Answered (2026-11-11): the transport is a boundary concern.** The framework does not choose
a transport. We already support SSE, HTTP and WebSocket, and ConnectRPC is also possible.
Header negotiation already tells the client which transport and data format comes next. The
user decides what their server serves.

D9 keeps MessagePack as an opaque body type, so it has no question.

## Payload tiers ([03](./03-payload-tiers.md))

**Q6. Default threshold between tier 1 (`OpBatch`) and tier 2 (`ColumnarBatch`)?**
**Rec.** Configuration, default 64 operations, set from the benchmark.

**Q7. When do we retire the hand-written columnar code?**
**Answered (2026-11-11): when the new form delivers the same functionality on the web.** The
new consolidated representation serves tier 1 and tier 2. Retire the hand-written code when
the consolidated form matches it, and keep one representation for both tiers.

**Q8. Arrow IPC on the UI wire?**
**Answered (2026-11-11): keep it.** We support the different data formats that a caller wants
to use. Arrow IPC stays as a union member for data that goes to Arrow tools. `DataTable` does
not replace it.

## Vocabulary ([04](./04-vocabulary-and-interning.md))

**Q9. What is the primitive set?** The document says eight families. The research proposes:
Stack, Text, Image, Button, TextInput, Toggle, List, Scroll, Spacer, Overlay.
**Rec.** Decide from the first three real screens, then freeze version 1.

**Q10. Does the web render the core vocabulary by default?**
**Answered by D10 (2026-11-11): yes.** One consistent vocabulary, extended with the parts that
only the web has, and a string fallback at the bottom. The web maps the core names to HTML
names; the web-only subset is for tags that the core does not have.

## Web applicator ([05](./05-web-zero-copy-applicator.md))

**Q11. Do frames from local WASM logic use the same `Frame` schema and the same applicator?**
**Answered (2026-11-11): yes.** We build the native side to process the frames. So the
server, WASM and Rust all send frames, and the receiver applies them with the same schema and
the same applicator. The 14-byte `WasmEnvelope` goes away.

## Native rendering ([06](./06-native-rendering.md))

**Q12. Who reads frames on device?**
**Answered (2026-11-11): (a).** Swift and Kotlin generated readers edit the node store. Rust
is for logic only. This starts after the web works (D4).

**Q13. Minimum OS versions?**
**Answered (2026-11-11): iOS 17 and Android API 26**, unless a customer needs older. The
observation API is `@Observable` on iOS 17 and the current Compose version on Android.

**Q14. First spike.** One screen, one button, one server action round trip. Do it on the web
first, then on both native platforms (D4).
**Answered (2026-11-11): yes**, before the design spec is final.

## Element-riding state ([07](./07-element-riding-state.md))

**Q15. Default state size cap per component?**
**Answered (2026-11-11): configuration, default 4 KiB.** State is ids and flags, not data.

**Q16. Private state: opt-in or default?**
**Answered (2026-11-11): (a).** HMAC by default; authenticated encryption when a component
marks its state private. Most state is ids and flags; encryption adds cost and hides it from
debug tools.

## State security ([08](./08-state-security-and-replay.md))

**Q17. Reading of D5 for browser WASM.**
**Answered (2026-11-11): yes.** A key in the browser can be read by the user. So state signed
by browser WASM is checked only by that same WASM runtime, and a remote server never accepts a
client-made tag.

**Q18. Default expiry of a state token?**
**Rec.** Configuration, default 24 hours, with a reload on expiry. LiveView uses 14 days for
its session token, which carries no view state.

**Q19. Store for single-use nonces?**
**Answered by D14 (2026-11-11): none.** The domain data owns single use. The server knows when
it first creates a payment or accepts a delete. A second request for the same resource is
rejected by the domain data. The framework adds no nonce store and no used-id list.

## Stateless execution ([09](./09-stateless-execution.md))

**Q20. Where do live streams run?**
**Answered (2026-11-11): use what we already have.** We do not reinvent the stream path. Live
streams use the existing `<mount-stream>` paths and the header negotiation. A client can
connect specifically for SSE or for WebSocket. The server does not get a prescribed Durable
Object, thread or replica layout.

## Actions ([10](./10-action-model.md))

**Q21. Are actions ConnectRPC procedures?**
**Withdrawn by D18 (2026-11-11).** Actions are not RPC procedures. `#[action]` is a plain
function attribute, and it does not build on `#[service]`.

**Q28. How does an action report an error?** The client gets an `Error` frame
([02](./02-unified-wire-contract.md)). The document's `Response` has no error case.
- (a) Add one case to the same enum: `Response::Error(ActionError)`.
- (b) The library wraps the call and builds the `Error` frame from a returned error value.

**Rec.** (a). It keeps one return type, in the enum style of the document.

## App Store and command bus ([11](./11-app-store-and-command-bus.md))

**Q25. What happens to spec 52 and `foundation_platform`?** (D2, D3)
**Answered (2026-11-11): they have valid use.** We build on them. As the work proceeds, we
decide what to keep, what to remove, and what stays for later. Do not freeze them by default.

**Q30. Do we add a `#[remote_action]` marker?** The owner proposes it as a thought
(2026-11-11). A remote action marks a client call that delivers the name and the data to the
remote side. The client uses the same registry shape.
- (a) Yes, as a second marker beside `#[action]`.
- (b) One marker with a `remote` option.

**Rec.** (a), as proposed, if the generated client call differs from a local one. Decide when
the first remote action exists.

## WebView mount ([06](./06-native-rendering.md))

**Q26. When does an app use the WebView mount?** (D11)
- (a) A whole screen only.
- (b) Also island regions inside a native screen.

**Rec.** (b). It is the same web runtime, and `ClientHello.mount` already tells the server.

## Scoped behaviour ([12](./12-scoped-behaviour.md))

**Q27. Which behaviours get a native binding first?** The ten modules are `dialog`, `dismiss`,
`focus_trap`, `gestures`, `hover`, `listbox`, `measure`, `position`, `scroll_lock` and
`transition`.
**Rec.** Only the ones that the first native screens use. The rest come with their screens, and
the build check keeps the matrix complete.
