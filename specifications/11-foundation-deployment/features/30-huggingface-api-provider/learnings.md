# Learnings: HuggingFace API Provider

## Architecture: Standalone Functions Pattern

**Date:** 2026-04-06

### Design Decision

The HuggingFace client follows a **standalone function pattern** where API operations are implemented as module-level functions that take a `&HFClient` instance as the first parameter, rather than methods on the client struct.

### Benefits

1. **Flexibility**: Users can initialize a client once and pass it to whichever operations they need
2. **Testability**: Functions are easier to test in isolation
3. **Consistency**: Follows the pattern used by other providers (e.g., fly_io)
4. **Composability**: Functions can be combined and reused more easily

### Usage

```rust
// Initialize client
let client = HFClient::builder()
    .token("hf_...")
    .build()?;

// Use standalone functions
let user = whoami(&client)?;
let models = list_models(&client, &ListModelsParams { .. })?;
let repo = HFRepository::new(client.clone(), "owner", "repo", RepoType::Model);
```

### Client Responsibilities

The `HFClient` struct retains:
- Configuration (endpoint, token, redirect handling)
- Connection management (SimpleHttpClient wrapper)
- Helper methods (`auth_headers()`, `apply_auth_headers()`, `api_url()`, `download_url()`)
- Repository handle constructors (`model()`, `dataset()`, `space()`)

### Operation Functions

All CRUD and listing operations are standalone functions:
- `whoami(client: &HFClient) -> Result<User>`
- `auth_check(client: &HFClient) -> Result<()>`
- `list_models(client: &HFClient, params: &ListModelsParams) -> Result<...>`
- `list_datasets(client: &HFClient, params: &ListDatasetsParams) -> Result<...>`
- `list_spaces(client: &HFClient, params: &ListSpacesParams) -> Result<...>`
- `create_repo(client: &HFClient, params: &CreateRepoParams) -> Result<RepoUrl>`
- `delete_repo(client: &HFClient, params: &DeleteRepoParams) -> Result<()>`
- `move_repo(client: &HFClient, params: &MoveRepoParams) -> Result<RepoUrl>`

## Parameter Structs

All parameter structs derive `JsonHash` and `Serialize` for:
- Deterministic caching based on parameter content
- Serialization for request building

### RepoDownloadFileParams

The `destination` field was renamed to `directory` and made **mandatory**:
- `directory` specifies where files should be stored
- The filename is automatically appended to form the complete path
- This ensures consistent behavior and avoids accidental overwrites

```rust
let params = RepoDownloadFileParams {
    filename: "config.json".to_string(),
    revision: Some("main".to_string()),
    directory: PathBuf::from("./downloads"),
};
// File will be saved to: ./downloads/config.json
```

## RepositoryArgs

For flexible repository construction, use `RepositoryArgs`:

```rust
let args = RepositoryArgs {
    owner: "meta-llama".to_string(),
    name: "Llama-2-7b".to_string(),
    repo_type: RepoType::Model,
    default_revision: Some("main".to_string()),
};
let repo = HFRepository::with_args(client, args);
```

---

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

---

## Manual 302 Redirect Handling for HuggingFace CDN Downloads

**Date:** 2026-04-06

### Problem

HuggingFace file downloads return HTTP 302 redirects to their CDN (`cas-bridge.xethub.hf.co`). The standard redirect flow in `RequestRedirect` was not being used because we were building requests manually with `build_client()`. This caused downloads to fail with "HTTP 302" errors.

### Root Cause

The `repo_download_file()` function was using `build_client()` + `start()` to get response intro/headers, but this bypassed the automatic redirect handling. When HuggingFace returned a 302, we received the redirect response directly instead of following it.

### Solution

Implemented manual 302 redirect handling with a two-part request flow:

1. **First Request** - Get response intro and headers
   - Use `request.start()` to get `(intro_stream, body_stream)`
   - Extract `ResponseIntro` and `SimpleHeaders` from intro stream
   - Check if status code is 302

2. **Second Request** (if 302) - Download from CDN URL
   - Extract `Location` header from first response
   - Preserve `LINK` header for Xet authentication
   - Make new GET request to CDN URL
   - Collect body bytes and write to file

### Implementation

**Files Changed:**

1. `backends/foundation_core/src/wire/simple_http/impls.rs`
   - Added `into_parts()` method to `SimpleResponse<T>` for decomposing response
   - Added `take_body()` method to extract body by value

