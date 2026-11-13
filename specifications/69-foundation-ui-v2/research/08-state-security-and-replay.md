# 08. State security and replay protection

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** user interface (UI); hash-based message authentication code
(HMAC); Secure Hash Algorithm 256 (SHA-256); authenticated encryption with associated data
(AEAD); WebAssembly (WASM); cross-site request forgery (CSRF); remote code execution (RCE);
key identifier (kid); number used once (nonce); identifier (id); AeroSpace and Defence
Industries Association of Europe (ASD), the owner of the STE standard ASD-STE100.

## 1. The idea, laid out

Source: `foundation_ui_rearchitecture.md` §4 ("The Security Lifecycle") and §2
(`security_hmac`).

**The server signs state with HMAC-SHA256 and a rotating master secret. Any replica checks the
tag before it trusts the state.** The document gives three steps: generate and sign at render,
harvest on interaction, verify on any replica.

The document is silent on four things that a real system needs: what the tag binds to, how long
it is valid, how keys rotate, and replay. Owner decision D6 asks us to learn from Livewire and the
industry. The full study is in [`08b-replay-protection-study.md`](./08b-replay-protection-study.md).

_North star fit: one security rule for the state envelope, whatever the platform (D35,
[13](./13-platform-capabilities.md))._

## 2. Who signs (owner decision D5)

"Client never signs, its always from the server or the wasm bundle running on the client on the
web – for on-device, this does not matter."

| Where the logic runs | Who signs | Who verifies | Trust |
|---|---|---|---|
| Remote server | Server key | Any server replica | Full: the user cannot forge tags |
| Browser WASM bundle (local logic) | The WASM runtime | The same WASM runtime only | Integrity against bugs and stale state, not against the user: a key in the browser can be read |
| On device (native, local logic) | Not needed | Not needed | No boundary: view and logic are in one signed app |

Rule: **a remote server never accepts a tag made by a client-side key.** (Q17 confirms this
reading.)

## 3. Current state

| Part | Where |
|---|---|
| HMAC-SHA256 signer for session tokens, private to `foundation_auth`; key ring with key ids and rotation | `backends/foundation_auth/src/shared/session.rs` (`TokenSigner`, `KeyRing`); commits `25cab64fe`, `540611b30` |
| Its tag compare is constant time (`Mac::verify_slice`) | same file |
| ChaCha20-Poly1305 | `backends/foundation_compact/src/crypto/chacha.rs:24`, `:50` |
| Key type that zeroes on drop | `backends/foundation_compact/src/crypto/key.rs:15` |
| No UI state token and no failure counter | — |
| No nonce store | By design: single use is a domain rule (D14) |

## 4. The new way

### 4.1 What the tag covers

```text
tag = HMAC-SHA256(key[key_id],
        "ewe.ui.state.v1" ||            // purpose label: domain separation
        format_version || key_id ||
        issued_at || expires_at ||
        component_type || root_node ||
        session_binding || body)
```

Each field stops one attack class:

| Field | Stops |
|---|---|
| Purpose label | A tag made for another purpose (a session token) being used as a state tag |
| `format_version` | Old tokens after the state type changes shape |
| `key_id` | Nothing on its own; it makes rotation possible |
| `expires_at` | Replay of old state after a configured time window |
| `component_type`, `root_node` | State of component A sent to an action of component B |
| `session_binding` | User B replaying user A's captured state |
| `body` | Edits to the state itself |

### 4.2 Verification order on any replica

```mermaid
flowchart TD
    R[ActionRequest arrives] --> L{Failure rate limit<br/>by session and address}
    L -->|over| X1[Reject 429]
    L --> K{key_id in key ring?}
    K -->|no| X2[Reject: unknown key]
    K --> M{Constant-time tag compare}
    M -->|bad| X3[Reject, count failure]
    M --> E{Not expired?}
    E -->|expired| X4[Reject: ask client to reload]
    E --> B{Session and component match?}
    B -->|no| X5[Reject]
    B --> D[Decode body into the one generated type]
    D --> A{Authorize action from server data}
    A -->|no| X6[Reject]
    A --> RUN[Run action<br/>domain rule rejects a second request]
```

_The diagram shows the checks in order. Cheap checks run first. Decode happens only after the
tag passes. Single use is a domain rule, not a store._

### 4.3 Keys

- A key ring: one current key and some previous keys, each with a `key_id`. Sign with current;
  verify with any key in the ring. Rotation: add a new key, make it current, remove the old key
  after `expires_at` of the last token signed with it has passed.
- The state key is separate from every other key (session, auth). A leak of one does not forge
  the other.
- Keys come from the platform secret source, never from a config file in the repository.

### 4.4 Replay: the layered model

There is no fully stateless replay protection. Every system that stops replay keeps some state
on the server, or accepts a time window. So the new way has layers:

1. **Default, stateless, every action:** expiry window, session binding, component binding,
   constant-time compare, typed decode, authorization on every action.
2. **Stale writes:** domain data carries a row version. A write is a compare-and-swap on it. Two
   clicks with the same old state: the second write fails cleanly.
3. **Single use is a domain rule** (payments, deletes). The server knows when it first creates a
   payment or accepts a delete. A second request for the same resource is rejected by the
   domain data, for example a unique constraint or a state machine on the row. No nonce store
   and no used-id list. The server owns this decision (D14).
4. **Web only:** a CSRF guard on the action endpoint (token or required custom header, plus
   `SameSite` cookies). Native uses bearer auth with no cookies, so CSRF does not apply.

The server may also ignore the returned state and load the authoritative state from the backend
(D13, [07](./07-element-riding-state.md) §3.5). A valid tag is not a claim of authority. The
domain data is.

### 4.5 What we take from the study

| From | We take |
|---|---|
| Livewire | One signed snapshot per component; constant-time compare; failure rate limit; CSRF on the endpoint; signed state is never updatable by the client |
| Phoenix LiveView | Token format version; expiry; purpose separation; authorize every event; a way to end all live connections of a user |
| ASP.NET ViewState | Bind the tag to the user or session |
| Hotwire | Keep state small: ids and flags, not domain data |
| JWT practice | `kid` for rotation; single use stays a domain rule, not a used-id list (D14) |
| Livewire CVE-2025-54068, ViewState RCE | Never decode attacker-chosen types; decode only after the tag passes, into one fixed type |

## 5. What this changes for us

- A new signer module with a purpose label, expiry, component and session binding, and an AEAD
  mode. `foundation_auth` session tokens move to it later. The key ring and constant-time
  compare already exist there.
- A failure counter behind a trait, with limits as configuration. Single use needs no new
  store: the domain data rejects a second request (D14).

## 6. Open questions

Q18 in [`questions.md`](./questions.md): the default expiry window. Q17 is answered: yes, a
browser-signed tag is checked only by the same browser runtime. Q19 is answered by D14: no
nonce store; the domain owns single use.
