---
feature: "Code-first Mode 3: #[connectrpc::service] + cross-crate generate! (D10)"
description: "Trait-driven service definition (json/arrow families) + descriptor-macro cross-crate generation"
status: "pending"
priority: "medium"
phase: 1
depends_on: ["26-codegen-proto"]
estimated_effort: "medium"
created: 2026-07-03
---
# Feature 27-codegen-code-first: Code-first Mode 3: #[connectrpc::service] + cross-crate generate! (D10)

## Description

Proto is not the only source of truth: define services as Rust traits and generate the same artifacts, including cross-crate via descriptor-macro token shipping — no dummy host items.

## Normative sources (single source of truth — read before writing code)

- decisions/10-codegen.md — §Mode 3 + §Cross-crate generation (normative, incl. conformance-class caveats)

## Scope

- #[connectrpc::service(package, codecs(...))] attribute macro on plain Rust traits; four D04 shapes recognized syntactically
- Codec tables from declared families (serde json / ToArrow arrow); proto stays proto-first
- Exported descriptor macro (token shipping) + function-like connectrpc::generate!(tokens => mod x { server, client }) at item position
- Extension-class documentation: code-first JSON uses wire name json, no schema-interop claim

## Acceptance criteria

- A trait defined in crate A generates server+client modules in crate B and round-trips end-to-end
- A type lacking every declared family fails with a clear compile error