2. `backends/foundation_core/src/wire/simple_http/client/api.rs`
   - Wrapped `SimpleResponse<T>` in `Option` inside `FinalizedResponse`
   - Added `into_parts()` returning `(Status, SimpleHeaders, T, Option<Pool>, Option<Conn>)`
   - Uses `Option::take()` to safely extract fields from type with `Drop` impl
   - Drop impl handles pool return when `into_parts()` is not called

3. `backends/foundation_core/src/wire/simple_http/client/body_reader.rs`
   - Added `collect_bytes_from_send_safe()` helper function
   - Handles all `SendSafeBody` variants: Text, Bytes, Stream, ChunkedStream, LineFeedStream, None
   - DRYs repetitive byte collection logic

4. `backends/foundation_deployment/src/providers/huggingface/repository.rs`
   - Updated `repo_download_file()` to handle 302 manually
   - Uses `RunOnDrop` guard for connection pool return after `into_parts()`
   - Preserves `LINK` header for Xet storage authentication

### Code Pattern: into_parts with RunOnDrop

```rust
let response = http_client
    .get(url)
    .build_client()?
    .send()?;

// Decompose response, get RunOnDrop guard for pool return
let (_, _, body, pool, conn) = response.into_parts();
let _guard = RunOnDrop::new(move || {
    if let (Some(pool), Some(conn)) = (pool, conn) {
        pool.return_to_pool(conn);
    }
});

// Collect bytes from any SendSafeBody variant
let bytes = collect_bytes_from_send_safe(body);
```

### Important Headers from HuggingFace 302 Response

| Header | Purpose |
|--------|---------|
| `Location` | CDN URL for actual file download (required) |
| `Link` | Xet authentication token and reconstruction info |
| `X-Linked-ETag` | Xet ETag for integrity verification |
| `X-Linked-Size` | Expected file size from Xet |
| `X-Xet-Hash` | Xet hash for integrity verification |

The presigned CDN URL in `Location` is self-contained with AWS S3 signatures, but preserving the `LINK` header enables Xet-specific authentication flows.

### Security Consideration

The `preserve_auth_on_redirect(true)` setting in `HFClientBuilder` ensures Bearer tokens are preserved when following redirects to the CDN. This is safe because HuggingFace controls both the API host and CDN host.

---

## FinalizedResponse.into_parts() Pattern

**Date:** 2026-04-06

### Motivation

`FinalizedResponse<T, R>` has a `Drop` impl that returns connections to the pool. This prevents moving fields out directly (Rust's "cannot move out of type with Drop trait" error).

### Pattern

1. Wrap the `SimpleResponse<T>` field in `Option`
2. Use `Option::take()` to extract values (replaces with `None`, returns original)
3. Return all components including pool/conn for caller-managed cleanup
4. Drop impl becomes no-op when fields are already `None`

```rust
pub fn into_parts(mut self) -> (Status, SimpleHeaders, T, Option<Arc<HttpConnectionPool<R>>>, Option<HttpClientConnection>) {
    let response = self.0.take().expect("response already taken");
    let status = response.get_status();
    let headers = response.get_headers_ref().clone();
    let body = response.take_body();
    let pool = self.1.take();
    let conn = self.2.take();
    (status, headers, body, pool, conn)
}
```

### Benefits

- No `unsafe` or `ManuallyDrop` required
- Drop impl still works correctly (sees `None` values, does nothing)
- Caller can use `RunOnDrop` guard for deferred pool return
- Clean separation of concerns

---

## collect_bytes_from_send_safe() Helper

**Date:** 2026-04-06

### Motivation

Multiple code paths needed to collect bytes from `SendSafeBody` variants. The match logic was repetitive and error-prone.

### Implementation

```rust
pub fn collect_bytes_from_send_safe(body: SendSafeBody) -> Vec<u8> {
    match body {
        SendSafeBody::Text(t) => t.into_bytes(),
        SendSafeBody::Bytes(b) => b,
        SendSafeBody::None => Vec::new(),
        SendSafeBody::Stream(mut opt_iter) => { /* collect chunks */ }
        SendSafeBody::ChunkedStream(mut opt_iter) => { /* collect chunked data */ }
        SendSafeBody::LineFeedStream(mut opt_iter) => { /* collect lines with newlines */ }
    }
}
```

### Usage

```rust
let response = client.get(url).send()?;
let (_, _, body, pool, conn) = response.into_parts();
let bytes = collect_bytes_from_send_safe(body);
```

### Benefits

- Single source of truth for byte collection logic
- Handles all variants including edge cases (LineFeedStream, ChunkedStream)
- Proper error logging via `tracing::warn!` for stream errors
- Returns empty `Vec` for `None` body (graceful degradation)
