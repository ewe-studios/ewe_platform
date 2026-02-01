---
feature: cookie-jar
description: Automatic cookie storage, sending, and lifecycle management
status: pending
priority: low
depends_on:
  - public-api
estimated_effort: medium
created: 2026-01-19
last_updated: 2026-01-24
author: Main Agent
context_optimization: true  # Sub-agents MUST generate COMPACT_CONTEXT.md before work, reload after updates
compact_context_file: ./COMPACT_CONTEXT.md  # Ultra-compact current task context (97% reduction)
context_reload_required: true  # Clear and reload from compact context regularly to prevent context limit errors
tasks:
  completed: 0
  uncompleted: 17
  total: 17
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

# Cookie Jar Feature

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

Add automatic cookie handling to the HTTP client. This feature provides a cookie jar that stores cookies from `Set-Cookie` headers, automatically sends matching cookies on subsequent requests, and handles cookie expiration, security attributes, and domain isolation.

## Dependencies

This feature depends on:
- `public-api` - Requires complete client for cookie integration

This feature is required by:
- None (end-user feature)

## Requirements

### Cookie Storage

Store and retrieve cookies with full attribute support:

```rust
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub expires: Option<DateTime<Utc>>,
    pub max_age: Option<Duration>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: SameSite,
}

pub enum SameSite {
    Strict,
    Lax,
    None,
}

impl Cookie {
    pub fn new(name: &str, value: &str) -> Self;
    pub fn parse(header: &str) -> Result<Self, CookieParseError>;
    pub fn domain(self, domain: &str) -> Self;
    pub fn path(self, path: &str) -> Self;
    pub fn secure(self, secure: bool) -> Self;
    pub fn http_only(self, http_only: bool) -> Self;
    pub fn expires(self, expires: DateTime<Utc>) -> Self;
    pub fn max_age(self, max_age: Duration) -> Self;
}
```

### Cookie Jar Implementation

```rust
pub struct CookieJar {
    cookies: HashMap<CookieKey, Cookie>,
}

#[derive(Hash, Eq, PartialEq)]
struct CookieKey {
    domain: String,
    path: String,
    name: String,
}

impl CookieJar {
    pub fn new() -> Self;

    /// Add a cookie to the jar
    pub fn add(&mut self, cookie: Cookie);

    /// Get cookies matching a URL
    pub fn get_for_url(&self, url: &ParsedUrl) -> Vec<&Cookie>;

    /// Remove a specific cookie
    pub fn remove(&mut self, domain: &str, path: &str, name: &str);

    /// Clear all cookies
    pub fn clear(&mut self);

    /// Clear expired cookies
    pub fn clear_expired(&mut self);

    /// Get all cookies for a domain
    pub fn get_for_domain(&self, domain: &str) -> Vec<&Cookie>;
}
```

### Automatic Cookie Handling

```rust
// On response: parse and store Set-Cookie headers
impl ClientResponse {
    fn store_cookies(&self, jar: &mut CookieJar, request_url: &ParsedUrl) {
        for header in self.headers.get_all("Set-Cookie") {
            if let Ok(cookie) = Cookie::parse(header) {
                let cookie = self.apply_defaults(cookie, request_url);
                jar.add(cookie);
            }
        }
    }
}

// On request: add matching cookies
impl PreparedRequest {
    fn add_cookies(&mut self, jar: &CookieJar, url: &ParsedUrl) {
        let cookies = jar.get_for_url(url);
        if !cookies.is_empty() {
            let cookie_header = cookies
                .iter()
                .map(|c| format!("{}={}", c.name, c.value))
                .collect::<Vec<_>>()
                .join("; ");
            self.headers.insert("Cookie", cookie_header);
        }
    }
}
```

### Cookie Matching Rules

Domain matching:

```rust
impl CookieJar {
    fn domain_matches(cookie_domain: &str, request_host: &str) -> bool {
        // Exact match
        if cookie_domain == request_host {
            return true;
        }

        // Subdomain match (cookie domain starts with .)
        if cookie_domain.starts_with('.') {
            let without_dot = &cookie_domain[1..];
            return request_host == without_dot
                || request_host.ends_with(cookie_domain);
        }

        false
    }

    fn path_matches(cookie_path: &str, request_path: &str) -> bool {
        request_path.starts_with(cookie_path)
    }
}
```

### Security Attributes

Handle Secure and HttpOnly:

```rust
impl CookieJar {
    pub fn get_for_url(&self, url: &ParsedUrl) -> Vec<&Cookie> {
        self.cookies.values()
            .filter(|c| {
                // Check domain match
                Self::domain_matches(&c.domain.as_ref().unwrap_or(&url.host), &url.host)
                // Check path match
                && Self::path_matches(&c.path.as_ref().unwrap_or(&"/".to_string()), &url.path)
                // Secure cookies only over HTTPS
                && (!c.secure || url.scheme == Scheme::Https)
                // Check expiration
                && !self.is_expired(c)
            })
            .collect()
    }
}
```

