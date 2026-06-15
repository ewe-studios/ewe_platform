---
feature: "Access Control & Budget Surfacing — SessionAccessProvider + AllowAllAccess + budget retrieval"
description: "The agentic access-control trait SessionAccessProvider (domain methods can_use_tool/model/session/spend + generic authorize escape hatch; named to avoid the foundation_ai::AuthProvider credentials collision), authorization via foundation_cedar (Cedar policies, embedded-default/hosted-optional PolicyStore), identity via foundation_auth, a trivial AllowAllAccess impl, and token-budget surfaced to the model via F04's ledger"
status: "pending"
priority: "high"
depends_on: ["01-message-model", "04-token-accounting-budget"]
estimated_effort: "medium"
created: 2026-06-14
last_updated: 2026-06-14
author: "Main Agent"
tasks:
  completed: 0
  uncompleted: 10
  total: 10
  completion_percentage: 0%
---

# Feature 18: Access Control & Budget Surfacing

> **RESOLVED (user, 2026-06-15) — Item #4 / discussion §H2: authorize with `foundation_cedar`; don't
> hand-roll RBAC.** This SUPERSEDES the "minimal bespoke trait + bridge-only, RBAC deferred" framing in
> the review note below.
> - **Authn/identity:** `foundation_auth` (`AuthCredential`/JWT/OAuth) supplies the **principal**.
> - **Authz:** **`foundation_cedar`** (`cedar-policy 4.11`) — engine + request/response + `PolicyStore`.
>   Tool/model/session gating is expressed as **Cedar policies**, not bespoke if-chains.
> - **`SessionAccessProvider` trait (abstract, in `foundation_ai::agentic`):** ergonomic **domain
>   methods** — `can_use_tool(principal, name)`, `can_use_model(principal, id)`,
>   `can_access_session(principal, sid)`, `can_spend(principal, budget)` — **plus a generic
>   `authorize(principal, action, resource) -> Decision`** escape hatch for custom policies.
> - **Impls:** `CedarAccess` (real — evaluates Cedar against a `PolicyStore`: **embedded default**
>   `InMemory`/`File`, **hosted optional** `Sql`/`Kv` by config) and **`AllowAllAccess`** (trivial, no
>   Cedar dep — local/tests). The *trait* doesn't hard-depend on Cedar; `CedarAccess` does.
> - **wasm/CF:** Cedar is pure Rust → authz works on wasm; `KvPolicyStore` is the edge/hosted path.
> - **Budget surfacing (unchanged):** `token_budget(principal)` caps F04's `TokenLedger`; remaining
>   budget is noted to the model. (Still load-bearing.)
> - The naming stays **`SessionAccessProvider`** (avoids the `foundation_ai::AuthProvider` *credentials*
>   collision — still valid). `UserId`/principal comes from `foundation_auth`, not a bespoke `UserId(String)`.

