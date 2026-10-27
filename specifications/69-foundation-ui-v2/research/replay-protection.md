# Idea 7: tamper and replay protection for signed component state

_Written in ASD-STE100 Simplified Technical English (STE). Code and identifiers stay as they
are._

**Abbreviations in this file.** hash-based message authentication code (HMAC); Secure Hash
Algorithm 256 (SHA-256); cross-site request forgery (CSRF); remote code execution (RCE);
Common Vulnerabilities and Exposures (CVE); Common Vulnerability Scoring System (CVSS);
Cybersecurity and Infrastructure Security Agency (CISA); Known Exploited Vulnerabilities
(KEV); Internet Protocol (IP); JSON Web Token (JWT); JWT ID claim (jti); key identifier (kid);
time to live (TTL); authenticated encryption with associated data (AEAD); WebAssembly (WASM);
user interface (UI); AeroSpace and Defence Industries Association of Europe (ASD), the owner
of the STE standard ASD-STE100.

## 1. The question

Owner decision D6: "Look at industry standard, at Livewire and learn and refine what works
best for us."

The rearchitecture document signs state with HMAC-SHA256 and a rotating secret. It says
nothing about replay. A tag proves that the server made the bytes and that nobody changed
them. A tag does **not** prove that the bytes are current, that they belong to this user,
or that they belong to this action.

## 2. Study

### 2.1 Laravel Livewire (version 3)

Source: `livewire/livewire` on GitHub, `src/Mechanisms/HandleComponents/Checksum.php`,
`src/Mechanisms/HandleRequests/HandleRequests.php`,
`src/Features/SupportLockedProperties/BaseLocked.php`, and `docs/properties.md` (read
2026-10-27).

- **Model.** Stateless server. Each component sends a **snapshot** to the browser:
  `data` (public properties), `memo` (component id, name, path, method, children and more)
  and `checksum`. The browser returns the snapshot with each request, plus a list of
  property `updates` and method `calls`.
- **Tag.** `hash_hmac('sha256', json_encode($snapshot))`. The key is the application
  encryption key (`APP_KEY`). The tag covers the whole snapshot except `checksum` and
  `memo.children` (left out so the browser can change children during DOM morphing).
- **Check.** `hash_equals` (constant time). On a mismatch: fire `checksum.fail`, record the
  failure, throw `CorruptComponentPayloadException`.
- **Failure rate limit.** Ten failures per client IP in 600 seconds, then HTTP 429.
- **Replay.** None. No nonce, no expiry, no session or user binding. A valid snapshot stays
  valid for ever (until `APP_KEY` changes) and for any user.
- **CSRF.** The update route always gets Laravel's `web` middleware (session plus CSRF
  token) and a header guard (`RequireLivewireHeaders`). The code says: "Without it, CSRF
  protection is lost entirely on the update endpoint."
- **Locked properties.** `#[Locked]` makes the server reject a client `update` to that
  property (`CannotUpdateLockedPropertyException`). A typed Eloquent model property is locked
  by default.
- **Guidance.** The docs say "always treat public properties as user input" and show an
  attack where a user edits a hidden `id` to change another user's post. The fix is
  authorization in the action, or `#[Locked]`.
- **Incidents.** CVE-2025-54068 (CVSS 3.1 score 9.8, in the CISA KEV catalogue): unauthenticated
  RCE in `hydrateForUpdate`, through property updates on hydration. Fixed in 3.6.4. Separate
  research showed that a leaked `APP_KEY` lets an attacker forge snapshots and reach
  unsafe unmarshalling. Both are "the server decoded types the client chose" problems.

### 2.2 Phoenix LiveView

Source: `phoenixframework/phoenix_live_view`, `lib/phoenix_live_view/static.ex` and
`guides/server/security-model.md` (read 2026-10-27).

- **Model.** Stateful server. The state (assigns) lives in a server process. It never goes
  to the client.
- **Tokens.** The HTML carries `data-phx-session` and `data-phx-static`. Both are
  `Phoenix.Token.sign(endpoint, salt, {@token_vsn, data})` (`static.ex:405-406`). They hold
  the view module, ids, and the part of the session the view needs. Not the view state.
- **Version in the token.** `@token_vsn 6` (`static.ex:12`). The code comments say: if the
  signed term changes, bump the version. Old tokens then fail.
- **Expiry.** `Phoenix.Token.verify(..., max_age: @max_session_age)` with
  `@max_session_age 1_209_600` (14 days) (`static.ex:18`, `:31-33`).
- **Purpose separation.** A salt per endpoint, so a token for one purpose does not verify for
  another.
- **Reconnect.** The client reconnects and the server runs `mount/3` again. If the user lost
  access, mount fails and the user is redirected. A `live_socket_id` lets the server
  disconnect every socket of a user (for example at logout).
- **Guidance.** Authorize in `mount/3`, `handle_params/3` and every event handler. "An
  attacker can use browser developer tools or custom scripts to send any payload to your
  LiveView."

### 2.3 ASP.NET Web Forms ViewState

- **Model.** Stateless server. Page state goes to the client in a hidden `__VIEWSTATE` field.
- **Tag.** A MAC with the machine key. Encryption is optional.
- **User binding.** `ViewStateUserKey` mixes a per-user or per-session value into the MAC,
  so a ViewState from one user does not verify for another. This was added to stop
  "one-click" (CSRF-style) attacks.
