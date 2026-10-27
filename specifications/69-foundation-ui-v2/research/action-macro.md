# Idea 8: the `#[action]` macro that returns UI updates

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); Document Object Model (DOM);
hash-based message authentication code (HMAC); Remote Procedure Call (RPC); ConnectRPC (a
Remote Procedure Call protocol over HTTP); Hypertext Transfer Protocol (HTTP); WebAssembly
(WASM); AeroSpace and Defence Industries Association of Europe (ASD), the owner of the STE
standard ASD-STE100.

## 1. What the document proposes

Source: `foundation_ui_rearchitecture.md` §5, lines 174-236.

- `#[foundation_ui::action]` on a handler function. The macro parses the network request and
  routes it by the function signature.
- Signature: `fn(ctx: &Context, event: Interaction<T>) -> Response`. `Interaction<T>` holds
  `node_id`, `event_name`, `input_value` and the decoded state `T`.
- `Response` is `UIUpdate(Vec<DomOp>)` (sent as columnar or mobile layout) or
  `DataUpdate(Vec<u8>)` (raw Arrow, MessagePack or custom bytes).
- Inside the handler, `html! { ctx, Some(&next_state), … }` renders the next view. It encodes
  `next_state`, signs it, and adds `data-state` and `data-state-token` to the root element.
- An event attribute such as `primal:onclick="increment_counter"` names the action by string.

Small errors in the sample: `std::uuid::Uuid` does not exist (the `uuid` crate does), and the
handler is a sync `fn`.

## 2. What we have now

| Item | Location |
|---|---|
| `html!` pure form returns an `Html` tree; reactive form takes `ctx, receiver` and queues `DomOp`s plus one effect per dynamic slot | `backends/foundation_macros/src/html_macro/mod.rs:6-29` |
| `primal:onX={handler}` binds to a **local** Rust handler or a `SignalSetter` (`primal:setter` with a callback id) | `backends/foundation_macros/src/html_macro/mod.rs:22-24`, `codegen.rs:520`, `parser.rs:562` |
| Event path into WASM: JS calls `invoke_signal_callback(callback_id, memory_id)` with JSON `EventData` | `backends/foundation_wasm_ui/src/events.rs:1-17`, `:65` |
| No server action macro, no action registry, no action id on the wire | — |
| Code-first ConnectRPC `#[service]` macro: procedure constants, registration functions, typed clients, an unimplemented handler | `backends/foundation_macros/src/connectrpc_service.rs:1-19` |
| Async-only rule: native `async fn` all the way down | `CLAUDE.md` rule 6 |

## 3. Gap

1. **No server actions.** Today an event changes a local signal and effects re-render. There
   is no "send this action to the server and apply what comes back" path.
2. **No action identity.** The document uses a string (`"increment_counter"`). A string
   costs bytes and allows typos. Spec 39 decision 011 ("no string keys") removed string
   keys from signals for the same reason. The macro can generate a stable numeric id per
   action, plus the string for debug.
3. **`html!` does not take state.** The document's `html! { ctx, Some(&state), … }` form does
   not exist. The macro does not sign anything.
4. **Response is untyped.** `DataUpdate(Vec<u8>)` loses the type. Rule 5 says to use
   generated types.
5. **Sync handler.** Real actions read a database or call a service. Rule 6 needs
   `async fn`.

## 4. Open risks

1. **Macro size.** One macro that parses requests, checks tags, decodes state, renders,
   signs and encodes does too much. It is hard to test and hard to read in errors. Split
   it: the macro generates a typed registration and a thin adapter. Plain library code does
   the check, decode and encode.
2. **ConnectRPC overlap.** We already have a code-first RPC macro with typed clients. An
   action is an RPC with a fixed request shape (state token plus input) and a fixed response
   shape (layout ops plus new state). It may be simpler to make actions ConnectRPC
   procedures than to build a second router. This is question Q14.
3. **Coupling state to render.** If `html!` signs state, every render needs the key. That
   pulls a secret into view code. Better: the action returns `(next_state, view)` and the
   framework signs once at the boundary.
4. **Native first.** The response is native layout ops first (D4), not `DomOp`. The
   response type must name the target vocabulary, or the framework must pick it per client.

## 5. Recommendation: **adapt**

- **Adopt** an attribute macro for server actions, built the same way as the ConnectRPC
  `#[service]` macro (typed registration, generated ids, generated client stubs).
- **Shape:** `async fn name(ctx: &ActionCtx, state: S, input: I) -> Result<ActionResult<S>, ActionError>`,
  where `S` and `I` are generated types. The framework checks the token and decodes `S`
  before the call. It authorizes (hook on `ActionCtx`). It signs the returned state after the
  call.
- **`ActionResult<S>`** carries the next state and a typed view update (native layout ops,
  later web ops) and optional typed data frames. No `Vec<u8>`.
- **Action ids** are numeric, generated, and stable. The on-device registry (see
  [`app-store-compliance.md`](app-store-compliance.md)) uses the same ids.
- **Keep `html!` free of secrets.** Signing happens at the dispatch boundary, not in the
  template macro.

See Q14 and Q15 in [`questions.md`](questions.md).