> **Review status (2026-06-14) — self-review against code (subagent unavailable):**
> 1. **The naming collision is REAL and verified.** `foundation_ai::types::AuthProvider`
>    (`types/mod.rs:1457`) is `trait AuthProvider { fn auth(&self) -> Option<&AuthCredential>; }` — it
>    supplies **provider (Anthropic/OpenAI) CREDENTIALS** to a `ModelProvider` (it is `ModelProvider::
>    Config: AuthProvider`, :1462). Decision 12 reused the name `AuthProvider` for session ACL (Decision
>    12 line 132) — that would **shadow/clash**. So the agentic access trait MUST be named
>    **`SessionAccessProvider`** (requirements §6, F08 task). Different concern, different name.
> 2. **`foundation_auth` exists** (`backends/foundation_auth`) with `AuthCredential`/`AuthToken`
>    (OAuth/Jwt/Session/ApiKey, `shared/auth_token.rs:16`) — it is **credential/token** machinery, not a
>    session-ACL or budget store. So F18 defines its OWN `SessionAccessProvider` trait (the security
>    boundary Decision 12 §Rationale justifies) and *bridges* to `foundation_auth` for identity, rather
>    than consuming a ready-made ACL from it. (OD-18-2.)
> 3. **Decision 12's `UserId` collides with nothing but doesn't exist yet** — define `UserId(String)`
>    (or reuse a `foundation_auth` user id if present — none found in shared). `AuthError` is referenced
>    by F02's `AgenticError::Auth(#[from] AuthError)` (Decision 16 line 65) — F18 owns `AuthError`, and it
>    must be `Clone+PartialEq+Debug` so it rides the F03 error stream (F02). (OD-18-3.)
> 4. **The user's TODOs on Decision 12 reshape the scope.** Decision 12 line 9: "Locally we don't need
>    auth — add an always-allowed implementation." → `AllowAllAccess` (every check returns true; budget =
>    unlimited). Decision 12 line 12: "The Agent owns the session, unsure why access control matters" →
>    F18 keeps the trait MINIMAL (ownership + budget), and the heavy RBAC/tool-gating Decision 12 sketched
>    is **optional/server-side**, not required for local. So the core surface is: can this user use this
>    session, can they use this model, what's their token budget. (OD-18-1.)
> 5. **Budget surfacing is the load-bearing new behavior.** The provider retrieves the user's remaining
>    token budget and **surfaces the limit to the model** — concretely, it caps F04's `TokenLedger`
>    budget (`04-token-accounting-budget/feature.md:52` `TokenLedger` has a configurable max-token budget
>    that halts generation). So `SessionAccessProvider::token_budget(user)` → set as the ledger's budget
>    at session build; when the ledger halts (F04), the session reports the limit. The model also *sees*
>    the remaining budget (a system-prompt note) so it can be concise. (OD-18-4.)
> 6. **Tool-call gating is OPTIONAL** (Decision 12 §Tool Call Permission Gating). For local/AllowAll it's
>    a no-op. If present, F11's `execute_workflow` consults `SessionAccessProvider::can_use_tool` before a
>    stage and surfaces `AgenticError::ToolNotAuthorized` (Decision 16 line 33). Wire the hook; default
>    allow-all. (OD-18-5.)

