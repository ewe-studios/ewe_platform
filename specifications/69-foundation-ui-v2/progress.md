# Spec 69 progress

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** pull request (PR); user interface (UI); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## Landed

- 2026-10-27: research v1, as PR #157. One file per idea from the rearchitecture document.

- 2026-10-27: rework after owner comment. FlatBuffers became the top format (D7). The research
  split into eleven idea files under `research/`, each with the same five parts.
  `questions.md` holds 25 questions.

- 2026-11-12: the owner's review comments on PR #157 are applied.
  - D8: the HTML morph stays. It becomes `HtmlMorph`, a union member for the web target only.
    `MorphNode` and decision 027 are not retired.
  - `01` now answers "do we not already do this?" in §2. The compile-time id mechanism exists
    on the web. The new part is one schema and native readers.
  - `03` defines `HtmlMorph` in the `Op` union. `02`, `04`, `05` and `11` describe the web gate.
  - The stale bug notes are correct. `TokenSigner::verify` has a constant-time compare and a
    key id since commits `25cab64fe` and `540611b30`. The `docs/bug-backlog.md` file is gone.

- 2026-11-12, second review pass (13 comments):
  - D4 revised: the web comes first, with the `<mount-stream>` and `<mount-data>` paths
    unchanged. Native comes after.
  - D9: MessagePack stays as an opaque body type.
  - D10: one vocabulary. `DomOp` retires. The core names work on every platform, the web adds
    a web-only subset with a string fallback, and mobile reports a name it cannot render.
  - D11: a mobile app may mount a WebView. `ClientHello` gains `mount: Native | Web`.
  - D12: the web state rides as a base64url string.
  - New file `12-scoped-behaviour.md`: scoped JS stays on the web, and each behaviour gets a
    compiled native form.
  - `06` gains the WebView surface and the Rust-to-native command path, and no longer claims
    to rebuild the platform.

- 2026-11-12, third review pass (7 comments):
  - D13: the signed state is a handle, not the truth. The server can ignore the client copy and
    load the authoritative state from the backend. `07` §3.5 is new.
  - D14: single use is a domain rule. The server rejects a second request for the same resource
    (a payment, a delete). No nonce store and no used-id list. `08` §4.2 and §4.4, `08b` §3 and
    §4, and Q19 are changed. Q19 is now answered.
  - D15: the server decides how it handles state. It sends one update or a stream. The signal
    server, the `Broadcaster` and its replay backlog stay, and the research no longer presents
    them as a problem. `09` is rewritten around this, with a plain answer to "how is this any
    different?".
  - `09` title changes to "any replica, no per-user memory". Its current-state table marks the
    signal server, the broadcaster and the transport as keepers.

- 2026-11-12, fourth review pass (6 comments on `10`, all negative):
  - D16: an action returns a logical `Response`. The handler that delivers it decides the
    encoding. Encoding is not the action's business. `10` §1 and §3.3.
  - D17: `Ctx` wraps `foundation_context::Ctx`, so clocks and randomness pass down. `Context`
    wraps `Ctx` and adds action fields. `10` §3.4.
  - D18: actions are not remote procedure call (RPC) procedures. `10` §3.7 is rewritten, Q21 is
    withdrawn, and Q4 no longer ties the transport to ConnectRPC.
  - `10` §3.1 returns to the document's signature. The invented `ctx.authorize`, `ActionCtx`,
    `ActionResult` and `Result<ActionResult<T>>` are removed. The error path is Q28.
  - `10` §3.3 and §3.2 no longer claim a ConnectRPC client stub.
  - `questions.md` gains Q28 (how an action reports an error).

- 2026-11-12, fifth review pass (1 comment):
  - D19: the `#[action]` macro does more than register an action. It reads the signature and
    generates the decode of `Interaction<T>` and the call glue. `10` §3.2 is rewritten. The
    shared steps stay in the library.

