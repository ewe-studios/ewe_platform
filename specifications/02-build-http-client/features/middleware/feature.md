---
feature: middleware
description: Request/response interceptors for logging, timing, retry, and custom cross-cutting concerns
status: pending
priority: low
depends_on:
  - public-api
estimated_effort: medium
created: 2026-01-19
last_updated: 2026-01-24
author: Main Agent
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

# Middleware Feature

## Overview

Add a middleware system to the HTTP client for request/response interception. This feature enables cross-cutting concerns like logging, timing/metrics, automatic retry, and custom header injection without modifying core client logic.

## Dependencies

This feature depends on:
- `public-api` - Requires complete client for middleware integration

This feature is required by:
- None (end-user feature)

## Requirements

### Middleware Trait

Define the core middleware interface:

```rust
pub trait Middleware: Send + Sync {
    /// Called before the request is sent
    /// Return Err to abort the request
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError>;

    /// Called after the response is received
    /// Return Err to convert success to error
    fn handle_response(
        &self,
        request: &PreparedRequest,
        response: &mut ClientResponse,
    ) -> Result<(), HttpClientError>;

    /// Middleware name for debugging
    fn name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}
```

### Middleware Chain (Onion Model)

Middlewares execute in order for requests, reverse order for responses:

```rust
pub struct MiddlewareChain {
    middlewares: Vec<Arc<dyn Middleware>>,
}

impl MiddlewareChain {
    pub fn new() -> Self {
        Self { middlewares: Vec::new() }
    }

    pub fn add(&mut self, middleware: impl Middleware + 'static) {
        self.middlewares.push(Arc::new(middleware));
    }

    pub fn process_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        for mw in &self.middlewares {
            mw.handle_request(request)?;
        }
        Ok(())
    }

    pub fn process_response(
        &self,
        request: &PreparedRequest,
        response: &mut ClientResponse,
    ) -> Result<(), HttpClientError> {
        // Reverse order for responses (onion model)
        for mw in self.middlewares.iter().rev() {
            mw.handle_response(request, response)?;
        }
        Ok(())
    }
}
```

### Configuration API

```rust
// Add middleware to client
let client = SimpleHttpClient::new()
    .middleware(LoggingMiddleware::new())
    .middleware(TimingMiddleware::new())
    .middleware(RetryMiddleware::new(3, vec![429, 502, 503, 504]));

// Custom middleware
struct MyMiddleware;

impl Middleware for MyMiddleware {
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        request.headers.insert("X-Custom-Header", "value");
        Ok(())
    }

    fn handle_response(
        &self,
        _request: &PreparedRequest,
        response: &mut ClientResponse,
    ) -> Result<(), HttpClientError> {
        println!("Response status: {}", response.status());
        Ok(())
    }
}

// Per-request middleware skip
let response = client.get(url)
    .skip_middleware("LoggingMiddleware")
    .send()?;

// Skip all middleware for a request
let response = client.get(url)
    .no_middleware()
    .send()?;
```

### Built-in Middleware

#### LoggingMiddleware

```rust
pub struct LoggingMiddleware {
    log_body: bool,
    max_body_size: usize,
}

impl LoggingMiddleware {
    pub fn new() -> Self {
        Self {
            log_body: false,
            max_body_size: 1024,
        }
    }

    pub fn with_body(mut self, max_size: usize) -> Self {
        self.log_body = true;
        self.max_body_size = max_size;
        self
    }
}

impl Middleware for LoggingMiddleware {
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        log::debug!("Request: {} {}", request.method, request.url);
        for (name, value) in &request.headers {
            log::trace!("  {}: {}", name, value);
        }
        Ok(())
    }

    fn handle_response(
        &self,
        request: &PreparedRequest,
        response: &mut ClientResponse,
    ) -> Result<(), HttpClientError> {
        log::debug!(
            "Response: {} {} -> {}",
            request.method,
            request.url,
            response.status()
        );
        Ok(())
    }

    fn name(&self) -> &'static str {
        "LoggingMiddleware"
    }
}
```

#### TimingMiddleware

```rust
pub struct TimingMiddleware {
    on_timing: Option<Box<dyn Fn(&str, Duration) + Send + Sync>>,
}

impl TimingMiddleware {
    pub fn new() -> Self {
        Self { on_timing: None }
    }

    pub fn with_callback<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, Duration) + Send + Sync + 'static,
    {
        self.on_timing = Some(Box::new(callback));
        self
    }
}

impl Middleware for TimingMiddleware {
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        // Store start time in request extensions
        request.extensions.insert(Instant::now());
        Ok(())
    }

    fn handle_response(
        &self,
        request: &PreparedRequest,
        _response: &mut ClientResponse,
    ) -> Result<(), HttpClientError> {
        if let Some(start) = request.extensions.get::<Instant>() {
            let duration = start.elapsed();
            if let Some(ref callback) = self.on_timing {
                callback(&request.url.to_string(), duration);
            } else {
                log::info!("Request to {} took {:?}", request.url, duration);
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "TimingMiddleware"
    }
}
```