> Implements Decision 12 (DB+auth integration), corrected. Owns the **`SessionAccessProvider`** trait
> (renamed to avoid the `foundation_ai::AuthProvider` credentials collision), an **`AllowAllAccess`**
> impl for local/no-auth, **token-budget retrieval surfaced to the model** (via F04's ledger), and a
> bridge to `foundation_auth`. Storage stays direct-on-`foundation_db` (Decision 12 §No Intermediate
> Layer) — F18 adds only the auth/budget boundary.

## WHY: Problem Statement

The platform serves multiple users, but locally there is no auth (Decision 12 user TODO). The agent
needs one clean boundary that (a) defaults to allow-everything locally, (b) can gate session/model
access and retrieve a per-user token budget in a hosted setting, and (c) surfaces that budget to the
model so it stays within limits. It must NOT collide with the existing `foundation_ai::AuthProvider`
(which feeds provider credentials, an entirely different thing).

## WHAT: Solution

```rust
// backends/foundation_ai/src/agentic/access.rs
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UserId(pub String);

#[derive(Debug, Clone, PartialEq)]      // rides AgenticError (F02) — Clone+PartialEq+Debug
pub enum AuthError {
    Unauthenticated,
    SessionForbidden { session: SessionId, user: UserId },
    ModelForbidden   { model: ModelId, user: UserId },
    BudgetExhausted  { user: UserId, limit: u64 },
    Backend(String),
}

pub struct TokenBudget { pub limit: Option<u64>, pub used: u64 }  // None = unlimited

/// The agentic access boundary — DISTINCT from foundation_ai::AuthProvider (provider credentials).
pub trait SessionAccessProvider: Send + Sync {
    /// May this user open/continue this session? (Owner / shared_with — Decision 12.)
    fn can_access_session(&self, user: &UserId, session: &SessionId) -> Result<bool, AuthError>;
    /// May this user use this model?
    fn can_use_model(&self, user: &UserId, model: &ModelId) -> Result<bool, AuthError>;
    /// May this user run this tool? (Optional gating; default true.)
    fn can_use_tool(&self, _user: &UserId, _tool: &str) -> Result<bool, AuthError> { Ok(true) }
    /// The user's remaining token budget — surfaced to the model + caps F04's ledger.
    fn token_budget(&self, user: &UserId) -> Result<TokenBudget, AuthError>;
    /// Record consumption (billing/tracking) — default no-op.
    fn record_usage(&self, _user: &UserId, _tokens: u64) -> Result<(), AuthError> { Ok(()) }
}

/// Local / no-auth: everything allowed, unlimited budget.
pub struct AllowAllAccess;
impl SessionAccessProvider for AllowAllAccess {
    fn can_access_session(&self, _u: &UserId, _s: &SessionId) -> Result<bool, AuthError> { Ok(true) }
    fn can_use_model(&self, _u: &UserId, _m: &ModelId) -> Result<bool, AuthError> { Ok(true) }
    fn token_budget(&self, _u: &UserId) -> Result<TokenBudget, AuthError> {
        Ok(TokenBudget { limit: None, used: 0 })   // unlimited
    }
}
```

### Budget surfacing (the load-bearing behavior, OD-18-4)

```text
session build (F20 preflight):
    budget = access.token_budget(user)
    if let Some(limit) = budget.limit: ledger.set_budget(limit - budget.used)   // F04 caps generation
    context note: "You have ~{remaining} tokens of budget for this session."     // model sees it
runtime:
    F04 ledger halts generation when budget hit -> AgenticError / AuthError::BudgetExhausted surfaced
    on turn end: access.record_usage(user, turn_tokens)
```

### foundation_auth bridge (OD-18-2)

`foundation_auth` provides identity (`AuthToken`/`AuthCredential`). A `FoundationAuthAccess` adapter
maps an authenticated `AuthToken` → `UserId` and looks up ACL/budget from a `foundation_db` store
(Decision 12 keys `session:{id}:metadata` with `owner_id`/`shared_with`). This adapter is the hosted
path; `AllowAllAccess` is the local path.

## Architecture

```mermaid
graph TD
    SB[Session build F20 preflight] -->|can_access_session + can_use_model| AC[SessionAccessProvider]
    AC -->|AllowAll local| YES[allow + unlimited]
    AC -->|FoundationAuthAccess hosted| FA[foundation_auth identity + foundation_db ACL]
    SB -->|token_budget| BUD[TokenBudget]
    BUD -->|set_budget| LED[TokenLedger F04 caps generation]
    LED -->|halt at limit| ERR[AuthError::BudgetExhausted -> stream]
    LED -.remaining.-> NOTE[system-prompt budget note -> model]
    F11[Tool exec] -->|optional can_use_tool| AC
```

## Fundamentals Documentation (zero-to-expert) — REQUIRED

Author `fundamentals/` covering: authn vs authz vs credentials (and why naming a session-ACL trait
`AuthProvider` collides with a provider-credentials trait — the rename rationale); the allow-all local
pattern (zero-friction dev, opt-in hosting); token budgets & cost governance (retrieving a per-user
limit, capping generation via the ledger, surfacing the remaining budget to the model so it self-limits);
bridging an external auth crate (identity → UserId → ACL/budget lookup) without coupling the agent to
it; making auth errors `Clone+PartialEq` for the error stream; the security-boundary exception to "no
intermediate traits" (Decision 12). (Task — see list.)

## HOW: Implementation Steps

1. `UserId`, `AuthError` (`Clone+PartialEq+Debug`), `TokenBudget`.
2. `SessionAccessProvider` trait (session/model/tool checks + budget + usage; defaults for optional).
3. `AllowAllAccess` (allow-all, unlimited) — the local default.
4. Budget surfacing: `token_budget` → F04 `ledger.set_budget`; system-prompt remaining-budget note.
5. `FoundationAuthAccess` bridge: `AuthToken` → `UserId`, ACL/budget from `foundation_db` metadata.
6. Optional tool gating hook for F11 (`can_use_tool`, default allow).
7. Tests: AllowAll allows everything + unlimited budget; budget caps the ledger (generation halts at
   limit → `BudgetExhausted`); session/model forbidden errors; remaining-budget note present; bridge
   maps token→user; `AuthError` is `Clone+PartialEq`; wasm build.

## Resolved Decisions (Item #4, user 2026-06-15)

- **OD-18-1 — surface shape: RESOLVED → domain methods + generic escape hatch.** `SessionAccessProvider`
  exposes `can_use_tool`/`can_use_model`/`can_access_session`/`can_spend` **plus**
  `authorize(principal, action, resource) -> Decision`. Not a "minimal session-only" trait — authz is
  real (Cedar), tool-gating included.
- **OD-18-2 — `foundation_cedar` + `foundation_auth`: RESOLVED → use them deliberately.** Authz via
  `foundation_cedar` (Cedar policies), identity via `foundation_auth`. The *abstract* trait doesn't
  hard-depend on Cedar; the `CedarAccess` impl does (`AllowAllAccess` needs neither). No "avoid the dep"
  bridge.
- **OD-18-3 — principal/attribution: RESOLVED → from `foundation_auth`; refine via Cedar.** No bespoke
  `UserId(String)`; the principal is the authenticated identity, and Cedar policies do attribute-based
  refinement. Policies are **local or hosted** via the `PolicyStore` backends (embedded default
  `InMemory`/`File`; hosted optional `Sql`/`Kv`).
- **OD-18-4 — budget surfacing: RESOLVED → both** (proactive system-prompt note **and** hard cap via
  F04's `TokenLedger`). `token_budget(principal)` sets the ledger budget at session build; on halt the
  session reports the limit; the model also sees the remaining budget so it can be concise.
- **OD-18-5 — tool gating placement: RESOLVED → F11, per-call.** F11's `execute_workflow` calls
  `can_use_tool(principal, name)` before a stage (default allow under `AllowAllAccess`); denial →
  `AgenticError::ToolNotAuthorized`. Per-call/per-user (not at F09 registration).
        

## Target Files

- `backends/foundation_ai/src/agentic/access.rs` (new) — `SessionAccessProvider`, `AllowAllAccess`,
  `UserId`, `AuthError`, `TokenBudget`, `FoundationAuthAccess` bridge
- coordinates F04 (`TokenLedger` budget cap), F01 (`SessionId`/`ModelId`), F11 (tool gating), F02
  (`AgenticError::Auth`/`ToolNotAuthorized`), F20 (preflight checks), `foundation_auth` (identity)

## Tests

```bash
cargo test -p foundation_ai -- agentic::access
cargo build -p foundation_ai --target wasm32-unknown-unknown
```

## Verification

```bash
cargo build -p foundation_ai
cargo build -p foundation_ai --target wasm32-unknown-unknown
cargo clippy -p foundation_ai -- -D warnings
cargo test  -p foundation_ai -- agentic::access
```

## Done When

- `SessionAccessProvider` (renamed — no clash with `foundation_ai::AuthProvider`) + `AllowAllAccess`
  (local default, unlimited); token budget retrieved and surfaced to the model (caps F04's ledger +
  system-prompt note); `foundation_auth` bridge for hosted identity/ACL; optional tool gating hook for
  F11; `AuthError` is `Clone+PartialEq+Debug`; builds native + wasm.
- OD-18-1..5 resolved (OD-18-4 flagged); fundamentals authored.
