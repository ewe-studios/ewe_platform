---
description: "Create an HTTP/1.1 client reusing the existing simple_http module structures, using iterator-based patterns with valtron executors and pluggable TLS/DNS/resolution components."
status: "in-progress"
priority: "high"
created: 2026-01-18
author: "Main Agent"
metadata:
  version: "5.4"
  last_updated: 2026-03-02
  estimated_effort: "medium"
  tags:
    - http-client
    - networking
    - rust
    - iterator-patterns
    - valtron-executors
  skills: []
  tools:
    - Rust
    - cargo
has_features: true
has_fundamentals: true
builds_on: "specifications/04-condvar-primitives"
related_specs:
  - "specifications/03-wasm-friendly-sync-primitives"
features:
  completed: 11
  uncompleted: 3
  total: 14
  completion_percentage: 79
---

# Overview

This specification defines the implementation of a robust, idiomatic HTTP/1.1 client for the ewe_platform project. The client reuses existing `simple_http` module structures, employs iterator-based patterns with valtron executors, and provides pluggable components for TLS, DNS resolution, and connection management.

## Goals

- Implement HTTP/1.1 client reusing `simple_http` module patterns
- Use iterator-based streaming patterns (TaskIterator / ExecutionAction)
- Integrate with `valtron` executors (single/multi-threaded via feature flag)
- Provide pluggable DNS resolver trait with default implementations
- Support connection pooling, redirects, proxies, and compression
- Leverage existing TLS functionality from `netcap` and foundation core

## Implementation Location

- Primary implementation: `backends/foundation_core/src/wire/simple_http/client/`
- Feature specifications: `specifications/02-build-http-client/features/*/feature.md`
- Documentation: `documentation/simple_http/doc.md`, `documentation/valtron/doc.md`, `documentation/netcap/doc.md`

## Known Issues

None currently identified.

## Feature Index

The HTTP client implementation is divided into features with clear dependencies. Each feature contains detailed requirements, tasks, and verification steps in its respective `feature.md` file.

**Implementation Guidelines:**
- Implement features in dependency order
- Each feature contains complete requirements and tasks
- Refer to individual feature.md files for detailed specifications

| #  | Feature | Description | Dependencies | Status |
|----|---------|-------------|--------------|--------|
| 0  | [valtron-utilities](./features/valtron-utilities/feature.md) | Reusable ExecutionAction types, unified executor, and state machine helpers | None | ✅ Complete |
| 1  | [tls-verification](./features/tls-verification/feature.md) | Verify and fix TLS backends (rustls, openssl, native-tls) | 0 | ✅ Complete |
| 2  | [foundation](./features/foundation/feature.md) | Error types, DNS resolution, and common foundations | 1 | ✅ Complete |
| 3  | [compression](./features/compression/feature.md) | gzip, deflate, brotli support and streaming integration | 2 | ✅ Complete |
| 4  | [connection](./features/connection/feature.md) | URL parsing, TCP, TLS handshakes (HTTP/HTTPS connection layer) | 2 | ✅ Complete |
| 5  | [proxy-support](./features/proxy-support/feature.md) | HTTP/HTTPS/SOCKS5 proxy handling and configuration | 4 | ⬜ Pending |
| 6  | [request-response](./features/request-response/feature.md) | Request builder, response types, headers and body handling | 4 | ✅ Complete |
| 7  | [auth-helpers](./features/auth-helpers/feature.md) | Basic, Bearer, Digest auth helpers and flows | 6 | ✅ Complete |
| 8  | [task-iterator](./features/task-iterator/feature.md) | TaskIterator, ExecutionAction types and executor integration | 0, 6 | ✅ Complete |
| 9  | [public-api](./features/public-api/feature.md) | User-facing API (SimpleHttpClient), ergonomics and integration | 8 | ✅ Complete |
| 10 | [connection-pooling](./features/connection-pooling/feature.md) | Connection pool design, checkout/checkin, cleanup and metrics | 4 | ⬜ Pending |
| 11 | [cookie-jar](./features/cookie-jar/feature.md) | Automatic cookie storage and policy handling | 9 | ✅ Complete |
| 12 | [middleware](./features/middleware/feature.md) | Request/response interceptors and middleware pipeline | 9 | ✅ Complete |
| 13 | [websocket](./features/websocket/feature.md) | WebSocket client and server | 4, 9 | ⬜ Pending |

Status Key: ⬜ Pending | 🔄 In Progress | ✅ Complete

## Requirements Conversation Summary

This specification was created through collaborative requirements gathering with the user, focusing on:
- Reusing existing `simple_http` module structures
- Iterator-based patterns for streaming and state management
- Pluggable architecture for DNS, TLS, and proxy components
- Feature flag for single vs. multi-threaded execution
- Comprehensive testing and verification strategy

## High-Level Architecture

The HTTP client follows a layered architecture:

1. **Foundation Layer**: Error handling, DNS resolution
2. **Transport Layer**: TCP connections, TLS handshakes
3. **Protocol Layer**: HTTP/1.1 request/response handling
4. **Client Layer**: Public API, connection pooling, middleware
5. **Extensions**: WebSocket, compression, authentication helpers

Each layer is implemented as a separate feature with clear dependencies.

# Success Criteria (Spec-Wide)

This specification is considered complete when:

## Functionality
- All 14 features completed and verified (see Feature Index)
- HTTP/1.1 requests work correctly (GET, POST, PUT, DELETE, etc.)
- TLS connections, connection pooling, redirects, and compression work together
- Proxy support functional (HTTP/HTTPS/SOCKS where configured)

## Code Quality
- Zero warnings from `cargo clippy -- -D warnings` for impacted crates
- `cargo fmt -- --check` passes for all modified files
- All unit and integration tests pass
- End-to-end integration tests demonstrate full feature interoperability

## Documentation
- Module documentation updated (`documentation/simple_http/doc.md`, etc.)
- `LEARNINGS.md` captures design decisions and trade-offs
- `VERIFICATION.md` produced with all verification checks passing
- `REPORT.md` created documenting final implementation
- `fundamentals/00-overview.md` created for public API overview

## Module References

Agents implementing features should read these documentation files:
- `documentation/simple_http/doc.md` - Simple HTTP module patterns
- `documentation/valtron/doc.md` - Valtron executor patterns
- `documentation/netcap/doc.md` - TLS and network capture patterns
- `.agents/stacks/rust.md` - Rust conventions and crate usage

---

_Created: 2026-01-18_
_Last Updated: 2026-02-28_
_Structure: Feature-based (has_features: true)_
