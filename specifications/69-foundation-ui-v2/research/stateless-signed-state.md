# Idea 6: stateless servers with element-riding signed state

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); hash-based message authentication code
(HMAC); Secure Hash Algorithm 256 (SHA-256); authenticated encryption with associated data
(AEAD); WebAssembly (WASM); JavaScript (JS); Document Object Model (DOM); time to live (TTL);
key identifier (kid); Server-Sent Events (SSE); AeroSpace and Defence Industries Association
of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. What the document proposes

Source: `foundation_ui_rearchitecture.md` §4, lines 161-170, and the state tables at lines
96-127.

1. When the server renders a component, it encodes the view state to bytes. It computes an
   HMAC-SHA256 tag with a rotating master secret. It puts both on the element as
   `data-state` and `data-state-token`.
2. When the user taps or clicks, the client runtime reads these two values from the element.
   It puts them into a `UserInteractionRequest`.
3. Any server replica can receive the request. The replica checks the tag. If the tag is
   good, it rebuilds the state, runs the action, renders the result, and drops the memory.
   No session store, no sticky routing.

Owner decision D5: "Client never signs, its always from the server or the wasm bundle
running on the client on the web – for on-device, this does not matter."

## 2. What we have now

The current model is the opposite: a **stateful** server.

| Item | Location |
|---|---|
| Server `App` holds signals in memory on one thread and broadcasts frames to clients. "The App never leaves its thread." | `backends/foundation_wasm_ui/src/server/mod.rs:1-24`, `:71` (`Broadcaster`, with a replayed backlog for late joiners) |
| Decision: "The client never tells the server its state." | `specifications/completed/39-foundation-wasm-ui/decisions/025-server-client-state-sync.md:8` |
| Decision: "No signal values need to be synced or embedded." | same file, line 40 |
| Components put view flags on elements as plain `data-*` attributes (`data-checked`, `data-disabled`) for CSS. They are not signed and the server does not read them back. | `backends/foundation_ui_components/src/lib.rs:7-10` |
| HMAC-SHA256 signer for session tokens. Private to `foundation_auth`. Single key, no key id. | `backends/foundation_auth/src/shared/session.rs:348-386` |
| The same signer compares tags with a short-circuit iterator, not in constant time. | `backends/foundation_auth/src/shared/session.rs:384` (added to `docs/bug-backlog.md` in this change) |
| ChaCha20-Poly1305 AEAD encrypt and decrypt | `backends/foundation_compact/src/crypto/chacha.rs:24`, `:50` |
| Key type with zeroize on drop | `backends/foundation_compact/src/crypto/key.rs:15` |

## 3. What decision D5 means for the design

- The **user's client code never signs**. Not JS, not native view code.
- The **signer is whatever plays the server role**:
  - a remote server, or
  - on the web, the WASM bundle that runs the app logic in the browser (spec 39 execution
    modes: main thread, worker, service worker).
- **On device** (native app with local logic), signing "does not matter": there is no trust
  boundary between the view and the logic, because both are in one signed app binary.

Consequence that the design must state: a WASM bundle in the browser cannot keep a secret.
Any key it holds, the user can read. So a tag made by browser WASM proves nothing to a
**remote** server. Two rules follow:

1. State signed by browser WASM is checked only by that same WASM runtime (a local loop). It
   protects against bugs and stale state, not against a hostile user.
2. A remote server accepts only state that a server key signed. It never accepts a tag from
   a client key.

This is question Q8.

## 4. Gap

1. **No state token type.** Nothing signs, encodes or checks view state today.
2. **No reusable signer.** `TokenSigner` is private, has one key, and has no key id, so key
   rotation is not possible. It also has the timing bug above.
3. **No key ring.** The document says "rotating master secret". Rotation needs a key id in
   each token, a current key, and one or more old keys still accepted for checks.
4. **No binding.** The document's tag covers only the state bytes. It does not bind the
   state to a session, a user, a component type, an action, or a time. See
   [`replay-protection.md`](replay-protection.md) for why that is not enough.
5. **Stateful parts stay.** `<mount-stream>` and the `Broadcaster` hold a live connection
   per client. A stream is state on one replica. Stateless actions and stateful streams must
   live side by side.

## 5. Open risks

1. **Per element or per component.** The document puts state on each element. Livewire puts
   one snapshot on each component. Per element repeats bytes and tags. Per component root is
   smaller and matches how actions work (an action acts on a component). Recommendation: per
   component, with the node id inside the signed data.
2. **Size.** Every render and every request carries the state. Large state makes every
   frame large. A size cap on state is configuration with a documented default (rule 8).
   Large data stays on the server (database) and the state carries only ids.
3. **Secrecy.** HMAC proves integrity, not secrecy. The user can read the state. If state can
   hold anything private, use AEAD (`foundation_compact` already has ChaCha20-Poly1305).
   Livewire and ASP.NET ViewState both leave state readable by default and tell developers
   not to put secrets in it.
4. **Decoding untrusted bytes.** The server decodes state only after the tag passes. It must
   decode into one fixed generated type per component. It must never pick a type from the
   payload. Livewire CVE-2025-54068 and the ASP.NET ViewState attacks were both "decode
   attacker-chosen types" bugs. See [`replay-protection.md`](replay-protection.md).
5. **Canonical bytes.** Sign the exact bytes that go on the wire (the FlatBuffers state
   table). Do not re-serialize before the check. Re-serialization can change bytes and break
   the tag, or open a mismatch between what was signed and what was used.
6. **Concurrency.** Two tabs or two fast clicks can send two actions with the same old
   state. A stateless server cannot see this. The domain layer (database row version) must
   catch it where it matters.

## 6. Conflict with earlier decisions

Per owner decision D3, these give way:

- Decision 025 line 8 ("the client never tells the server its state"): the client now returns
  server-signed state with each action. The spirit stays: the client never **authors** state.
- Decision 025 line 40 ("no signal values … embedded"): state is now embedded.
- `server/mod.rs` single-thread stateful `App`: it stays for streams, but it is no longer the
  default for actions.

## 7. Recommendation: **adapt**

- **Adopt** server-signed state tokens so that any replica can handle an action.
- **One token per component root**, not per element. The node id goes inside the signed
  data.
- **Token fields** (signed): format version, key id, issued-at, expires-at, component type
  id, node id, session binding, state bytes. See [`replay-protection.md`](replay-protection.md)
  for the binding and replay fields.
- **Client input is separate.** `input_value` and event data are not signed. The server
  treats them as untrusted input, like a form post. Only state the server wrote is signed.
- **Build one signer crate or module** with a key ring (key id, current key, old keys),
  constant-time compare, and an AEAD option. `foundation_auth` session tokens can move to it
  later. Token TTL and size caps are configuration (rule 8).
- **Keep the stateful `App` + `Broadcaster` for streams.** Make "stateless action" and
  "stateful stream" two named modes.

See Q8, Q9 and Q10 in [`questions.md`](questions.md).
