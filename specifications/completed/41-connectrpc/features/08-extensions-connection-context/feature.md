---
feature: "Arc-valued Extensions + ConnectionContext on requests (D12 §13)"
description: "Cheap-Clone Extensions for the COW Ctx model; typed per-connection context on SimpleIncomingRequest"
status: "complete"
priority: "high"
phase: 1
depends_on: []
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 08-extensions-connection-context: Arc-valued Extensions + ConnectionContext on requests (D12 §13)

## Description

netio enablers for the owned-Clone Ctx model: Arc-valued Extensions (cheap-Clone map) and the typed ConnectionContext carried once per connection on SimpleIncomingRequest.

## Normative sources (single source of truth — read before writing code)

- decisions/12-foundation-enablement.md — §13 (normative)
- decisions/04-handler-and-interceptor-model.md — §Extensions travel pathway, Q13

## Scope

- Extensions values become Arc<dyn Any + Send + Sync>; Clone = refcount bumps; insert<T> wraps Arc::new (call sites unchanged)
- ConnectionContext defined in netcap: peer identity (Endpoint<I>), TLS peer certs, ALPN/HTTP version, 0-RTT, QUIC conn id
- SimpleIncomingRequest.connection: Arc<ConnectionContext> (empty default — additive); HTTP/1.1 front end populates at accept/handshake

## Acceptance criteria

- Extensions clone is shallow; existing insert/get call sites compile unchanged
- ConnectionContext reachable from a request in the HTTP/1.1 path; existing tests unchanged
