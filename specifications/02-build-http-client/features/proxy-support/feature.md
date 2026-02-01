---
feature: proxy-support
description: HTTP/HTTPS/SOCKS5 proxy support with environment variable detection
status: pending
priority: medium
depends_on:
  - connection
estimated_effort: medium
created: 2026-01-19
last_updated: 2026-01-24
author: Main Agent
context_optimization: true  # Sub-agents MUST generate COMPACT_CONTEXT.md before work, reload after updates
compact_context_file: ./COMPACT_CONTEXT.md  # Ultra-compact current task context (97% reduction)
context_reload_required: true  # Clear and reload from compact context regularly to prevent context limit errors
tasks:
  completed: 0
  uncompleted: 13
  total: 13
  completion_percentage: 0
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

# Proxy Support Feature

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

## 🚀 CRITICAL: Token and Context Optimization

**ALL agents implementing this specification/feature MUST follow token and context optimization protocols.**

### Machine-Optimized Prompts (Rule 14)

**Main Agent MUST**:
1. Generate `machine_prompt.md` from this file when specification/feature finalized
2. Use pipe-delimited compression (58% token reduction)
3. Commit machine_prompt.md alongside human-readable file
4. Regenerate when human file updates
5. Provide machine_prompt.md path to sub-agents

**Sub-Agents MUST**:
- Read `machine_prompt.md` (NOT verbose human files)
- Parse DOCS_TO_READ section for files to load
- 58% token savings

### Context Compaction (Rule 15)

**Sub-Agents MUST** (before starting work):
1. Read machine_prompt.md and PROGRESS.md
2. Generate `COMPACT_CONTEXT.md`:
   - Embed machine_prompt.md content for current task
   - Extract current status from PROGRESS.md
   - List files for current task only (500-800 tokens)
3. CLEAR entire context
4. RELOAD from COMPACT_CONTEXT.md only
5. Proceed with 97% context reduction (180K→5K tokens)

**After PROGRESS.md Updates**:
- Regenerate COMPACT_CONTEXT.md (re-embed machine_prompt content)
- Clear and reload
- Maintain minimal context

**COMPACT_CONTEXT.md Lifecycle**:
- Generated fresh per task
- Contains ONLY current task (no history)
- Deleted when task completes
- Rewritten from scratch for next task

**See**:
- Rule 14: .agents/rules/14-machine-optimized-prompts.md
- Rule 15: .agents/rules/15-instruction-compaction.md

---

## Overview

Add comprehensive proxy support for the HTTP client, including HTTP CONNECT tunneling, HTTPS proxies, and SOCKS5 proxies. This feature enables transparent proxy usage with automatic environment variable detection and per-request overrides.

## Dependencies

This feature depends on:
- `connection` - Uses HttpClientConnection for TCP/TLS connections

This feature is required by:
- `public-api` - Exposes proxy configuration to users

## Requirements

### Proxy Types

Support three proxy protocols:

```rust
pub enum ProxyProtocol {
    /// HTTP proxy (CONNECT method for HTTPS targets)
    Http,

    /// HTTPS proxy (TLS to proxy, then CONNECT tunnel)
    Https,

    /// SOCKS5 proxy (feature-gated)
    Socks5,
}

pub struct ProxyConfig {
    pub protocol: ProxyProtocol,
    pub host: String,
    pub port: u16,
    pub auth: Option<ProxyAuth>,
}

pub struct ProxyAuth {
    pub username: String,
    pub password: String,
}
```

### HTTP Proxy (CONNECT Tunneling)

For HTTPS targets through HTTP proxy:

```rust
// 1. Connect to proxy via TCP
// 2. Send CONNECT request
// 3. Receive 200 Connection Established
// 4. Perform TLS handshake through tunnel
// 5. Send actual HTTP request

// CONNECT request format:
// CONNECT target.example.com:443 HTTP/1.1
// Host: target.example.com:443
// Proxy-Authorization: Basic dXNlcjpwYXNz  (if auth)
```

### HTTPS Proxy

TLS connection to proxy, then CONNECT tunnel:

```rust
// 1. Connect to proxy via TCP
// 2. Perform TLS handshake with proxy
// 3. Send CONNECT request over TLS
// 4. Receive 200 Connection Established
// 5. Perform TLS handshake for target through tunnel
// 6. Send actual HTTP request
```

### SOCKS5 Proxy (Feature-Gated)

SOCKS5 protocol support:

```rust
// Feature-gated: requires `socks5` feature
#[cfg(feature = "socks5")]
pub struct Socks5Connector {
    proxy: ProxyConfig,
}

// SOCKS5 handshake:
// 1. Send greeting with auth methods
// 2. Receive server's chosen method
// 3. Authenticate if required
// 4. Send connect request
// 5. Receive connection response
// 6. Proceed with TLS/HTTP
```

### Environment Variable Detection

Auto-detect proxies from environment:

