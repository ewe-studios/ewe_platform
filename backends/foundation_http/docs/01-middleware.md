# Fundamentals 01 — Middleware pipeline

Zero-to-expert on the HTTP middleware system.

---

## 1. The middleware trait

```rust
#[async_trait]
pub trait Middleware: Send + Sync {
    async fn handle(&self, req: Request, next: Box<dyn Next>) -> Response;
}
```

Each middleware receives the request and a `Next` handler. It can:
- **Modify the request** before passing to `next`
- **Short-circuit** by returning a Response directly
- **Modify the response** after `next.await` returns

## 2. Built-in middleware

### Auth middleware
Checks `Authorization` header against `SessionAccessProvider`. Supports:
- Bearer tokens (JWT, API keys)
- Session cookies
- Public path exemptions (health checks, static files)

### CORS middleware
Handles preflight (OPTIONS) requests and adds CORS headers:
- `Access-Control-Allow-Origin`
- `Access-Control-Allow-Methods`
- `Access-Control-Allow-Headers`
- `Access-Control-Max-Age`

### Compression middleware
Compresses responses based on `Accept-Encoding`:
- gzip (deflate)
- deflate (zlib)
- brotli (if feature enabled)
- Minimum size threshold (don't compress tiny responses)

### Logger middleware
Logs request/response pairs:
- Method, path, status code
- Response time
- Client IP (optional)
- Request/response size (optional)

### BodyLimit middleware
Rejects requests with body size > configured limit:
- Returns 413 Payload Too Large
- Applied before parsing (streaming check)

## 3. Building the pipeline

```rust
let app = Router::new()
    .middleware(Auth::new(provider))
    .middleware(Cors::new(allowed_origins))
    .middleware(Logger::new())
    .middleware(BodyLimit::new(10 * 1024 * 1024)) // 10MB
    .route("/api/users", get(list_users))
    .route("/api/users", create_user);
```

Order matters: body limit runs first (reject large payloads early), then auth
(verify identity), then your handler.

## 4. Custom middleware

```rust
struct RateLimiter {
    store: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
    max_requests: usize,
    window: Duration,
}

#[async_trait]
impl Middleware for RateLimiter {
    async fn handle(&self, req: Request, next: Box<dyn Next>) -> Response {
        let ip = req.client_ip();
        if self.is_rate_limited(&ip) {
            return Response::status(429).body("Too many requests");
        }
        next.run(req).await
    }
}
```

## 5. Error handling in middleware

Middleware can transform errors:

```rust
async fn handle(&self, req: Request, next: Box<dyn Next>) -> Response {
    match next.run(req).await {
        Ok(response) => response,
        Err(e) => {
            log::error!("Handler error: {}", e);
            Response::status(500).json(&ErrorBody { message: "Internal error" })
        }
    }
}
```

## 6. Integration with foundation_ai

The AI API endpoints use middleware for:
- **Auth** — verify API key before allowing model access
- **Rate limiting** — per-user token budget enforcement
- **CORS** — allow browser-based AI apps
- **Compression** — compress large model responses
- **Logging** — track model usage, latency, errors