- 2026-11-12, sixth review pass (4 comments):
  - D20 (answers Q1): element ids are `primal-id`, as today. An id is not state, so it needs no
    stability rule. A `View` or a `Div` has one fixed id; a custom element uses a string or a
    user-defined id. `01` §3 item 5 and §5 change.
  - D21 (answers Q2): the wire crate is `foundation_ui_wire`. `02` §3.1, §4 and §5 change.
  - D22: one registry model on web and device. The event names the action; the registry finds
    it; the action gets the node, the parent and the data we extract. The action can be local
    or remote.
  - D23: the registry is user-built. The macro generates a struct named after the function, and
    the app registers it. No generated global registry, no fixed list of kinds. `11` §1 and
    §3.2 are rewritten. Q30 asks about `#[remote_action]`.

- 2026-11-12, seventh review pass (30 comments and answers):
  - D24 (Q3): enforce 8-byte alignment. D25 (Q4): the transport is a boundary concern; SSE,
    HTTP, WebSocket and ConnectRPC are all possible. D26 (Q5): stored `foundation_ai` data is
    out of scope. `02` §2 and §5 change.
  - D27 (Q7): retire the hand-written columnar code when the consolidated form delivers the
    same web functionality; one representation for tier 1 and tier 2. D28 (Q8): keep Arrow IPC
    and the other data formats. `03` §3.2, §3.5 and §4 change.
  - Q10, Q12 to Q17 are answered; the recommendations stand. `04`, `06`, `07` and `08` record
    the answers.
  - D29 (Q11): the server, WASM and Rust all send frames; the native side processes them. `05`
    §3 changes.
  - D30 (Q20): use the existing `<mount-stream>` paths and header negotiation; do not
    prescribe a Durable Object. `09` §3.4 and §5 change.
  - D31: an action is identified by its name, not a derived numeric id. `02` §3.3 and §3.5,
    `09` §3.2, `10` §3.2 and §3.6, and `11` §3.2 change. Q22 is removed.
  - D32 (Q23): the framework delivers state updates or DOM/mobile UI updates. D33 (Q24): the
    native update policy is not a current concern. D34 (Q25): build on spec 52 and decide what
    to keep or remove as the work proceeds. Q23 and Q24 are removed; `11` §5 changes.
  - `10` §3.1 states that `Interaction<T>` is typed by its `T`, and `input_value` stays a
    `String` inside that typed struct.

- 2026-11-12, eighth review pass (north star, PR review 5868672793):
  - D35: the north star. One FlatBuffers contract carries structured UI, data and actions
    across web, mobile, WASM and compiled Rust. Do not redesign everything.
  - D36: one cross-platform form set, plus platform-only forms for what the set cannot
    express. A change has one form; there is no core form beside a web or mobile form. Each
    runtime applies the common forms, applies its own platform forms, and skips the rest.
    `02` §3.2 and §3.5, `03` §3.1 and `04` §3.1 change.
  - D37: one action model with a registry in Rust or JS. The web `<script>` functions that
    become callable after mount stay, join the JS registry and never reach mobile. `05` §3,
    `10` §3.8, `11` §3.2 and `12` §3.1 change.
  - D38: platform capability hooks. Shared standard at the top, platform sets at the edges.
  - D39: `mount-stream` and `mount-data` on mobile too, over local, SSE, WebSocket or HTTP.
    `06` §3.7 is new.
  - D40: build on the current shape; keep the agreed parts.
  - New file `13-platform-capabilities.md`: the standard, the shared runtime and the platform
    sets, the message variant rule, the action and behaviour hooks, the live mount contract and
    the reuse map.
  - `README.md` is rewritten around the north star, with the four goals, the layer diagram and
    D35 to D40. Every idea file gains a short north star note.
  - `questions.md` gains Q31: the exact platform scope syntax.

## In progress

- None. The research records the owner's answers in `questions.md`.

## Next

- The owner answers the remaining open questions: Q6, Q9, Q18, Q26, Q27, Q28, Q30 and Q31.
- The design specification starts from the answers.
