# Fundamentals 01 — HTTP client and connection management

Zero-to-expert on the HTTP layer. If you're making requests, handling responses,
or building a provider adapter, you need this.

---

## 1. The HttpClient trait

All HTTP goes through this single trait:

```rust
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn send_async(&self, req: PreparedRequest)
        -> Result<SimpleResponse<SendSafeBody>, HttpClientError>;
    async fn send_sse_async(&self, req: PreparedRequest)
        -> Result<BoxedSseFutureStream, HttpClientError>;
    fn send(&self, req: PreparedRequest)
        -> Result<SimpleResponse<SendSafeBody>, HttpClientError>;
    fn send_sse(&self, req: PreparedRequest)
        -> Result<BoxedSseIterator, HttpClientError>;
}
```

Two implementations:
- **`SimpleHttpClient`** (native) — uses std networking, rustls for TLS
- **`FetchHttpClient`** (wasm) — uses `web_sys::fetch` API

## 2. PreparedRequest

A fully-formed HTTP request:

```rust
pub struct PreparedRequest {
    pub method: SimpleMethod,  // GET, POST, PUT, DELETE, etc.
    pub url: Uri,              // Parsed URL with scheme/host/path/query
    pub headers: SimpleHeaders, // Header name → list of values
    pub body: SendSafeBody,    // Text(bytes) or Bytes(Vec<u8>) or Stream
    pub extensions: Extensions, // Per-request metadata (timeouts, etc.)
}
```

## 3. SimpleResponse

The response from the server:

```rust
pub struct SimpleResponse<T>(Status, SimpleHeaders, T);

impl<T> SimpleResponse<T> {
    pub fn into_parts(self) -> (Status, SimpleHeaders, T);
    pub fn get_status(&self) -> Status;
    pub fn get_body_ref(&self) -> &T;
}
```

## 4. SendSafeBody

The request/response body — three variants:

- **`Text(String)`** — UTF-8 text (JSON, HTML, etc.)
- **`Bytes(Vec<u8>)`** — Raw binary (images, protobufs)
- **`Stream(Pin<Box<dyn Stream<Item = Result<Vec<u8>>> + Send>>)`** — Streaming body

The "safe" name refers to the fact that this type is always `Send` (unlike raw
`&str` or `&[u8]` references which have lifetimes).

## 5. Redirect following

The HTTP client supports automatic redirect following with configurable limits:

```rust
let config = RedirectConfig {
    max_redirects: 5,
    follow_policy: FollowPolicy::SameHost, // or All, None
};
```

## 6. Cookie management

Automatic cookie handling with domain/path scoping:

```rust
let cookies = CookieJar::new();
// Response cookies are automatically added to the jar
// Request cookies are automatically attached to matching URLs
```

Cookie attributes supported: `Domain`, `Path`, `Expires`, `Max-Age`, `Secure`,
`HttpOnly`, `SameSite`.

## 7. Latency and load tracking

The client tracks request latency and connection load for adaptive timeouts:

```rust
let tracker = LatencyTracker::new();
tracker.record_sample(Duration::from_millis(150));
let p99 = tracker.percentile(99.0); // 99th percentile latency

let load = LoadTracker::new();
load.record_request_start();
load.record_request_end();
let level = load.current_level(); // Low, Medium, High
```

Load levels affect timeout scaling:
- **Low** — 1.0× base timeout
- **Medium** — 0.9× base timeout (aggressive)
- **High** — 1.2× base timeout (lenient, system is under load)

## 8. Timeout handling

Three timeout types:
- **Connect timeout** — time to establish TCP/TLS connection
- **Read timeout** — time between receiving bytes (scales with body size)
- **Write timeout** — time to send the request body

Read timeout per KB: `base_timeout + (body_size_kb × timeout_per_kb)`. This
prevents large downloads from timing out while still catching stalled connections.

## 9. TLS verification

TLS is handled by the `rustls` backend (default) or `native-tls`:

```rust
// rustls: uses webpki-roots for CA certificates
// native-tls: uses the OS certificate store
```

TLS verification can be customized per-request via `Extensions`:
- Skip verification (testing only — NEVER in production)
- Pin a specific certificate
- Use a custom CA bundle

## 10. SSE (Server-Sent Events)

The SSE client handles the full SSE protocol:

```rust
let stream = client.send_sse_async(request).await?;
// stream: BoxedSseFutureStream — yields ParseResult items
```

`ParseResult` variants:
- **`Event { data, event_type, id, retry }`** — parsed SSE event
- **`Comment(text)`** — SSE comment line (ignored by most consumers)
- **`Progress { received, total }`** — connection progress

Automatic reconnection with exponential backoff is handled by the `SseClient`
wrapper around the raw SSE stream.
