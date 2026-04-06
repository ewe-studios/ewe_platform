# Learnings: HuggingFace API Provider

## Redirect Auth Header Preservation for CDN Downloads

**Date:** 2026-04-06

### Problem

HuggingFace file downloads use a redirect pattern:
1. Client requests file from `huggingface.co/api/...`
2. Server responds with HTTP 302 redirect to CDN (`cas-bridge.xethub.hf.co`)
3. CDN requires the same Bearer token for authenticated downloads

The original `strip_sensitive_headers_for_redirect()` implementation unconditionally stripped the `Authorization` header when redirecting to a different host, breaking authenticated downloads.

### Solution

Made the header stripping behavior configurable by adding two flags to `ClientConfig`:

```rust
pub struct ClientConfig {
    // ... existing fields ...
    
    /// Whether to preserve Authorization header on cross-host redirects (default: false)
    pub preserve_auth_on_redirect: bool,
    
    /// Whether to preserve Cookie header on cross-host redirects (default: false)
    pub preserve_cookies_on_redirect: bool,
}
```

### Implementation

**Files Changed:**

1. `backends/foundation_core/src/wire/simple_http/client/redirects.rs`
   - Added `preserve_auth` and `preserve_cookies` parameters to `strip_sensitive_headers_for_redirect()`
   - Updated `build_followup_request_from_request_descriptor()` and `build_followup_request_from()` to pass flags

2. `backends/foundation_core/src/wire/simple_http/client/client.rs`
   - Added config fields with default `false` (secure by default)
   - Added builder methods: `with_preserve_auth_on_redirect()`, `with_preserve_cookies_on_redirect()`
   - Added `SimpleHttpClient` methods: `preserve_auth_on_redirect()`, `preserve_cookies_on_redirect()`

3. `backends/foundation_core/src/wire/simple_http/client/tasks/request_redirect.rs`
   - Updated redirect handling to use config flags when building follow-up requests

4. `backends/foundation_deployment/src/providers/huggingface/client.rs`
   - Set `preserve_auth_on_redirect(true)` in `HFClientBuilder::build()`

### Security Consideration

**Default behavior remains secure:** Auth and cookies are stripped on cross-host redirects by default. Users must explicitly opt-in to preservation for trusted redirect chains.

### Usage

```rust
// For HuggingFace authenticated CDN downloads
let client = SimpleHttpClient::from_system()
    .preserve_auth_on_redirect(true);

// For applications requiring cookies on cross-host redirects (e.g., SSO)
let client = SimpleHttpClient::from_system()
    .preserve_cookies_on_redirect(true);

// Both (for trusted redirect chains)
let client = SimpleHttpClient::from_system()
    .preserve_auth_on_redirect(true)
    .preserve_cookies_on_redirect(true);
```

### Testing

Added 5 unit tests in `redirects.rs`:
- `test_strip_sensitive_headers_same_host` - nothing stripped on same-host redirect
- `test_strip_sensitive_headers_different_host_default_behavior` - both stripped by default
- `test_strip_sensitive_headers_preserve_auth_only` - only auth preserved
- `test_strip_sensitive_headers_preserve_cookies_only` - only cookies preserved
- `test_strip_sensitive_headers_preserve_both` - both preserved

### Best Practices

When implementing security features:
1. **Default to secure behavior** - strip sensitive headers on cross-host redirects
2. **Provide escape hatches** - allow opt-in for legitimate use cases (CDN downloads, SSO flows)
3. **Make configuration explicit** - clear method names and documentation
4. **Test all combinations** - verify all flag combinations work correctly

---

## HuggingFace-Specific Learnings

### Token Resolution Order

The HuggingFace client resolves tokens in the following order:
1. `HF_TOKEN` environment variable
2. `HF_TOKEN_PATH` environment variable (file containing token)
3. `$HF_HOME/token` (default: `~/.cache/huggingface/token`)
4. `~/.cache/huggingface/token` (fallback)

### Repository URL Patterns

Different repository types use different URL prefixes:
- **Model** (default): No prefix
- **Dataset**: `datasets/`
- **Space**: `spaces/`
- **Kernel**: `kernels/`

### CDN Redirect Pattern

File downloads follow this pattern:
```
GET /api/models/{repo_id}/resolve/{revision}/{filename}
→ 302 Found → Location: https://cas-bridge.xethub.hf.co/xet/{bucket}/{path}
```

The CDN requires the same Bearer token as the main API, hence `preserve_auth_on_redirect(true)`.

### API Endpoints Summary

| Operation | Endpoint | Method |
|-----------|----------|--------|
| User info | `/api/whoami-v2` | GET |
| List models | `/api/models` | GET |
| List datasets | `/api/datasets` | GET |
| List spaces | `/api/spaces` | GET |
| Create repo | `/api/repos/create` | POST |
| Delete repo | `/api/repos/delete` | DELETE |
| Move repo | `/api/repos/move` | POST |
| Get repo info | `/api/{type}s/{repo_id}` | GET |
| Download file | `/{prefix}{repo_id}/resolve/{revision}/{filename}` | GET |
| Create commit | `/api/{type}s/{repo_id}/commit/{revision}` | POST |