#### RetryMiddleware

```rust
pub struct RetryMiddleware {
    max_retries: u32,
    retry_status_codes: Vec<u16>,
    backoff: BackoffStrategy,
}

impl RetryMiddleware {
    pub fn new(max_retries: u32, retry_status_codes: Vec<u16>) -> Self {
        Self {
            max_retries,
            retry_status_codes,
            backoff: BackoffStrategy::Exponential {
                base: Duration::from_millis(100),
                multiplier: 2.0,
            },
        }
    }

    pub fn with_backoff(mut self, backoff: BackoffStrategy) -> Self {
        self.backoff = backoff;
        self
    }
}

impl Middleware for RetryMiddleware {
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        // Store retry state in extensions
        request.extensions.insert(RetryState::new(self.max_retries));
        Ok(())
    }

    fn handle_response(
        &self,
        request: &PreparedRequest,
        response: &mut ClientResponse,
    ) -> Result<(), HttpClientError> {
        if self.retry_status_codes.contains(&response.status()) {
            if let Some(state) = request.extensions.get_mut::<RetryState>() {
                if state.attempt < self.max_retries {
                    state.attempt += 1;
                    // Signal retry needed
                    return Err(HttpClientError::RetryNeeded {
                        attempt: state.attempt,
                        delay: self.backoff.next_delay(state.attempt),
                    });
                }
            }
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "RetryMiddleware"
    }
}
```

#### HeaderMiddleware

```rust
pub struct HeaderMiddleware {
    headers: Vec<(String, String)>,
}

impl HeaderMiddleware {
    pub fn new() -> Self {
        Self { headers: Vec::new() }
    }

    pub fn add(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }
}

impl Middleware for HeaderMiddleware {
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        for (name, value) in &self.headers {
            // Only add if not already present
            if !request.headers.contains_key(name) {
                request.headers.insert(name.clone(), value.clone());
            }
        }
        Ok(())
    }

    fn handle_response(
        &self,
        _request: &PreparedRequest,
        _response: &mut ClientResponse,
    ) -> Result<(), HttpClientError> {
        Ok(())
    }

    fn name(&self) -> &'static str {
        "HeaderMiddleware"
    }
}
```

### Request Extensions

Type-safe extension storage for passing data between middleware:

```rust
pub struct Extensions {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Extensions {
    pub fn new() -> Self {
        Self { map: HashMap::new() }
    }

    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T) {
        self.map.insert(TypeId::of::<T>(), Box::new(value));
    }

    pub fn get<T: 'static>(&self) -> Option<&T> {
        self.map.get(&TypeId::of::<T>())
            .and_then(|v| v.downcast_ref::<T>())
    }

    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T> {
        self.map.get_mut(&TypeId::of::<T>())
            .and_then(|v| v.downcast_mut::<T>())
    }
}
```

## Implementation Details

### File Structure

```
client/
├── middleware.rs    (NEW - Middleware trait, chain, built-in middleware)
├── extensions.rs    (NEW - Request extensions for middleware data)
└── ...
```

### Error Handling

```rust
#[derive(From, Debug)]
pub enum HttpClientError {
    // ... existing variants ...

    #[from(ignore)]
    MiddlewareError { middleware: String, message: String },

    #[from(ignore)]
    RetryNeeded { attempt: u32, delay: Duration },
}
```

## Success Criteria

- [ ] `middleware.rs` exists and compiles
- [ ] `Middleware` trait is defined correctly
- [ ] `MiddlewareChain` executes in correct order (onion model)
- [ ] `LoggingMiddleware` logs requests and responses
- [ ] `TimingMiddleware` records request duration
- [ ] `RetryMiddleware` retries on configured status codes
- [ ] `HeaderMiddleware` adds default headers
- [ ] `middleware()` builder method works
- [ ] `skip_middleware()` per-request skip works
- [ ] `no_middleware()` bypasses all middleware
- [ ] Request extensions work for passing data
- [ ] All unit tests pass
- [ ] Code passes `cargo fmt` and `cargo clippy`

## Verification Commands

```bash
cargo fmt -- --check
cargo clippy -- -D warnings
cargo test --package foundation_core -- middleware
cargo build --package foundation_core
```

## Notes for Agents

### Before Starting
- **MUST VERIFY** public-api feature is complete
- **MUST READ** existing request/response structures
- **MUST READ** valtron-utilities for BackoffStrategy (if available)

### Implementation Guidelines
- Middleware must be Send + Sync for thread safety
- Use Arc for sharing middleware across requests
- Onion model: request handlers run first-to-last, response handlers last-to-first
- Extensions use TypeId for type-safe storage
- Built-in middleware should be optional (feature-gated if heavy dependencies)

### Error Handling
- MiddlewareError should include middleware name
- RetryNeeded is a special error that triggers retry logic
- Middleware errors should be distinguishable from HTTP errors

---
*Created: 2026-01-19*
*Last Updated: 2026-01-19*
