---
feature: cookie-jar
completed: 0
uncompleted: 15
last_updated: 2026-01-19
tools:
  - Rust
  - cargo
---

# Cookie Jar - Tasks

## Task List

### Module Setup
- [ ] Create `client/cookie.rs` - Cookie and CookieJar module
- [ ] Add chrono or time dependency for date parsing (if needed)

### Core Types
- [ ] Define `Cookie` struct with all standard attributes
- [ ] Define `SameSite` enum (Strict, Lax, None)
- [ ] Define `CookieKey` for jar indexing
- [ ] Define `CookieJar` with HashMap storage
- [ ] Define `CookieParseError` enum

### Cookie Parsing
- [ ] Implement `Cookie::new(name, value)` constructor
- [ ] Implement `Cookie::parse(header)` for Set-Cookie parsing
- [ ] Implement builder methods (domain, path, secure, etc.)
- [ ] Implement HTTP date parsing for Expires attribute
- [ ] Implement Max-Age parsing

### Cookie Jar Operations
- [ ] Implement `CookieJar::new()`
- [ ] Implement `CookieJar::add(cookie)`
- [ ] Implement `CookieJar::get_for_url(url)`
- [ ] Implement `CookieJar::remove(domain, path, name)`
- [ ] Implement `CookieJar::clear()`
- [ ] Implement `CookieJar::clear_expired()`

### Matching Logic
- [ ] Implement domain matching (exact and subdomain)
- [ ] Implement path matching (prefix)
- [ ] Implement Secure attribute checking (HTTPS only)
- [ ] Implement expiration checking

### Client Integration
- [ ] Add automatic Set-Cookie parsing in response handling
- [ ] Add automatic Cookie header generation in request building
- [ ] Add `cookie_jar(bool)` to SimpleHttpClient builder
- [ ] Add `with_cookie_jar(jar)` for custom jar
- [ ] Add `with_shared_cookie_jar(Arc<Mutex<CookieJar>>)` for sharing
- [ ] Add `no_cookies()` per-request bypass

### Persistent Storage (Optional)
- [ ] Define `CookieStore` trait
- [ ] Implement `FileCookieStore` for file-based persistence
- [ ] Add `with_cookie_store()` method

## Implementation Order

1. **cookie.rs** - Core types (Cookie, SameSite, CookieKey)
2. **cookie.rs** - Set-Cookie header parsing
3. **cookie.rs** - CookieJar with add/get operations
4. **cookie.rs** - Matching logic (domain, path, security)
5. **Integration** - Response Set-Cookie handling
6. **Integration** - Request Cookie header generation
7. **client.rs** - Builder methods for cookie configuration
8. **Persistence** (optional) - CookieStore trait and FileCookieStore

## Notes

### Cookie Struct Pattern
```rust
pub struct Cookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub expires: Option<DateTime>,
    pub max_age: Option<Duration>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: SameSite,
}
```

### Set-Cookie Format
```
Set-Cookie: session=abc123; Path=/; Domain=.example.com; Secure; HttpOnly; SameSite=Lax
```

### Cookie Header Format
```
Cookie: session=abc123; user=john; theme=dark
```

### Domain Matching Pattern
```rust
fn domain_matches(cookie_domain: &str, request_host: &str) -> bool {
    cookie_domain == request_host
        || (cookie_domain.starts_with('.') && request_host.ends_with(cookie_domain))
}
```

---
*Last Updated: 2026-01-19*
