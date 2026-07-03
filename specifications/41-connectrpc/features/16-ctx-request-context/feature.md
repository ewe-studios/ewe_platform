---
feature: "Ctx, RequestContext, CancelSignal, Extensions pathway (D04)"
description: "The owned-Clone per-call context: COW with_* API, CancelSignal semantics, the extensions travel pathway"
status: "pending"
priority: "high"
phase: 1
depends_on: ["08-extensions-connection-context", "12-error-model"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 16-ctx-request-context: Ctx, RequestContext, CancelSignal, Extensions pathway (D04)

## Description

The per-call context model: owned + cheap-Clone with Arc-backed internals, write = COW rebuild-and-move, read = share; CancelSignal as the only cross-clone mutable object with explicit-only linking; the zero-lock extensions travel pathway.

## Normative sources (single source of truth — read before writing code)

- decisions/04-handler-and-interceptor-model.md — §RequestContext, §Ctx, §Extensions pathway, Decided Details Q4 (all normative)
- decisions/07-client-architecture.md — §Client-side Ctx contract

## Scope

- RequestContext: owned + cheap Clone, Arc'd internals (Spec/Peer/headers), Copy deadline, Extensions, Arc<ConnectionContext>, CancelSignal
- CancelSignal: new()/linked(&parent)/cancel()/is_canceled()/cancelled().await — clone shares; detach/link explicit, one-way down
- Ctx { bag, request } + client()/background() constructors + with_deadline/with_extension/with_cancellation COW derivations + delegates
- Extensions hop: middleware &mut-inserts on SimpleIncomingRequest → dispatch take()-moves → COW downstream-only after construction

## Out of scope

- Auth info contract (feature 25)

## Acceptance criteria

- Two Ctx clones can never diverge on shared state (type-level); cancel fires across clones
- with_extension visibility is downstream-only (test: earlier clone doesn't see later insert)
- linked() propagates parent→child only; child cancel never reaches parent
