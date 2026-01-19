---
feature: middleware
completed: 0
uncompleted: 14
last_updated: 2026-01-19
tools:
  - Rust
  - cargo
---

# Middleware - Tasks

## Task List

### Module Setup
- [ ] Create `client/middleware.rs` - Middleware trait and chain
- [ ] Create `client/extensions.rs` - Request extensions for middleware data

### Core Types
- [ ] Define `Middleware` trait with handle_request/handle_response
- [ ] Define `MiddlewareChain` with onion-model execution
- [ ] Define `Extensions` for type-safe middleware data passing

### Built-in Middleware
- [ ] Implement `LoggingMiddleware` with configurable body logging
- [ ] Implement `TimingMiddleware` with callback support
- [ ] Implement `RetryMiddleware` with backoff strategy
- [ ] Implement `HeaderMiddleware` for default headers

### Client Integration
- [ ] Add `extensions` field to PreparedRequest
- [ ] Add `middleware()` method to SimpleHttpClient builder
- [ ] Add `skip_middleware()` per-request method
- [ ] Add `no_middleware()` per-request method
- [ ] Integrate MiddlewareChain into request execution flow

### Error Handling
- [ ] Add `MiddlewareError` variant to HttpClientError
- [ ] Add `RetryNeeded` variant for retry signaling
- [ ] Handle retry logic in request execution

## Implementation Order

1. **extensions.rs** - Type-safe extension storage (dependency for middleware)
2. **middleware.rs** - Middleware trait definition
3. **middleware.rs** - MiddlewareChain with execution logic
4. **Built-in** - LoggingMiddleware (simplest)
5. **Built-in** - HeaderMiddleware (simple)
6. **Built-in** - TimingMiddleware (uses extensions)
7. **Built-in** - RetryMiddleware (uses extensions and errors)
8. **errors.rs** - Add new error variants
9. **Integration** - Add to request/client builders
10. **Integration** - Wire into request execution

## Notes

### Middleware Trait Pattern
```rust
pub trait Middleware: Send + Sync {
    fn handle_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError>;
    fn handle_response(
        &self,
        request: &PreparedRequest,
        response: &mut ClientResponse,
    ) -> Result<(), HttpClientError>;
    fn name(&self) -> &'static str;
}
```

### Onion Model Execution
```rust
// Request: A -> B -> C
// Response: C -> B -> A

impl MiddlewareChain {
    pub fn process_request(&self, request: &mut PreparedRequest) -> Result<(), HttpClientError> {
        for mw in &self.middlewares {
            mw.handle_request(request)?;
        }
        Ok(())
    }

    pub fn process_response(&self, ...) -> Result<(), HttpClientError> {
        for mw in self.middlewares.iter().rev() {
            mw.handle_response(request, response)?;
        }
        Ok(())
    }
}
```

### Extensions Pattern
```rust
pub struct Extensions {
    map: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl Extensions {
    pub fn insert<T: Send + Sync + 'static>(&mut self, value: T);
    pub fn get<T: 'static>(&self) -> Option<&T>;
    pub fn get_mut<T: 'static>(&mut self) -> Option<&mut T>;
}
```

---
*Last Updated: 2026-01-19*
