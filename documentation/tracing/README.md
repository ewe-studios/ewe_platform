# Tracing Documentation - EWE Platform

Comprehensive documentation for using the `tracing` crate in the EWE Platform project.

---

## Overview

The EWE Platform uses the [`tracing`](https://docs.rs/tracing) crate for structured diagnostic data, going far beyond traditional logging. This documentation covers everything from basic usage to production-ready observability patterns.

---

## Documentation Index

### For Getting Started

1. **[Beyond Logging](./beyond-logging.md)** - Start here!
   - Why tracing over traditional logging
   - Core concepts (spans, events, fields)
   - Setting up tracing subscribers
   - The `instrument` macro
   - Context propagation
   - Performance considerations

### For Production Deployment

2. **[Production Setup](./production-setup.md)** - Production configuration
   - Production subscriber setup
   - Log aggregation (Datadog, ELK, etc.)
   - Metrics and alerting integration
   - Distributed tracing with OpenTelemetry
   - Security considerations
   - Troubleshooting guide

### For Practical Examples

3. **[Examples](./examples.md)** - Real-world code samples
   - Basic instrumentation
   - Async patterns
   - Error handling
   - HTTP/WebSocket tracing
   - Database query tracing
   - Testing with tracing

---

## Quick Start

### 1. Add Dependencies

```toml
# Cargo.toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

### 2. Initialize Tracing

```rust
// main.rs
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn main() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    // Your application code
}
```

### 3. Use Instrument Macro

```rust
use tracing::{info, instrument};

#[instrument]
fn process_data(input: &str) -> String {
    info!("Processing data");
    format!("Processed: {}", input)
}
```

### 4. Run with Logging

```bash
RUST_LOG=info cargo run
```

---

## EWE Platform's Trace Crate

The project includes `ewe_trace`, a lightweight abstraction over `tracing`:

```toml
# Cargo.toml
[dependencies]
ewe_trace = { path = "crates/trace" }
```

```rust
use ewe_trace::{info, debug, warn, error};

// Logs only when respective feature is enabled
info!("Info message");
debug!("Debug message");

// Features: log_info, log_debug, log_warnings, log_errors
```

### Feature Flags

| Feature | Enables |
|---------|---------|
| `log_info` | `info!` macros |
| `log_debug` | All macros including `debug!` |
| `log_warnings` | `warn!` macros |
| `log_errors` | `error!` macros |
| `debug_trace` | All logging enabled |

---

## Key Concepts

### Spans vs Events

| Concept | Description | Example |
|---------|-------------|---------|
| **Span** | A period of time with start/end | Wrapping a function, measuring duration |
| **Event** | A point-in-time occurrence | Logging a message, recording a value |

### Levels

```
TRACE < DEBUG < INFO < WARN < ERROR
```

- **TRACE**: Finest granularity, usually disabled in production
- **DEBUG**: Diagnostic information for developers
- **INFO**: General operational information
- **WARN**: Unexpected but handled situations
- **ERROR**: Operation failures

### Structured Fields

```rust
// Add structured data to your logs
info!(
    user_id = %user.id,
    request_id = %request_id,
    duration_ms = elapsed.as_millis(),
    "Request completed"
);
```

---

## Common Patterns

### Function Instrumentation

```rust
#[instrument(skip(self))]
impl Service {
    #[instrument(fields(user_id = %user.id))]
    async fn process(&self, user: User) -> Result<()> {
        // Automatic span creation with user_id field
    }
}
```

### Error Logging

```rust
#[instrument(err)]
fn fallible() -> Result<(), Error> {
    // Automatically logs error on failure
}
```

### Context Propagation

```rust
use tracing::Instrument;

// Propagate span context to spawned task
tokio::spawn(async {
    work().await
}.in_current_span());
```

### Manual Span Creation

```rust
let span = info_span!("operation", id = %uuid::Uuid::new_v4());
let _enter = span.enter();
// Code here is inside the span
```

---

## Environment Configuration

```bash
# Default level
RUST_LOG=info cargo run

# Per-module levels
RUST_LOG=info,ewe_trace=debug,sqlx=warn cargo run

# Full debug for development
RUST_LOG=debug cargo run

# Production with minimal noise
RUST_LOG=info,hyper=warn,reqwest=warn cargo run
```

---

## Integration with EWE Platform

### Foundation Core

```rust
// Executors use tracing for task tracking
use tracing::{instrument, info_span};

#[instrument(skip(task))]
fn execute<T>(task: T) -> Result<Output>
where
    T: TaskIterator,
{
    // Task execution is automatically traced
}
```

### WASM Support

Tracing works seamlessly in WASM targets:

```rust
#[instrument]
fn wasm_operation(data: &[u8]) -> Result<Vec<u8>> {
    // Fully functional in wasm32-unknown-unknown
}
```

---

## Troubleshooting

### No Logs Appearing

```bash
# Ensure RUST_LOG is set
export RUST_LOG=info

# Or set in code
std::env::set_var("RUST_LOG", "info");
```

### Missing Span Context

```rust
// Ensure you enter the span
let span = info_span!("my_span");
span.in_scope(|| {
    // Code here has span context
});

// Or use _enter guard
let _enter = span.enter();
```

### Performance Issues

```rust
// Use enabled! check for expensive operations
if tracing::enabled!(tracing::Level::DEBUG) {
    let data = expensive_computation();
    debug!(?data, "Debug info");
}
```

---

## Resources

### External

- [tracing crate docs](https://docs.rs/tracing)
- [tracing-subscriber docs](https://docs.rs/tracing-subscriber)
- [The tracing Book](https://tokio.rs/tokio/topics/tracing)
- [OpenTelemetry Rust](https://github.com/open-telemetry/opentelemetry-rust)

### Internal

- [ewe_trace crate](../../crates/trace/src/lib.rs)
- [Foundation Core tracing examples](../../backends/foundation_core/src/)

---

## Related Documentation

- [Mise Documentation](../mise/README.md) - Task runner and environment setup
- [Foundation NoStd](../foundation_nostd/doc.md) - Core primitives
- [Foundation Core](../foundation_core/doc.md) - Core functionality
