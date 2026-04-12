---
workspace_name: "ewe_platform"
spec_directory: "specifications/08-foundation-errstacks"
feature_directory: "specifications/08-foundation-errstacks"
this_file: "specifications/08-foundation-errstacks/feature.md"

status: draft
priority: high
created: 2026-04-12

depends_on: []

tasks:
  completed: 0
  uncompleted: 21
  total: 21
  completion_percentage: 0%
---

# Foundation ErrStacks Specification

**Status:** Draft (Pending Review)
**Created:** 2026-04-12
**Target Crate:** `foundation_errstacks`

---

## 1. Executive Summary

This specification defines `foundation_errstacks` - a minimal, ergonomic error-handling library for the Foundation project ecosystem. It extracts the core value proposition from `error-stack` (HASH's context-aware error library) while:

- **Minimizing dependencies** - no async runtime requirements, minimal third-party deps
- **Leveraging `derive_more`** - for expressive, custom error types with less boilerplate
- **Providing std::Result-style ergonomics** - familiar patterns with enhanced capabilities
- **Enabling structured error traces** - for debugging, logging, and Slack alerting

### 1.1 Why This Exists

The existing `error-stack` crate (at `/home/darkvoid/Boxxed/@formulas/src.rust/src.hashintel/hash/libs/error-stack/`) provides excellent context-aware error handling but:

- Has optional dependencies on `anyhow`, `eyre`, `tracing`, `futures`
- Supports `no_std` which adds complexity we don't need
- Uses a global hook system that may be overkill for our use case
- Has a complex formatting subsystem

For the Foundation project, we need:
1. **Simpler mental model** - focused on the core context/attachment pattern
2. **Better derive integration** - using `derive_more` for custom error types
3. **Async-optional** - no required async runtime dependencies
4. **Slack-friendly output** - structured error traces suitable for alerting

---

## 2. Core Concepts (Extracted from error-stack)

### 2.1 The Report/Context Model

```
Report<C> = A stack of Frames, where C is the "current context"

Frame Types:
├── Context Frame    - An Error that provides semantic meaning
├── Printable Attachment - Display+Debug data attached to help debugging
└── Opaque Attachment    - Any Send+Sync+'static data for programmatic access
```

**Key Insight:** The generic parameter `C` in `Report<C>` enforces that errors are always viewed through a contextual lens. When crossing module/crate boundaries, you _must_ change context, which improves error documentation.

### 2.2 Error Trace Structure

```
ErrorTrace {
    frames: Vec<Frame>,           // Stack of contexts + attachments
    current_context: PhantomData<C>, // Type-level tracking of "current view"
}

Frame {
    kind: FrameKind,              // Context or Attachment
    sources: Vec<Frame>,          // Child frames (the "caused by" chain)
}

FrameKind:
├── Context(T)     where T: Error + Send + Sync + 'static
├── Printable(A)   where A: Display + Debug + Send + Sync + 'static
└── Opaque(A)      where A: Send + Sync + 'static
```

### 2.3 The Attachment System

Attachments are the key differentiator from plain `anyhow`-style errors:

```rust
// Printable - shown in Display/Debug output
report.attach(format!("user_id={}", user_id))
      .attach(format!("request_path={}", path));

// Opaque - programmatic access only
report.attach_opaque(RequestMetadata { trace_id, span_id })
      .attach_opaque(UserContext { org_id, role });

// Later retrieval
let metadata = report.downcast_ref::<RequestMetadata>();
```

---

## 3. Design Goals

### 3.1 Primary Goals

| Goal | Rationale |
|------|-----------|
| **Minimal deps** | Only `std`, `derive_more`, optionally `serde` |
| **No required async** | Async support via optional feature, not default |
| **Familiar ergonomics** | `ResultExt` trait on standard `Result` |
| **Type-safe contexts** | Generic parameter enforces context awareness |
| **Rich attachments** | Any `Send + Sync + 'static` type |
| **Structured output** | JSON-serializable for logging/Slack |

### 3.2 Non-Goals (Explicitly Out of Scope)

- `no_std` support (we target server/Cloudflare Workers)
- `anyhow`/`eyre` compatibility layers
- Global hook system (too complex for v1)
- `tracing`/`SpanTrace` integration (can be added later)
- Procedural macros (use `derive_more` instead)

---

## 4. API Specification

### 4.1 Core Types

#### 4.1.1 `ErrorTrace<C>` (analogous to `Report<C>`)

```rust
/// A structured error trace with context and attachments.
///
/// `C` is the "current context" - the error type that describes
/// how the current code interprets the underlying failure.
pub struct ErrorTrace<C: ?Sized> {
    frames: Box<Vec<Frame>>,
    _context: PhantomData<fn() -> *const C>,
}

impl<C> ErrorTrace<C> {
    /// Create a new ErrorTrace from an Error context
    pub fn new(context: C) -> Self
    where
        C: std::error::Error + Send + Sync + 'static;

    /// Change the context type (when crossing module boundaries)
    pub fn change_context<T>(self, context: T) -> ErrorTrace<T>
    where
        T: std::error::Error + Send + Sync + 'static;

    /// Add a printable attachment (shown in Display/Debug)
    pub fn attach<A>(self, attachment: A) -> Self
    where
        A: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static;

    /// Add an opaque attachment (programmatic access only)
    pub fn attach_opaque<A>(self, attachment: A) -> Self
    where
        A: Send + Sync + 'static;

    /// Lazy variants (closure only called on error)
    pub fn attach_with<A, F>(self, f: F) -> Self
    where
        A: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
        F: FnOnce() -> A;

    pub fn attach_opaque_with<A, F>(self, f: F) -> Self
    where
        A: Send + Sync + 'static,
        F: FnOnce() -> A;

    /// Downcast to get an attachment or context
    pub fn downcast_ref<T>(&self) -> Option<&T>
    where
        T: Send + Sync + 'static;

    /// Check if trace contains a type
    pub fn contains<T>(&self) -> bool
    where
        T: Send + Sync + 'static;

    /// Iterate over frames
    pub fn frames(&self) -> FrameIter<'_>;

    /// Get current context
    pub fn current_context(&self) -> &C
    where
        C: 'static;
}
```

#### 4.1.2 `Frame`

```rust
/// A single context or attachment in an ErrorTrace
pub struct Frame {
    frame: Box<dyn FrameImpl>,
    sources: Box<[Frame]>,
}

/// Internal trait implemented by all frame types.
/// This is an implementation detail - users interact with `Frame` directly.
pub(crate) trait FrameImpl: Send + Sync + 'static {
    /// Returns the kind of frame (context or attachment)
    fn kind(&self) -> FrameKind<'_>;
    
    /// Returns self as Any for downcasting
    fn as_any(&self) -> &dyn std::any::Any;
    
    /// Returns self as mutable Any for downcasting
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    
    /// Provide values for the Provider API (nightly only)
    #[cfg(nightly)]
    fn provide<'a>(&'a self, request: &mut std::error::Request<'a>);
}

/// Iterator over frames in an ErrorTrace
pub struct FrameIter<'a> {
    frames: std::slice::Iter<'a, Frame>,
}

impl<'a> Iterator for FrameIter<'a> {
    type Item = &'a Frame;
    fn next(&mut self) -> Option<Self::Item>;
}

pub enum FrameKind<'a> {
    Context(&'a dyn std::error::Error),
    Attachment(AttachmentKind<'a>),
}

pub enum AttachmentKind<'a> {
    Printable(&'a dyn (std::fmt::Display + std::fmt::Debug)),
    Opaque(&'a dyn std::any::Any),
}
```

**Implementation Notes:**
- `FrameImpl` is implemented for internal types: `ContextFrame<C>`, `AttachmentFrame<A>`, `PrintableAttachmentFrame<A>`
- Each frame type wraps the actual data and provides the trait methods
- `FrameIter` wraps a `std::slice::Iter` for zero-cost abstraction

#### 4.1.3 `ResultExt` Trait

```rust
/// Extension trait for Result to provide error trace methods
pub trait ResultExt {
    type Context: ?Sized;
    type Ok;

    // Printable attachments
    fn attach<A>(self, attachment: A) -> Result<Self::Ok, ErrorTrace<Self::Context>>
    where
        A: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static;

    fn attach_with<A, F>(self, f: F) -> Result<Self::Ok, ErrorTrace<Self::Context>>
    where
        A: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static,
        F: FnOnce() -> A;

    // Opaque attachments
    fn attach_opaque<A>(self, attachment: A) -> Result<Self::Ok, ErrorTrace<Self::Context>>
    where
        A: Send + Sync + 'static;

    fn attach_opaque_with<A, F>(self, f: F) -> Result<Self::Ok, ErrorTrace<Self::Context>>
    where
        A: Send + Sync + 'static,
        F: FnOnce() -> A;

    // Context changes
    fn change_context<T>(self, context: T) -> Result<Self::Ok, ErrorTrace<T>>
    where
        T: std::error::Error + Send + Sync + 'static;

    fn change_context_lazy<T, F>(self, f: F) -> Result<Self::Ok, ErrorTrace<T>>
    where
        T: std::error::Error + Send + Sync + 'static,
        F: FnOnce() -> T;
}

impl<T, E> ResultExt for Result<T, E>
where
    E: Into<ErrorTrace<E>>,
{
    // ... implementation
}
```

### 4.2 Convenience Macros

```rust
/// Create and return an ErrorTrace immediately
macro_rules! bail {
    ($err:expr $(,)?) => {{
        return Err($crate::IntoErrorTrace::into_error_trace($err));
    }};
}

/// Ensure condition or return error
macro_rules! ensure {
    ($cond:expr, $err:expr $(,)?) => {{
        if !bool::from($cond) {
            bail!($err);
        }
    }};
}

/// Create an ErrorTrace from an error type with attachments
macro_rules! report {
    ($err:expr) => {{
        $crate::ErrorTrace::new($err)
    }};
    ($err:expr, $($attachment:expr),+ $(,)?) => {{
        $crate::ErrorTrace::new($err)
            $(.attach($attachment))+*
    }};
}
```

### 4.3 Integration with `derive_more`

The library is designed to work seamlessly with `derive_more` for custom error types:

```rust
use derive_more::{Display, Error, From};
use foundation_errstacks::{ErrorTrace, ResultExt};

#[derive(Debug, Display, Error, From)]
pub enum DatabaseError {
    #[display("connection failed: {_0}")]
    Connection(String),

    #[display("query failed: {query}")]
    Query {
        query: String,
    },

    #[display("constraint violation: {constraint}")]
    Constraint {
        constraint: String,
    },
}

// Usage with context changes
fn get_user(id: UserId) -> Result<User, ErrorTrace<DatabaseError>> {
    let conn = get_connection()
        .change_context(DatabaseError::Connection("pool exhausted".to_string()))?;

    let row = conn.query("SELECT * FROM users WHERE id = ?", [id])
        .map_err(|e| DatabaseError::Query {
            query: format!("SELECT * FROM users WHERE id = {:?}", id)
        })?;

    Ok(User::from_row(row)?)
}
```

---

## 5. Output Formatting

### 5.1 Location and Backtrace Handling

**Location Capture:**
- Every `ErrorTrace::new()` call captures `core::panic::Location` via `#[track_caller]`
- Location is stored as an opaque attachment on the frame
- Displayed in Debug output as `at file.rs:line:col`
- Can be hidden via custom Debug hook (future feature)

**Backtrace Capture (optional, feature-gated):**
- Enabled via `backtrace` feature flag
- Uses `std::backtrace::Backtrace` from standard library
- Captured when:
  - `RUST_BACKTRACE=1` or `RUST_LIB_BACKTRACE=1` is set, AND
  - The root error does not already provide a backtrace
- Multiple backtraces can exist in a single `ErrorTrace` (one per context boundary)
- Displayed in Debug output after location information

```rust
// Internal implementation detail
impl<C> ErrorTrace<C> {
    #[track_caller]
    pub fn new(context: C) -> Self
    where
        C: std::error::Error + Send + Sync + 'static,
    {
        let location = *core::panic::Location::caller();
        
        #[cfg(feature = "backtrace")]
        let backtrace = std::backtrace::Backtrace::capture();
        
        // Create frame with location (and optionally backtrace) attachments
        // ...
    }
}
```

### 5.2 Display Implementations

```rust
// Basic Display - shows only top-level context
impl<C> fmt::Display for ErrorTrace<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.current_context())
    }
}

// Alternate Display ({:#}) - shows all contexts
impl<C> fmt::Display for ErrorTrace<C>
where
    C: 'static,
{
    // Shows: "context1: context2: context3: root_cause"
}

// Debug - shows full trace with attachments
impl<C> fmt::Debug for ErrorTrace<C>
where
    C: 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Full tree visualization like error-stack:
        // Error: context message
        // ├╴at file.rs:line:col
        // ├╴attachment1
        // ├╴attachment2
        // │
        // ╰─▶ caused by: inner context
        //     ├╴at inner.rs:line:col
        //     ╰╴attachment
    }
}
```

### 5.2 Structured Output (for Slack/Logging)

```rust
impl<C> ErrorTrace<C>
where
    C: 'static,
{
    /// Convert to structured JSON-serializable representation
    pub fn to_structured(&self) -> ErrorTraceStruct {
        ErrorTraceStruct {
            message: self.current_context().to_string(),
            frames: self.frames()
                .map(|frame| FrameStruct {
                    kind: match frame.kind() {
                        FrameKind::Context(ctx) => FrameKindStruct::Context(ctx.to_string()),
                        FrameKind::Attachment(AttachmentKind::Printable(att)) =>
                            FrameKindStruct::Printable(att.to_string()),
                        FrameKind::Attachment(AttachmentKind::Opaque(_)) =>
                            FrameKindStruct::Opaque,
                    },
                    // Optionally include location info
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
pub struct ErrorTraceStruct {
    pub message: String,
    pub frames: Vec<FrameStruct>,
}

#[derive(Serialize)]
pub struct FrameStruct {
    pub kind: FrameKindStruct,
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum FrameKindStruct {
    #[serde(rename = "context")]
    Context(String),
    #[serde(rename = "printable")]
    Printable(String),
    #[serde(rename = "opaque")]
    Opaque,
}
```

### 5.3 Slack Alert Format

```rust
/// Format for Slack blocks API
pub fn to_slack_blocks<C>(trace: &ErrorTrace<C>) -> serde_json::Value
where
    C: 'static,
{
    json!({
        "blocks": [
            {
                "type": "section",
                "text": {
                    "type": "mrkdwn",
                    "text": format!("*Error:* {}", trace.current_context())
                }
            },
            // Add attachments as context blocks
            {
                "type": "context",
                "elements": trace.frames()
                    .filter_map(|frame| match frame.kind() {
                        FrameKind::Attachment(AttachmentKind::Printable(a)) =>
                            Some(json!({"type": "mrkdwn", "text": a.to_string()})),
                        _ => None,
                    })
                    .collect()
            }
        ]
    })
}
```

---

## 6. Crate Structure

```
foundation_errstacks/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API, re-exports
│   ├── error_trace.rs      # Core ErrorTrace type
│   ├── frame.rs            # Frame types and iteration
│   ├── result_ext.rs       # ResultExt trait
│   ├── macros.rs           # bail!, ensure!, report!
│   ├── fmt/
│   │   ├── mod.rs          # Display/Debug implementations
│   │   └── structured.rs   # JSON/Slack formatters
│   └── serde.rs            # Optional serialization (feature-gated)
└── tests/
    ├── basic_tests.rs
    ├── attach_tests.rs
    ├── context_tests.rs
    └── formatting_tests.rs
```

---

## 7. Dependencies

### 7.1 Required

```toml
[dependencies]
# derive_more for error type derives
derive_more = { version = "1", features = ["display", "error", "from"] }
```

### 7.2 Optional Features

```toml
[features]
default = []

# Enable serde serialization
serde = ["dep:serde"]

# Enable backtrace capture (std only)
backtrace = []

# Enable async Future extension methods
async = ["dep:futures-core"]

# Enable Slack formatting helpers
slack = ["serde", "dep:serde_json"]
```

---

## 8. Usage Examples

### 8.1 Basic Usage

```rust
use foundation_errstacks::{ErrorTrace, ResultExt};
use derive_more::{Display, Error, From};

#[derive(Debug, Display, Error)]
#[display("file operation failed")]
struct FileError;

#[derive(Debug, Display, Error)]
#[display("parse error: invalid format")]
struct ParseError;

fn read_config(path: &str) -> Result<Config, ErrorTrace<FileError>> {
    std::fs::read_to_string(path)
        .attach(format!("path={}", path))
        .attach_opaque(ConfigLocation::new(path))
        .change_context(FileError)
}

fn parse_config(content: &str) -> Result<Config, ErrorTrace<ParseError>> {
    serde_json::from_str(content)
        .attach(format!("content_preview={}", &content[..50.min(content.len())]))
        .change_context(ParseError)
}

fn load_config(path: &str) -> Result<Config, ErrorTrace<AppError>> {
    let content = read_config(path)?;
    parse_config(&content)
        .change_context(AppError::ConfigLoadFailed)
}
```

### 8.2 Multiple Attachments

```rust
#[derive(Debug, Clone)]
struct RequestContext {
    user_id: UserId,
    request_id: RequestId,
    trace_id: TraceId,
}

fn process_request(ctx: RequestContext) -> Result<Response, ErrorTrace<ApiError>> {
    validate_input(&ctx)
        .attach(format!("user_id={}", ctx.user_id))
        .attach(format!("request_id={}", ctx.request_id))
        .attach_opaque(ctx.clone())
        .change_context(ApiError::ValidationFailed)?;

    // ...
}
```

### 8.3 Downcasting for Programmatic Access

```rust
match load_config("config.json") {
    Ok(config) => { /* ... */ }
    Err(trace) => {
        // Get structured data
        if let Some(ctx) = trace.downcast_ref::<RequestContext>() {
            log::error!("Failed for user: {}", ctx.user_id);
        }

        // Check error type
        if trace.contains::<FileError>() {
            // Handle file-specific case
        }

        // Format for Slack
        let slack_payload = trace.to_slack_blocks();
        send_slack_alert(slack_payload).await;
    }
}
```

---

## 9. Migration Path from error-stack

For code already using `error-stack`:

```rust
// error-stack
use error_stack::{Report, ResultExt};
let err: Report<MyError> = do_thing().change_context(MyError)?;

// foundation_errstacks
use foundation_errstacks::{ErrorTrace, ResultExt};
let err: ErrorTrace<MyError> = do_thing().change_context(MyError)?;
```

Key differences:
- `Report` → `ErrorTrace`
- No `anyhow`/`eyre` compatibility (use `From` trait instead)
- Simpler hook system (none in v1, may add later)
- Better `derive_more` integration examples

---

## 10. Testing Strategy

### 10.1 Unit Tests

- `ErrorTrace::new()` creates proper frame stack
- `attach()` adds printable frames
- `attach_opaque()` adds opaque frames
- `change_context()` properly transforms types
- `downcast_ref()` retrieves correct types
- `contains()` returns accurate results

### 10.2 Integration Tests

- Multiple context changes preserve full trace
- Attachments survive context changes
- Serialization round-trips (serde feature)
- Display/Debug output matches expected format

### 10.3 Compile-Fail Tests

- Ensure context type changes are enforced
- Verify Send + Sync bounds
- Test trait object safety

---

## 11. Future Considerations

### 11.1 Potential v2 Features

- Global hook system (if needed for custom formatting)
- `tracing` integration for SpanTrace
- `anyhow`/`eyre` compatibility layer
- Async `TryFutureExt` / `TryStreamExt` implementations
- `no_std` support (if community demand exists)

### 11.2 Related Crates

Consider separate crates for:
- `foundation_errstacks-slack` - Slack-specific formatters
- `foundation_errstacks-tracing` - tracing ecosystem integration
- `foundation_errstacks-axum` - Web framework error handling

---

## 12. Appendix: Key error-stack Files Reference

For implementation reference, these files contain the core logic:

| File | Purpose | Key Concepts |
|------|---------|--------------|
| `src/report.rs` | Core `Report<C>` type | Frame storage, context management |
| `src/frame/mod.rs` | `Frame` struct | Context vs Attachment distinction |
| `src/frame/frame_impl.rs` | `FrameImpl` trait | Internal frame representation |
| `src/context.rs` | `ResultExt` trait | Extension methods for `Result` |
| `src/fmt/mod.rs` | Formatting system | Debug output, hooks |
| `src/macros.rs` | `bail!`, `ensure!` | Convenience macros |
| `src/hook/mod.rs` | Global hook system | Debug format hooks |
| `src/iter.rs` | Frame iteration | Request API for nightly features |

---

## 13. Decision Log

### 13.1 Why `ErrorTrace` instead of `Report`?

- `Report` has connotations of "already formatted"
- `ErrorTrace` better describes "trace of error propagation"
- Avoids confusion with existing `Report` types in ecosystem

### 13.2 Why minimize async?

- Most error handling is synchronous
- Async adds dependency complexity (tokio vs async-std)
- Can add via optional `async` feature later
- Follows "pay for what you use" principle

### 13.3 Why `derive_more` over custom macros?

- Well-maintained, stable crate
- Users likely already have it
- Less maintenance burden for us
- More flexible for users (they control derives)

### 13.4 Why no hooks in v1?

- Adds significant complexity (global state, RwLock)
- Most users don't need custom formatting
- Can be added later without breaking changes
- Simple `Display`/`Debug` is sufficient for v1

---

## 14. Implementation Tasks

### Phase 1: Core Types

- [ ] **Task 1.1**: Create `foundation_errstacks` crate skeleton in `backends/foundation_errstacks/`
- [ ] **Task 1.2**: Implement `ErrorTrace<C>` struct with frame storage
- [ ] **Task 1.3**: Implement `Frame`, `FrameImpl`, and `FrameIter` types
- [ ] **Task 1.4**: Implement `ResultExt` trait for `Result<T, E>`
- [ ] **Task 1.5**: Implement `IntoErrorTrace` trait for error conversion

### Phase 2: Formatting & Output

- [ ] **Task 2.1**: Implement `Display` for `ErrorTrace` (basic and alternate)
- [ ] **Task 2.2**: Implement `Debug` for `ErrorTrace` with tree visualization
- [ ] **Task 2.3**: Implement location capture using `core::panic::Location`
- [ ] **Task 2.4**: Add optional backtrace capture (feature-gated)

### Phase 3: Serialization & Integration

- [ ] **Task 3.1**: Implement `Serialize` for `ErrorTrace` (serde feature)
- [ ] **Task 3.2**: Implement `to_structured()` method for JSON output
- [ ] **Task 3.3**: Implement `to_slack_blocks()` helper (slack feature)
- [ ] **Task 3.4**: Add `derive_more` integration examples in documentation

### Phase 4: Testing & Documentation

- [ ] **Task 4.1**: Write unit tests for core types
- [ ] **Task 4.2**: Write integration tests for context changes
- [ ] **Task 4.3**: Add compile-fail tests for type safety
- [ ] **Task 4.4**: Write comprehensive crate-level documentation
- [ ] **Task 4.5**: Add usage examples to `examples/` directory

### Phase 5: Integration

- [ ] **Task 5.1**: Integrate `foundation_errstacks` into `foundation_auth` crate
- [ ] **Task 5.2**: Migrate existing error handling to use `ErrorTrace`
- [ ] **Task 5.3**: Verify Slack alert formatting works end-to-end

---

## 15. Verification Commands

After implementation, verify with:

```bash
# Format check
cargo fmt --package foundation_errstacks -- --check

# Clippy linting (no warnings allowed)
cargo clippy --package foundation_errstacks -- -D warnings

# Run all tests
cargo test --package foundation_errstacks

# Run tests with all features enabled
cargo test --package foundation_errstacks --all-features

# Build documentation
cargo doc --package foundation_errstacks --no-deps

# Check MSRV (Minimum Supported Rust Version: 1.83.0)
cargo +1.83.0 check --package foundation_errstacks
```

---

## 16. Notes for Agents

### Implementation Guidelines

1. **Start with minimal working implementation** - Get `ErrorTrace::new()` and `change_context()` working before adding attachments

2. **Use `Box<[Frame]>` for frame storage** - This matches error-stack's approach and provides good performance for typical error chain depths (3-10 frames)

3. **Location capture** - Use `#[track_caller]` and `core::panic::Location::caller()` for capturing where errors are created

4. **Backtrace handling** - Make backtrace capture optional via `backtrace` feature; use `std::backtrace::Backtrace` when enabled

5. **Thread safety** - All types must be `Send + Sync`; use `PhantomData` appropriately for variance

6. **Error trait** - Target `std::error::Error` for v1; `core::error::Error` can be considered for future no_std support

### Common Pitfalls to Avoid

- Don't use `loop {}` in iterator implementations - use `Option`/`None` to terminate
- Don't capture `Backtrace` by default - it's expensive and should be opt-in
- Don't implement global hooks in v1 - adds unnecessary complexity
- Don't forget `#[track_caller]` on methods that should report caller location

### Testing Priorities

1. **Type safety** - Ensure context type changes are enforced at compile time
2. **Attachment preservation** - Verify attachments survive `change_context()` calls
3. **Serialization round-trip** - Test that `to_structured()` + deserialize preserves data
4. **Output formatting** - Snapshot test Debug output to prevent regressions

---

## 17. Review Checklist

Before implementation begins:

- [ ] API ergonomics validated with sample code
- [ ] Dependency list reviewed for minimality
- [ ] Feature flags cover all optional functionality
- [ ] serde serialization format agreed upon
- [ ] Slack formatting requirements documented
- [ ] Migration guide from error-stack complete
- [ ] Test coverage targets defined

---

## 18. Next Steps

1. Review this specification with the team
2. Iterate based on feedback
3. Create `foundation_errstacks` crate skeleton
4. Implement core types (`ErrorTrace`, `Frame`, `ResultExt`)
5. Add formatting and serialization
6. Write comprehensive tests
7. Document with examples
8. Integrate into `backends/foundation_auth` as first consumer