```rust
impl ProxyConfig {
    /// Detect proxy from environment variables
    pub fn from_env(scheme: &Scheme) -> Option<Self> {
        match scheme {
            Scheme::Http => Self::from_env_var("HTTP_PROXY")
                .or_else(|| Self::from_env_var("http_proxy")),
            Scheme::Https => Self::from_env_var("HTTPS_PROXY")
                .or_else(|| Self::from_env_var("https_proxy")),
        }
    }

    /// Check if host should bypass proxy
    pub fn should_bypass(host: &str) -> bool {
        let no_proxy = std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .unwrap_or_default();

        for pattern in no_proxy.split(',') {
            let pattern = pattern.trim();
            if pattern == "*" {
                return true;
            }
            if host == pattern || host.ends_with(&format!(".{}", pattern)) {
                return true;
            }
        }
        false
    }

    fn from_env_var(var: &str) -> Option<Self> {
        std::env::var(var).ok().and_then(|url| Self::parse(&url).ok())
    }
}
```

### Configuration API

```rust
// Client-level proxy
let client = SimpleHttpClient::new()
    .proxy("http://proxy.example.com:8080")
    .proxy_auth("user", "password");

// Environment detection
let client = SimpleHttpClient::new()
    .proxy_from_env();

// Per-request override
let response = client.get(url)
    .proxy("http://other-proxy:8080")
    .send()?;

// Bypass proxy for specific request
let response = client.get(url)
    .no_proxy()
    .send()?;

// SOCKS5 (feature-gated)
let client = SimpleHttpClient::new()
    .proxy("socks5://proxy:1080");
```

### Error Handling

```rust
#[derive(From, Debug)]
pub enum HttpClientError {
    // ... existing variants ...

    #[from(ignore)]
    ProxyConnectionFailed(String),

    #[from(ignore)]
    ProxyAuthenticationFailed(String),

    #[from(ignore)]
    ProxyTunnelFailed { status: u16, message: String },

    #[from(ignore)]
    InvalidProxyUrl(String),

    #[from(ignore)]
    Socks5Error(String),
}
```

## Implementation Details

### File Structure

```
client/
├── proxy.rs    (NEW - Proxy configuration and connection)
└── ...
```

### Proxy URL Parsing

```rust
impl ProxyConfig {
    pub fn parse(url: &str) -> Result<Self, HttpClientError> {
        // Formats:
        // http://proxy:8080
        // http://user:pass@proxy:8080
        // https://proxy:8443
        // socks5://proxy:1080
        // socks5://user:pass@proxy:1080
    }
}
```

### HTTP CONNECT Implementation

```rust
pub struct ProxyTunnel {
    connection: Connection,
    config: ProxyConfig,
}

impl ProxyTunnel {
    pub fn connect_http_proxy(
        proxy: &ProxyConfig,
        target_host: &str,
        target_port: u16,
    ) -> Result<Connection, HttpClientError> {
        // 1. TCP connect to proxy
        let mut conn = TcpStream::connect((proxy.host.as_str(), proxy.port))?;

        // 2. Send CONNECT request
        let connect_req = format!(
            "CONNECT {}:{} HTTP/1.1\r\nHost: {}:{}\r\n{}\r\n",
            target_host, target_port,
            target_host, target_port,
            proxy.auth.as_ref().map(|a| format!(
                "Proxy-Authorization: Basic {}\r\n",
                base64::encode(format!("{}:{}", a.username, a.password))
            )).unwrap_or_default()
        );
        conn.write_all(connect_req.as_bytes())?;

        // 3. Read response
        let response = read_connect_response(&mut conn)?;
        if response.status != 200 {
            return Err(HttpClientError::ProxyTunnelFailed {
                status: response.status,
                message: response.reason,
            });
        }

        Ok(Connection::from_tcp(conn))
    }
}
```

### Feature Gates

```toml
[dependencies]
# SOCKS5 support (optional)
socks = { version = "0.3", optional = true }

[features]
socks5 = ["socks"]
```

## Success Criteria

- [ ] `proxy.rs` exists and compiles
- [ ] `ProxyConfig` correctly parses proxy URLs
- [ ] HTTP proxy with CONNECT tunneling works
- [ ] HTTPS proxy with TLS + CONNECT works
- [ ] Proxy authentication (Basic) works
- [ ] Environment variable detection works (HTTP_PROXY, HTTPS_PROXY)
- [ ] NO_PROXY bypass list works
- [ ] Per-request proxy override works
- [ ] `no_proxy()` bypasses client proxy for request
- [ ] SOCKS5 proxy works (feature-gated)
- [ ] Error handling covers all failure modes
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- proxy
cargo build --package foundation_core
cargo build --package foundation_core --features socks5
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** connection feature is complete
- **MUST READ** RFC 7231 Section 4.3.6 (CONNECT method)
- **MUST READ** existing netcap connection handling

### Implementation Guidelines
- HTTP proxy requires CONNECT method for HTTPS targets
- HTTPS proxy needs double TLS (to proxy, then to target)
- SOCKS5 must be feature-gated (different protocol)
- Environment variables have case-insensitive fallbacks
- NO_PROXY supports wildcards and domain matching
- Proxy auth uses same Base64 as Basic auth

### Security Considerations
- Proxy credentials should be handled securely
- Consider proxy credential exposure in logs
- Validate proxy certificates for HTTPS proxies

---
*Created: 2026-01-19*
*Last Updated: 2026-01-19*
