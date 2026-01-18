---
feature: request-response
completed: 0
uncompleted: 4
last_updated: 2026-01-18
tools:
  - Rust
  - cargo
---

# Request/Response - Tasks

## Task List

### Response Types
- [ ] Create `client/intro.rs` - ResponseIntro wrapper with From impl

### Request Types
- [ ] Create `client/request.rs` - PreparedRequest struct
- [ ] Implement `ClientRequestBuilder` with fluent API and all convenience methods

### Testing
- [ ] Write unit tests for request builder and response types

## Implementation Order

1. **intro.rs** - ResponseIntro (simple wrapper, no dependencies)
2. **PreparedRequest** - Internal request representation
3. **ClientRequestBuilder** - Fluent API (depends on PreparedRequest, ParsedUrl)
4. **Tests** - After implementations work

## Notes

### ResponseIntro Pattern
```rust
pub struct ResponseIntro {
    pub status: Status,
    pub proto: Proto,
    pub reason: Option<String>,
}

impl From<(Status, Proto, Option<String>)> for ResponseIntro {
    fn from((status, proto, reason): (Status, Proto, Option<String>)) -> Self {
        Self { status, proto, reason }
    }
}
```

### Builder Pattern
```rust
let request = ClientRequestBuilder::post("http://example.com/api")?
    .header(SimpleHeader::ContentType, "application/json")
    .body_json(&my_data)?
    .build();
```

### Default Headers
The builder should set sensible defaults:
- `Host` header from URL
- `User-Agent` header
- `Accept` header
- `Content-Length` for body requests

### Types to Import from impls.rs
```rust
use crate::wire::simple_http::{
    SimpleMethod, SimpleHeader, SimpleHeaders, SimpleBody,
    Status, Proto, Http11RequestIterator,
};
```

---
*Last Updated: 2026-01-18*
