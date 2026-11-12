# Spec 69 learnings

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** Document Object Model (DOM); HyperText Markup Language (HTML);
AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE standard
ASD-STE100.

- Compile-time targeting already works on the web. `html!` writes `primal-id`, `Part::node_id`
  and `DomOp` carry the number, and the JS `NodeRegistry` maps it to the DOM node. The new
  work is one schema for the ids and a native reader. Do not present the mechanism as new.

- The HTML morph is a reconciler, not a second page tree. It compares one received fragment
  with one live subtree. Its cost is bounded by the fragment. It is worth keeping, because it
  gives form preservation and move detection on the web. Express it as one schema type,
  `HtmlMorph`, and gate it by the `mount` field of `ClientHello`. A native reader must
  never render its string or run its scripts.

- An HTML string cannot be a typed operation for native. The schema can hold it, but only as a
  web-only union member. Every place that lists payload fields must name this exception, or the
  App Store 2.5.2 note is wrong.

- `flatc` generates Rust, TypeScript, Swift and Kotlin from one `.fbs` file. `planus` generates
  Rust only, so it cannot be the generator (D7).

- An HTML attribute holds text only. Binary state cannot ride it. The web state is a
  base64url string (D12). A JavaScript expando property can hold bytes, but the markup cannot
  see it and it does not survive a server render or a reload.

- A second vocabulary for the web is not needed. One core vocabulary plus a web-only subset
  with a string fallback is simpler (D10). Mobile reports a web-only name it cannot render,
  and the web supports every name.

- The web island machinery (`<mount-stream>`, `<mount-data>` and scoped JS) is a feature, not
  legacy. Keep it (D4, D11), and give each behaviour the same compiled form on native
  ([12](./research/12-scoped-behaviour.md)).

- The frame apply is synchronous. Recomposing the views is the platform render pass, and
  SwiftUI and Compose own it. Do not add a scheduler between the two.

- Signed state is not authority. The tag proves who made the bytes; it does not prove the
  values are current or that the user may use them. The server can ignore the client copy, keep
  a key from it and load the truth from the backend (D13). Design state as a handle.

- Single use belongs to the domain data. A payment is created once, and a delete changes a row
  once. A framework nonce store repeats a fact that the domain already owns, and adds a second
  store and a failure mode (D14). Do not add one.

- Do not design out the signal server or the replay backlog. The owner calls them good
  properties, and signals cannot move across threads (D15). A stateless mode is an addition for
  actions that do not need a live graph, never a replacement.

- One response and a stream were always both possible. A change in where state lives between
  requests does not change the wire shape (D15). Say what is new and what is unchanged, or the
  design looks like a removal.

- The framework does not prescribe a store, a cache or a session table. It supplies the
  request, the signed state, the action registry and the frame builder; the server decides the
  rest (D15).

- Do not invent an application programming interface (API) the source document does not have.
  `ctx.authorize`, `ActionCtx`, `ActionResult` and a nested `Result` all came from the research,
  not from the document, and the owner rejected them. Quote the document's signature and put a
  real gap in `questions.md`.

- Encoding sits after the action. The action returns a logical `Response`; the handler that
  delivers it chooses ops, columnar, JSON or MessagePack (D16). Do not give the action an
  encoder, and do not let the action choose a wire format.

- `Ctx` wraps `foundation_context::Ctx`. Clocks and randomness pass down through it, and
  `Context` wraps `Ctx` and adds what the action needs (D17). Do not make a parallel context.

- The `#[action]` macro is not a registry stub. It reads the signature, so the request decode
  and the call glue are generated code (D19). Put only the steps that are the same for every
  action in the library.

- An element id is not state. `primal-id` is built into the code, so it needs no stability rule.
  A `View` or a `Div` has one fixed id; a custom element uses a string or a user-defined id
  (D20, Q1).

- Name the crate. `foundation_ui_wire` (D21, Q2).

- One registry model on every surface. The event names the action; the registry finds it; the
  action gets the node, its parent and the data. The action can be local or remote, and the
  caller does not care (D22, D23). The app builds its own registry from macro-generated structs;
  do not generate a global registry or a list of action kinds.

- The owner did not ask for remote procedure calls (RPC). `#[action]` is a plain function
  attribute; `#[service]` stays for code that wants RPC (D18). Do not build the simple idea on
  a router because a macro exists.

- The transport is a boundary concern (D25). We already serve SSE, HTTP and WebSocket, and
  ConnectRPC is possible. Do not pick one for the user, and do not build a transport decision
  into the schema.

- An action is named, not numbered (D31). The macro-generated struct carries the function
  name, and that name is the lookup tag. A hash-based numeric id is invented work.

- The framework delivers a result. It sends state updates or DOM/mobile UI updates, and it
  does not decide the delivery strategy (D32). Do not give it opinions about local then
  remote, optimistic updates or an update policy.

- Do not reinvent what exists. Live streams already use `<mount-stream>` and header
  negotiation, and a client can connect for SSE or WebSocket (D30). The same rule applies to
  stored `foundation_ai` data, which is out of scope (D26).

- `Interaction<T>` is typed by its `T`. The generic names the payload type, so the macro
  decodes a typed interaction. `input_value` stays a `String` inside that struct; it is the raw
  event value, not an untyped design (Q29 is not a question).

- One consolidated representation serves tier 1 and tier 2 (D27). The hand-written columnar
  code retires when the new form delivers the same functionality on the web, not on a
  benchmark date. The different data formats, Arrow included, stay (D28).

- Spec 52 and `foundation_platform` have valid use (D34). Build on them and decide what to
  keep, remove or leave for later as the work proceeds. Do not freeze them as a reflex.

- Lead with the north star (D35). The research is one contract that carries UI, data and
  actions across every surface. Each idea file serves that contract; an idea that reads like a
  separate redesign makes the set look like over-architecture.

- One message can hold a core form, a web form and a mobile form (D36). The schema has the
  three slots in one row; the reader takes its own and skips the rest. Do not split the schema
  per platform and do not force the server to write one batch per surface.

- Keep the current web capability for script actions (D37). A `<script>` beside the markup
  defines functions that become callable after mount. They join the JS registry and stay on
  the web. Mobile never runs them. The action model is the same on every surface, and the
  registry has two hosts: Rust and JS.

- Reuse is a layer split, not a copy (D38). The standard and the shared runtime are one. Each
  platform implements a capability behind the same interface. An engineer adds a platform hook
  only where the surfaces differ.

- `mount-stream` and `mount-data` are core ideas, not web features (D39). Mobile uses a live
  mount record with the same four fields and the same frame schema. Only the representation
  and the channel host differ.

- The owner's review is a lens, not a delete request. Keep the agreed parts and make them fit
  the north star (D40). Do not drop the schema, the morph, the islands or the live mode to look
  simpler.

- When the owner rejects a part strongly, delete it and return to the source document. Do not
  defend the invented part with more detail.

- Remove nothing the owner did not ask to remove. MessagePack stays as an opaque body (D9).
  The morph stays as `HtmlMorph` (D8).

- The old `docs/bug-backlog.md` file no longer exists, and one finding in it is fixed.
  `foundation_auth` `TokenSigner::verify` compares tags in constant time with
  `Mac::verify_slice`, and the signer has a key ring with key ids (commits `25cab64fe` and
  `540611b30`). Cite the commits, not the deleted file.
