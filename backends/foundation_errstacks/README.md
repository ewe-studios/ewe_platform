# foundation_errstacks

[![Crates.io](https://img.shields.io/crates/v/foundation_errstacks.svg)](https://crates.io/crates/foundation_errstacks)
[![Documentation](https://docs.rs/foundation_errstacks/badge.svg)](https://docs.rs/foundation_errstacks)
[![Rust](https://img.shields.io/badge/rust-1.81+-blue.svg)](https://github.com/rust-lang/rust)
[![no_std](https://img.shields.io/badge/no__std-alloc-orange.svg)](https://docs.rust-embedded.org/book/intro/no-std.html)

Minimal, ergonomic, context-aware error traces for Rust. Provides `anyhow`-style ergonomics with compile-time type safety and `no_std` support.

## Features

- **Type-safe contexts**: The current error context is tracked via the type system
- **Stack of frames**: Preserve full history of contexts and attachments as errors propagate
- **`no_std` capable**: Works in embedded environments with just `alloc`
- **Ergonomic API**: Chainable methods for context changes and attachments
- **Serialization**: Optional JSON serialization for logging and telemetry
- **Slack integration**: Built-in Slack Block Kit formatting for alerts
- **`derive_more` friendly**: Designed to work seamlessly with `derive_more` v2

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
foundation_errstacks = "0.0.1"
derive_more = { version = "2", features = ["display", "error"] }
```

Basic usage:

```rust
use foundation_errstacks::{ErrorTrace, PlainResultExt, ErrorTraceResultExt};
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
#[display("database connection failed")]
struct DbError;

#[derive(Debug, Display, Error)]
#[display("user lookup failed")]
struct UserError;

fn connect_to_db() -> Result<Connection, ErrorTrace<DbError>> {
    Err(DbError)
        .attach("host=db.example.com")
        .attach("port=5432")
}

fn get_user(id: u64) -> Result<User, ErrorTrace<UserError>> {
    connect_to_db()
        .change_context(UserError)
        .attach_with(|| format!("user_id={}", id))
}
```

## Core Concepts

### ErrorTrace<C>

`ErrorTrace<C>` is a typed error trace where `C` represents the "current context" - how the error is interpreted at this point in the call stack.

```rust
use foundation_errstacks::ErrorTrace;

// Create a new trace with a context
let trace = ErrorTrace::new(MyError);

// Add attachments (debug info)
let trace = trace.attach("key=value");

// Change context when crossing module boundaries
let trace = trace.change_context(HigherLevelError);
```

### Attachments

Attachments enrich error traces with debugging information:

```rust
// Printable attachment (Display + Debug)
trace.attach("path=/etc/config.toml")

// Opaque attachment (for structured data)
trace.attach_opaque(RequestId(12345))

// Lazy attachment (only evaluated on error path)
trace.attach_with(|| format!("timestamp={}", now()))
```

### Result Extensions

Two extension traits provide ergonomic methods on `Result`:

```rust
use foundation_errstacks::{PlainResultExt, ErrorTraceResultExt};

// For plain Result<T, E>
fn inner() -> Result<(), MyError> { Err(MyError) }

fn outer() -> Result<(), ErrorTrace<MyError>> {
    inner().attach("called from outer()")
}

// For Result<T, ErrorTrace<C>>
fn outer2() -> Result<(), ErrorTrace<HigherError>> {
    outer().change_context(HigherError)
}
```

## Feature Flags

| Feature | Default | Requires | Description |
|---------|---------|----------|-------------|
| `alloc` | yes | - | Baseline; provides `alloc::{Box, String, Vec}` |
| `std` | yes | `alloc` | Enables std-only features |
| `backtrace` | no | `std` | Capture `std::backtrace::Backtrace` |
| `serde` | no | `alloc` | `Serialize` implementations |
| `to_structured` | no | `serde` | `to_structured()` method for structured output |
| `slack` | no | `to_structured` | Slack Block Kit formatting |
| `async` | no | `std` | Async extension traits (future) |

### Minimal (no_std) Usage

```toml
[dependencies]
foundation_errstacks = { version = "0.0.1", default-features = false, features = ["alloc"] }
```

### With Serialization

```toml
[dependencies]
foundation_errstacks = { version = "0.0.1", features = ["serde", "to_structured"] }
```

### With Slack Alerts

```toml
[dependencies]
foundation_errstacks = { version = "0.0.1", features = ["slack"] }
```

## Integration with derive_more

`foundation_errstacks` is designed to work seamlessly with [`derive_more`](https://docs.rs/derive_more/2) for ergonomic error type definitions.

### Basic Error Types

```rust
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
#[display("file not found: {_0}")]
struct FileNotFound(&'static str);

#[derive(Debug, Display, Error)]
#[display("parse error at line {line}: {message}")]
struct ParseError {
    line: u32,
    message: &'static str,
}
```

### Error Enums

```rust
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
enum IoError {
    #[display("read failed: {0}")]
    Read(&'static str),
    #[display("write failed: {0}")]
    Write(&'static str),
    #[display("permission denied: {0}")]
    Permission(&'static str),
}
```

### From Conversions

```rust
use derive_more::{Display, Error, From};

#[derive(Debug, Display, Error)]
#[display("operation failed")]
struct OperationError;

#[derive(Debug, Display, Error, From)]
#[display("high level error")]
struct HighError {
    source: OperationError,
}

// Automatic From impl lets you use ? operator
fn inner() -> Result<(), OperationError> { Err(OperationError) }
fn outer() -> Result<(), HighError> { inner()?; Ok(()) }
```

## Serialization and Structured Output

With the `to_structured` feature:

```rust
use foundation_errstacks::ErrorTrace;

let trace = ErrorTrace::new(MyError)
    .attach("key=value");

let structured = trace.to_structured();

// JSON serialization
let json = structured.to_json()?;

// Access structured fields
println!("Context: {}", structured.current_context);
for frame in &structured.frames {
    println!("  - [{}] {}", frame.kind, frame.message);
}
```

## Slack Integration

With the `slack` feature, format errors for Slack alerts:

```rust
use foundation_errstacks::ErrorTrace;

let trace = ErrorTrace::new(CriticalError)
    .attach("severity=critical")
    .attach("service=api");

let structured = trace.to_structured();
let slack_json = structured.to_slack_json()?;

// POST to Slack webhook
reqwest::blocking::post(SLACK_WEBHOOK_URL)
    .body(slack_json)
    .header("Content-Type", "application/json")
    .send()?;
```

Example Slack output:

```json
{
  "blocks": [
    {"type": "section", "text": {"type": "mrkdwn", "text": "*Error:* Critical error occurred"}},
    {"type": "divider"},
    {"type": "section", "fields": [
      {"type": "mrkdwn", "text": "*Frame 0 (context):*\nCritical error occurred"},
      {"type": "mrkdwn", "text": "*Frame 1 (printable):*\nat severity=critical"}
    ]}
  ]
}
```

## Backtrace Capture

With the `backtrace` feature (requires `std`):

```rust
use foundation_errstacks::ErrorTrace;

let trace = ErrorTrace::new(MyError);

// Full trace display includes backtrace
println!("{:#}", trace);  // Alternate format shows everything
```

## Inspecting Error Traces

```rust
use foundation_errstacks::{ErrorTrace, FrameKind};

let trace = ErrorTrace::new(MyError).attach("debug info");

// Iterate over all frames
for frame in trace.frames() {
    match frame.kind() {
        FrameKind::Context(ctx) => println!("Context: {}", ctx),
        FrameKind::Attachment(kind) => println!("Attachment: {:?}", kind),
    }
}

// Downcast to specific types
if let Some(req_id) = trace.downcast_ref::<RequestId>() {
    println!("Request ID: {:?}", req_id);
}

// Check for specific types
if trace.contains::<MyError>() {
    println!("Trace contains MyError");
}

// Get current context
let ctx = trace.current_context();
```

## Display Formats

```rust
use foundation_errstacks::ErrorTrace;

let trace = ErrorTrace::new(MyError).attach("info");

// Basic format: just current context
println!("{}", trace);  // "my error message"

// Alternate format: full trace with locations
println!("{:#}", trace);
// ErrorTrace:
//   [0] Context: my error message (at src/lib.rs:10:5)
//   [1] Attachment: info (at src/lib.rs:11:10)
```

## Migration Guide

### From anyhow

```rust
// anyhow
use anyhow::{Context, Result};

fn foo() -> Result<()> {
    bar().context("failed to bar")?;
    Ok(())
}

// foundation_errstacks
use foundation_errstacks::{ErrorTrace, PlainResultExt, ErrorTraceResultExt};

#[derive(Debug, derive_more::Display, derive_more::Error)]
#[display("failed to bar")]
struct BarError;

fn foo() -> Result<(), ErrorTrace<BarError>> {
    bar().change_context(BarError).attach("failed to bar")
}
```

### From thiserror + Box<dyn Error>

```rust
// Before
use thiserror::Error;

#[derive(Error, Debug)]
enum AppError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("validation error: {0}")]
    Validation(String),
}

// After - with context preservation
use foundation_errstacks::ErrorTrace;
use derive_more::{Display, Error};

#[derive(Debug, Display, Error)]
#[display("database error")]
struct DbError;

#[derive(Debug, Display, Error)]
#[display("validation error: {0}")]
struct ValidationError(String);

fn query() -> Result<(), ErrorTrace<DbError>> {
    // ... error handling with full context stack
}
```

## MSRV

Requires Rust 1.81+ for `core::error::Error`.

## License

Licensed under the MIT License or Apache License 2.0.
