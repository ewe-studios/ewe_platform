# cf-rauthy — Learnings Review

**Source**: `/home/darkvoid/Boxxed/@formulas/src.rust/src.gedweb/cf-rauthy/`
**Reviewed**: 2026-06-07

## Project Overview

This is not a codebase — it's an **idea/strategy document** for integrating [Rauthy](https://github.com/sebadob/rauthy) (self-hosted OIDC provider) with Cloudflare Workers. Rauthy provides the admin GUI, user GUI, server, and replication with OIDC and enterprise protocols. The goal is a CF Workers client that makes it easy for CF apps to use Rauthy.

**Design constraint**: ALL ideas must support both NATIVE and Cloudflare — developers can always fully self-host.

## Proposed Integration Points

### 1. OIDC Relay Client
- Rauthy issue tracking this: https://github.com/sebadob/rauthy/issues/1582
- A lightweight CF Workers client that relays OIDC flows to a self-hosted Rauthy server

### 2. Email Integration
- Rauthy sends emails; could add Cloudflare Email Service support
- Reference: `workers-rs-util` repo has `nu_plugin_email` + `cf_email_worker` (host + wasm, email send/receive via CF Email Service)

### 3. SQLite Replication via Cloudflare
- Rauthy uses Hiqlite (SQLite with Raft) for database sync between servers
- Proposal: use `connyay/EdgeReplica` (has ConnectRPC for Admin) to allow CF for SQLite sync and backups
- Requires discussion with Rauthy maintainers

### 4. Rauthy on Cloudflare (Long-term)
- Adapt Rauthy server to run on Cloudflare Workers
- SQLite replication via CF's inherent COLO sync
- Another future discussion item

### 5. Cedar ReBAC Integration
- Rauthy + ReBAC using Cedar Policy engine
- Cedar runs fine on Cloudflare (wasm-native, microsecond decisions)
- Rauthy client has required primitives
- Clean, extensible security system with minimal resource usage

### 6. ConnectRPC Middleware
- High-quality ConnectRPC middleware for Native and Cloudflare
- Reference: `cf-connectrpc-middleware` workspace
- ConnectRPC on CF allows zero-hop worker-to-worker communication
- Cross-language client support (TypeScript, Rust, etc.)

## Borrowable Patterns

### 1. Dual-Native + CF Support Constraint
"All ideas MUST support NATIVE and Cloudflare, so developers can always fully self-host." This is the right design principle — never lock users into a single platform. For ewe_platform: all our auth services should support both native Rust execution and wasm32 CF Workers.

### 2. OIDC Relay Pattern
Instead of running a full IdP in the Worker, relay OIDC flows to a self-hosted provider. The Worker handles session management, token storage, and CF-specific concerns while the heavy IdP runs on traditional infrastructure. For ewe_platform: our foundation_auth could follow this pattern — client-side hardening in the Worker, server-side IdP on traditional infrastructure.

### 3. Cedar ReBAC for Multi-Tenant Auth
Cedar policy engine for ReBAC (Relationship-Based Access Control) — rules like "an owner of the billing account can act on its orgs" expressed as ~5-line policies. Runs wasm-native in Workers. For ewe_platform: this is exactly what our Feature 14 (Cedar Policy Engine) in spec 38 covers.

### 4. Hiqlite / SQLite Replication
SQLite with Raft consensus for database sync. Interesting alternative to our current Turso/libsql/D1 approach for cases where SQLite replication is sufficient.

## Applicability to ewe_platform

**Directly useful:**
- The dual-native + CF constraint should be our design principle for all auth services
- The OIDC relay pattern matches our foundation_auth direction
- Cedar integration is our Feature 14 — this confirms the approach

**Conceptually useful:**
- The idea of adapting a full IdP to run on CF Workers — long-term goal for our spec 38 IdP server
- The SQLite replication approach could complement our foundation_db backends

## Status Assessment

This is a **strategic planning document**, not a codebase. It outlines a vision for integrating Rauthy with CF Workers across 6 integration points. No code has been written — all items require discussion with Rauthy maintainers and further design work.

The ideas here align well with spec 38 (foundation-auth-server) in ewe_platform:
- Feature 08 (Auth Manager) maps to the OIDC relay concept
- Feature 09 (IdP Server) maps to the Rauthy-on-CF concept
- Feature 14 (Cedar Policy Engine) maps to the Cedar ReBAC concept
- Features 01-07 (client hardening) map to the CF Workers client concept

## Recommendations

1. Track Rauthy issue #1582 for OIDC relay progress
2. Evaluate Hiqlite as a potential foundation_db backend alternative
3. The Cedar integration path is already covered by our spec 38 Feature 14
4. The dual-native + CF constraint should be added to our requirements.md as a design principle

---
*This is a strategy document, not a codebase. Cross-references: cf-connectrpc-middleware (middleware patterns), cf-do-locator (DO placement), cf-colo-hint (colo mapping).*