- **Incidents.** Leaked or copied machine keys let attackers forge ViewState and reach .NET
  deserialization gadgets (RCE). Microsoft warned in 2025 about thousands of machine keys that
  were public in code samples and repositories.

### 2.4 Hotwire (Turbo), htmx

- **Model.** Stateless server, no client state. Each request carries ids in the URL or form.
  The server loads the truth from the database every time. CSRF token on every non-GET.
- **Lesson.** The safest state token holds ids and view flags, not domain data.

### 2.5 Token standards (JWT, PASETO)

- `kid` (key id) for rotation, `iat`/`exp` for time limits, `aud` for purpose, `jti` for a
  unique id.
- A `jti` stops replay only if the server keeps a list of used ids until expiry. **There is
  no fully stateless replay protection.** Every system above that stops replay keeps some
  state on the server (a process, a session, a used-id list) or accepts a time window.

## 3. Threats and which control stops each

| # | Threat | Livewire | LiveView | What we should do |
|---|---|---|---|---|
| T1 | User edits state | HMAC | State not sent | HMAC (or AEAD) over the exact wire bytes. Constant-time compare. |
| T2 | User sends a state from component A to an action of component B | Partly (memo has name and id) | Salt, view in token | Sign component type id, node id, and the action set id. Use a purpose label in the MAC input (domain separation). |
| T3 | User B replays a state captured from user A | No | Session in token | Bind a hash of the session id (anonymous sessions too). Reject on mismatch. |
| T4 | User replays their own old state (stale tab, double submit, back button) | No | State not sent | Expiry window (configurable). Domain-level version check (compare-and-swap on a row version) for writes. Optional single-use nonce for sensitive actions. |
| T5 | State grants rights that the server has since removed | Docs: authorize | Authorize every event | Never put authority (role, owner, price) in state. Authorize every action from server data. |
| T6 | Old token after a format change or a key change | No (until key change) | `@token_vsn` | Format version and `kid` in the token. Key ring with current and old keys. |
| T7 | Attacker brute-forces or probes tags | Rate limit by IP | — | Count check failures. Rate-limit by session and by IP. Limits are configuration (rule 8). |
| T8 | Server decodes attacker-chosen types | CVE-2025-54068 | Not applicable | Decode only after the tag passes, into one generated type per component. No polymorphic decode. No type names in the payload. |
| T9 | Key leak | `APP_KEY` leak → forgery | Same | Separate state key from other keys. Key in the keychain or secret store, not in config files. Rotation by `kid`. |
| T10 | CSRF on the web action endpoint | `web` middleware + header guard | Phoenix CSRF | Web: CSRF token or a required custom header plus `SameSite` cookies. Native: bearer auth, no cookies, so CSRF does not apply. |
| T11 | State readable by the user | Readable | Not sent | HMAC by default. AEAD (ChaCha20-Poly1305, already in `foundation_compact`) when the component marks state as private. |

## 4. What we take

1. **From Livewire:** the snapshot model (one signed state per component), constant-time
   compare, a failure rate limit, CSRF on the web endpoint, and "locked" as the default.
   In our design **all** signed state is locked: the client cannot send updates to it. Client
   input is a separate, unsigned field.
2. **From LiveView:** a token format version, an expiry, purpose separation, and
   "authorize every event". Also: a way to end all of a user's live connections.
3. **From ViewState:** bind to the user or session.
4. **From Hotwire:** keep state small. Ids and view flags, not domain data.
5. **From JWT practice:** `kid` for rotation; `jti` with a shared used-id store only where
   single use is needed.

**Refinement for us.** A layered default plus an opt-in:

- **Default (stateless, every action):** format version, `kid`, issued-at, expires-at,
  component type id, node id, action-set id, session-id hash, state bytes. HMAC-SHA256 over
  a purpose label plus these exact bytes. Constant-time compare. Typed decode. Authorize the
  action. Domain row versions catch stale writes.
- **Opt-in (single use):** an action marked single-use (for example payment, delete) also
  carries a random nonce. The server records the nonce in a shared store until expiry and
  rejects a second use. This adds state only for those actions.

## 5. Gap against the repository

- No state token, no key ring, no failure counter, no nonce store.
- The only HMAC code (`backends/foundation_auth/src/shared/session.rs:348-386`) has no `kid`
  and compares tags in non-constant time (`:384`). This is now in `docs/bug-backlog.md`.

## 6. Recommendation: **adopt** the layered model in §4

See Q11, Q12 and Q13 in [`questions.md`](questions.md).

## 7. Sources

- Livewire `Checksum.php`:
  https://github.com/livewire/livewire/blob/main/src/Mechanisms/HandleComponents/Checksum.php
- Livewire `HandleRequests.php`:
  https://github.com/livewire/livewire/blob/main/src/Mechanisms/HandleRequests/HandleRequests.php
- Livewire properties documentation:
  https://github.com/livewire/livewire/blob/main/docs/properties.md
- CVE-2025-54068 advisory: https://advisories.gitlab.com/pkg/composer/livewire/livewire/CVE-2025-54068/
- Phoenix LiveView `static.ex`:
  https://github.com/phoenixframework/phoenix_live_view/blob/main/lib/phoenix_live_view/static.ex
- Phoenix LiveView security model:
  https://github.com/phoenixframework/phoenix_live_view/blob/main/guides/server/security-model.md
