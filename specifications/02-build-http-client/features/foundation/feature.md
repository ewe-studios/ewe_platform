---
feature: foundation
description: Error types (HttpClientError, DnsError) and DNS resolution with caching support
status: completed
priority: high
depends_on:
  - tls-verification
estimated_effort: small
created: 2026-01-18
last_updated: 2026-01-25
author: Main Agent
context_optimization: true  # Sub-agents MUST generate COMPACT_CONTEXT.md before work, reload after updates
compact_context_file: ./COMPACT_CONTEXT.md  # Ultra-compact current task context (97% reduction)
context_reload_required: true  # Clear and reload from compact context regularly to prevent context limit errors
tasks:
  completed: 9
  uncompleted: 0
  total: 9
  completion_percentage: 100
files_required:
  implementation_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/13-implementation-agent-guide.md
      - .agents/rules/11-skills-usage.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
      - ./templates/
  verification_agent:
    rules:
      - .agents/rules/01-rule-naming-and-structure.md
      - .agents/rules/02-rules-directory-policy.md
      - .agents/rules/03-dangerous-operations-safety.md
      - .agents/rules/04-work-commit-and-push-rules.md
      - .agents/rules/08-verification-workflow-complete-guide.md
      - .agents/stacks/rust.md
    files:
      - ../requirements.md
      - ./feature.md
---

# Foundation Feature

## 🔍 CRITICAL: Retrieval-Led Reasoning Required

**ALL agents implementing this feature MUST use retrieval-led reasoning.**

### Before Starting Implementation

**YOU MUST** (in this order):
1. ✅ **Search the codebase** for similar implementations using Grep/Glob
2. ✅ **Read existing code** in related modules to understand patterns
3. ✅ **Check stack files** (`.agents/stacks/[language].md`) for language-specific conventions
4. ✅ **Read parent specification** (`../requirements.md`) for high-level context
5. ✅ **Read module documentation** for modules this feature touches
6. ✅ **Check dependencies** by reading other feature files referenced in `depends_on`
7. ✅ **Follow discovered patterns** consistently with existing codebase

### FORBIDDEN Approaches

**YOU MUST NOT**:
- ❌ Assume patterns based on typical practices without checking this codebase
- ❌ Implement without searching for similar features first
- ❌ Apply generic solutions without verifying project conventions
- ❌ Guess at naming conventions, file structures, or patterns
- ❌ Use pretraining knowledge without validating against actual project code

### Retrieval Checklist

Before implementing, answer these questions by reading code:
- [ ] What similar features exist in this project? (use Grep to find)
- [ ] What patterns do they follow? (read their implementations)
- [ ] What naming conventions are used? (observed from existing code)
- [ ] How are errors handled in similar code? (check error patterns)
- [ ] What testing patterns exist? (read existing test files)
- [ ] Are there existing helper functions I can reuse? (search thoroughly)

### Enforcement

- Show your retrieval steps in your work report
- Reference specific files/patterns you discovered
- Explain how your implementation matches existing patterns
- "I assumed..." responses will be rejected - only "I found in [file]..." accepted

---

## Overview

Create the foundational layer for the HTTP 1.1 client: error types and DNS resolution. This feature establishes the error handling patterns and pluggable DNS resolution system that all subsequent features will build upon.

## Dependencies

This feature depends on:
- `tls-verification` - Ensures TLS infrastructure works before building on it

This feature is required by:
- `connection` - Uses DnsResolver for hostname resolution
- `task-iterator` - Uses HttpClientError for error handling
- `public-api` - Exposes error types to users

## Requirements

### Error Handling Pattern (MANDATORY)

All error types **MUST** follow this pattern:

```rust
use derive_more::From;

#[derive(From, Debug)]
pub enum HttpClientError {
    #[from(ignore)]
    DnsResolutionFailed(String),

    #[from(ignore)]
    ConnectionFailed(BoxedError),

    // ... other variants with clear descriptions
}

impl std::error::Error for HttpClientError {}

impl core::fmt::Display for HttpClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DnsResolutionFailed(host) => {
                write!(f, "DNS resolution failed for host: {}", host)
            }
            // ... clear, descriptive messages for each variant
        }
    }
}
```

### DNS Resolution

1. **DnsResolver Trait**
   - Generic trait for pluggable DNS implementations
   - Method: `resolve(&self, host: &str) -> Result<Vec<SocketAddr>, DnsError>`

2. **SystemDnsResolver**
   - Uses `std::net::ToSocketAddrs`
   - Default implementation for production use

3. **CachingDnsResolver<R: DnsResolver>**
   - Wraps any DnsResolver with caching
   - TTL-based expiration support
   - Generic wrapper (not boxed)

4. **MockDnsResolver**
   - For testing purposes
   - Configurable responses

### Generic Type Pattern (MANDATORY)

Use generics instead of boxed types:

```rust
// Preferred
pub fn with_resolver<R: DnsResolver>(resolver: R) -> Self

// Avoid when possible
pub fn with_resolver(resolver: Box<dyn DnsResolver>) -> Self
```

## Implementation Details

### File Structure

```
client/
├── mod.rs       (module entry, re-exports)
├── errors.rs    (HttpClientError, DnsError)
└── dns.rs       (DnsResolver trait + implementations)
```

### DnsError

```rust
#[derive(From, Debug)]
pub enum DnsError {
    #[from(ignore)]
    ResolutionFailed(String),

    #[from(ignore)]
    InvalidHost(String),

    #[from(ignore)]
    NoAddressesFound(String),

    #[from]
    IoError(std::io::Error),
}
```

### HttpClientError (Initial Variants)

```rust
#[derive(From, Debug)]
pub enum HttpClientError {
    #[from]
    DnsError(DnsError),

    #[from(ignore)]
    ConnectionFailed(String),

    #[from(ignore)]
    InvalidUrl(String),

    // Additional variants will be added in later features
}
```

### DnsResolver Trait

```rust
pub trait DnsResolver {
    fn resolve(&self, host: &str, port: u16) -> Result<Vec<SocketAddr>, DnsError>;
}
```

### CachingDnsResolver

```rust
pub struct CachingDnsResolver<R: DnsResolver> {
    inner: R,
    cache: HashMap<String, CachedEntry>,
    ttl: Duration,
}

struct CachedEntry {
    addresses: Vec<SocketAddr>,
    expires_at: Instant,
}
```

## Success Criteria

- [x] `client/mod.rs` exists and compiles
- [x] `DnsError` implements `From`, `Debug`, `Display`, `std::error::Error`
- [x] `HttpClientError` implements `From`, `Debug`, `Display`, `std::error::Error`
- [x] `DnsResolver` trait is defined with generic support
- [x] `SystemDnsResolver` resolves hostnames correctly
- [x] `CachingDnsResolver<R>` caches results with TTL
- [x] `MockDnsResolver` works for testing
- [x] All unit tests pass (20/20 tests passing)
- [x] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- dns
cargo build --package foundation_core
```

## Notes for Agents

### Before Starting
- **MUST READ** parent specification's requirements.md
- **MUST READ** existing `simple_http/errors.rs` for error patterns
- **MUST READ** existing `simple_http/mod.rs` for module structure
- **MUST VERIFY** derive_more is available in dependencies

### Implementation Guidelines
- Use `derive_more::From` for error enums
- Implement `Debug` and `Display` with clear error messages
- Use generic type parameters for CachingDnsResolver
- Add `#[must_use]` on pure functions

---
*Created: 2026-01-18*
*Last Updated: 2026-01-18*