### Configuration API

```rust
// Enable cookie jar (default: disabled)
let client = SimpleHttpClient::new()
    .cookie_jar(true);

// Custom cookie jar
let jar = CookieJar::new();
let client = SimpleHttpClient::new()
    .with_cookie_jar(jar);

// Shared cookie jar across clients
let jar = Arc::new(Mutex::new(CookieJar::new()));
let client1 = SimpleHttpClient::new().with_shared_cookie_jar(jar.clone());
let client2 = SimpleHttpClient::new().with_shared_cookie_jar(jar.clone());

// Per-request cookie bypass
let response = client.get(url).no_cookies().send()?;

// Manual cookie manipulation
client.cookie_jar().add(Cookie::new("name", "value").domain("example.com"));
client.cookie_jar().clear();
```

### Persistent Storage (Optional)

```rust
pub trait CookieStore {
    fn save(&self, jar: &CookieJar) -> Result<(), CookieStoreError>;
    fn load(&self) -> Result<CookieJar, CookieStoreError>;
}

pub struct FileCookieStore {
    path: PathBuf,
}

impl FileCookieStore {
    pub fn new(path: impl Into<PathBuf>) -> Self;
}

// Usage
let store = FileCookieStore::new("cookies.json");
let jar = store.load().unwrap_or_default();
let client = SimpleHttpClient::new()
    .with_cookie_jar(jar)
    .with_cookie_store(store);  // Auto-save on drop
```

### Error Handling

```rust
#[derive(Debug)]
pub enum CookieParseError {
    InvalidFormat(String),
    InvalidDate(String),
    InvalidAttribute(String),
}

impl std::error::Error for CookieParseError {}
```

## Implementation Details

### File Structure

```
client/
├── cookie.rs    (NEW - Cookie, CookieJar, CookieStore)
└── ...
```

### Set-Cookie Header Parsing

```rust
impl Cookie {
    pub fn parse(header: &str) -> Result<Self, CookieParseError> {
        // Format: name=value; Path=/; Domain=.example.com; Secure; HttpOnly
        let parts: Vec<&str> = header.split(';').collect();

        // First part is name=value
        let (name, value) = parts[0].split_once('=')
            .ok_or_else(|| CookieParseError::InvalidFormat("Missing =".into()))?;

        let mut cookie = Cookie::new(name.trim(), value.trim());

        // Parse attributes
        for attr in &parts[1..] {
            let attr = attr.trim();
            if let Some((key, val)) = attr.split_once('=') {
                match key.to_lowercase().as_str() {
                    "path" => cookie.path = Some(val.to_string()),
                    "domain" => cookie.domain = Some(val.to_string()),
                    "expires" => cookie.expires = Some(parse_http_date(val)?),
                    "max-age" => cookie.max_age = Some(Duration::from_secs(val.parse()?)),
                    "samesite" => cookie.same_site = parse_same_site(val),
                    _ => {}
                }
            } else {
                match attr.to_lowercase().as_str() {
                    "secure" => cookie.secure = true,
                    "httponly" => cookie.http_only = true,
                    _ => {}
                }
            }
        }

        Ok(cookie)
    }
}
```

## Success Criteria

- [ ] `cookie.rs` exists and compiles
- [ ] `Cookie` struct supports all standard attributes
- [ ] `Cookie::parse()` correctly parses Set-Cookie headers
- [ ] `CookieJar` stores cookies correctly
- [ ] `CookieJar::get_for_url()` returns matching cookies
- [ ] Domain matching follows RFC 6265
- [ ] Path matching follows RFC 6265
- [ ] Secure cookies only sent over HTTPS
- [ ] Cookie expiration is honored
- [ ] Automatic Set-Cookie parsing works
- [ ] Automatic Cookie header sending works
- [ ] `cookie_jar(bool)` configuration works
- [ ] `with_cookie_jar()` accepts custom jar
- [ ] `no_cookies()` per-request bypass works
- [ ] Persistent storage works (optional)
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- cookie
cargo build --package foundation_core
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** public-api feature is complete
- **MUST READ** RFC 6265 (HTTP State Management Mechanism)
- **MUST READ** existing response header handling

### Implementation Guidelines
- Cookie parsing should be lenient (real-world cookies vary)
- Domain matching must handle leading dots
- Expiration uses either Expires or Max-Age (Max-Age takes precedence)
- SameSite defaults to Lax per modern browser behavior
- HttpOnly cookies should not be exposed via JavaScript-like APIs

### Thread Safety
- CookieJar must be thread-safe if shared
- Consider `Arc<Mutex<CookieJar>>` for shared jars
- Persistent storage needs careful synchronization

---
*Created: 2026-01-19*
*Last Updated: 2026-01-19*
