---
feature: "Conformance harness + Phase-1 suites (Connect/gRPC-Web on h1)"
description: "Run the connectrpc conformance YAML suites against our server and client for the Phase-1 matrix"
status: "pending"
priority: "high"
phase: 1
depends_on: ["22-router-dispatch", "24-client-core", "26-codegen-proto"]
estimated_effort: "large"
created: 2026-07-03
---
# Feature 28-conformance-phase1: Conformance harness + Phase-1 suites (Connect/gRPC-Web on h1)

## Description

The proof: connect-go's 33+ YAML conformance suites running against our implementation for everything Phase 1 claims to support.

## Normative sources (single source of truth — read before writing code)

- plan.md — Reference Material (conformance suites location)
- decisions/05-protocol-wire-formats.md — wire shapes the suites exercise

## Scope

- Harness binary driving the conformance runner against foundation_connectrpc (server + client modes)
- Phase-1 matrix: Connect (unary POST/GET, streaming) + gRPC-Web (binary + text) over HTTP/1.1, codecs proto+json, compression, errors, timeouts, TLS
- Async tests via #[valtron_test] async fn (feature 03)

## Out of scope

- gRPC/HTTP-2 suites (feature 31)

## Acceptance criteria

- All applicable Phase-1 conformance suites green; failures triaged to spec text or code with the decision docs as arbiter
