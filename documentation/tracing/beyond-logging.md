# Tracing in Rust - Beyond Logging

A comprehensive guide to using the `tracing` crate for observability in Rust applications, with a focus on production-ready patterns.

---

## Table of Contents

1. [Introduction](#introduction)
2. [Why Tracing Over Logging?](#why-tracing-over-logging)
3. [Core Concepts](#core-concepts)
4. [Setting Up Tracing](#setting-up-tracing)
5. [Spans and Context](#spans-and-context)
6. [The `instrument` Macro](#the-instrument-macro)
7. [Context Propagation](#context-propagation)
8. [Production Patterns](#production-patterns)
9. [Integrating with Existing Code](#integrating-with-existing-code)
10. [Performance Considerations](#performance-considerations)

---

## Introduction

The `tracing` crate is not just another logging library. It provides **structured diagnostic data** through the concepts of **spans** and **events**, enabling powerful observability patterns that traditional logging cannot match.

This guide assumes you're familiar with basic logging concepts and will show you how to leverage `tracing` for:
- Distributed tracing
- Performance profiling
- Request context propagation
- Structured diagnostics
- Production observability

---

## Why Tracing Over Logging?

### Traditional Logging Limitations

```rust
// Traditional logging - loses context
log::info!("Starting request");
log::info!("Processing user");
log::info!("Query executed");
log::info!("Request complete");
```

Problems:
- No correlation between log lines
- No timing information
- No structured context
- Hard to filter by request/user/operation

### Tracing Approach

```rust
use tracing::{info, instrument, Span};
use tracing::field::{debug, display};

#[instrument(fields(user_id = %user.id, request_id = %request_id))]
async fn process_request(user: User, request_id: String) -> Result<Response> {
    info!("Starting request");  // Automatically includes user_id, request_id

    let result = do_work().await?;  // Spans nest automatically

    info!("Request complete");
    Ok(result)
}
```

Benefits:
- **Context inheritance**: Child spans automatically inherit parent context
- **Timing**: Spans measure duration automatically
- **Structure**: Fields are typed and queryable
- **Correlation**: All events in a span are linked

---

## Core Concepts

### Events vs Spans

| Concept | Description | Use When |
|---------|-------------|----------|
| **Event** | A point in time occurrence | Logging a message, recording a value |
| **Span** | A period of time with start/end | Wrapping an operation, measuring duration |

```rust
use tracing::{info, warn, span, Level};

// Event - happens instantly
info!(bytes_read = 1024, "Read from socket");

// Span - has duration
let db_span = span!(Level::INFO, "database_query", query = "SELECT * FROM users");
db_span.in_scope(|| {
    // This code is "inside" the span
    execute_query()
});
// Span ends when dropped
```

### Levels

```rust
use tracing::Level;

// Trace - finest granularity, usually disabled in production
tracing::trace!("Detailed internal state");

// Debug - diagnostic information for developers
tracing::debug!(?state, "State transition");

// Info - general operational information
tracing::info!(user_id = %user.id, "User logged in");

// Warn - unexpected but handled situations
tracing::warn!(retry_count = 3, "Retrying operation");

// Error - operation failed
tracing::error!(error = %err, "Database connection failed");
```

### Fields and Structured Data

```rust
use tracing::field::{debug, display, Empty};

// Primitive values
info!(count = 42, "Processing items");

// Display trait (uses ToString)
info!(user_id = display(user.id), "User action");

// Debug trait (uses Debug)
info!(config = debug(&config), "Configuration loaded");

// Record fields later
let span = span!(Level::INFO, "request", request_id = Empty);
let request_id = generate_id();
span.record("request_id", &request_id);

// Shorthand syntax
info!(?config);  // Expands to: info!(config = debug(&config))
info!(%user_id); // Expands to: info!(user_id = display(&user_id))
```

---

## Setting Up Tracing

### Basic Setup

```rust
// Cargo.toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

```rust
// main.rs
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

fn init_tracing() {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();
}

fn main() {
    init_tracing();

    // Your application code
}
```

### Environment Variable Control

```bash
# Default level
RUST_LOG=info cargo run

# Per-module levels
RUST_LOG=info,my_crate::module=debug cargo run

# Exclude noisy modules
RUST_LOG=info,hyper=warn,sqlx=warn cargo run

# Field filtering (tracing-subscriber 0.3+)
RUST_LOG=info,my_crate[request_id=12345]=debug cargo run
```

### Production-Ready Subscriber

```rust
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};
use std::sync::Arc;

pub fn init_production_tracing(log_dir: &str) -> anyhow::Result<()> {
    // Rotating file appender
    let file_appender = rolling::daily(log_dir, "application.log");

    // Non-blocking writer for performance
    let (non_blocking_writer, _guard) = non_blocking(file_appender);

    // JSON formatting for log aggregation
    let json_layer = fmt::layer()
        .json()
        .with_writer(non_blocking_writer)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true);

    // Console output for local development
    let console_layer = fmt::layer()
        .pretty()
        .with_ansi(true)
        .with_target(false);

    // Environment-based filtering
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,my_crate=debug"));

    // Combine layers
    let subscriber = Registry::default()
        .with(env_filter)
        .with(console_layer)
        .with(json_layer);

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}
```

---

## Spans and Context

### Creating Spans

```rust
use tracing::{span, Level, info};

// Basic span
let span = span!(Level::INFO, "my_operation");
span.in_scope(|| {
    // Code here is inside the span
    do_work();
});

// Span with fields
let span = span!(
    Level::INFO,
    "database_query",
    table = "users",
    query_type = "select"
);

// Span macros that also record entry/exit
{
    let _guard = span.enter();
    // Span is active until _guard is dropped
    do_work();
}
```

### Span Lifecycle

```rust
use tracing::{info, Span};

// Create span
let span = tracing::info_span!("operation");

// Enter the span (makes it current)
let _enter = span.enter();
info!("This event is inside 'operation' span");

// Span ends when _enter is dropped
drop(_enter);

// Can re-enter later
let _enter2 = span.enter();
info!("Back in the span");

// Or close permanently
span.close();
```

### Nested Spans

```rust
fn process_request() {
    let request_span = info_span!("request", id = %generate_id());
    let _enter = request_span.enter();

    info!("Request started");

    // Child span automatically nested
    let auth_span = info_span!("authenticate");
    auth_span.in_scope(|| {
        authenticate();  // Events here show: request.authenticate
    });

    // Another child span
    let db_span = info_span!("database", query = "select");
    db_span.in_scope(|| {
        query_db();  // Events here show: request.database
    });

    info!("Request completed");
}
```

Output structure:
```
request{id=abc123}
├── request.authenticate
└── request.database{query="select"}
```

### Adding Context to Spans

```rust
use tracing::{info_span, info};
use tracing::field::Empty;

// Define fields at creation, populate later
let span = info_span!(
    "http_request",
    method = Empty,
    path = Empty,
    status = Empty,
    duration_ms = Empty
);

// Record values as they become available
span.record("method", "GET");
span.record("path", "/api/users");

// Can also use display/debug
span.record("path", &tracing::field::display("/api/users"));

span.in_scope(|| {
    // Simulate request
    info!("Processing request");
    span.record("status", 200);
    span.record("duration_ms", 42);
});
```

---

## The `instrument` Macro

The `#[instrument]` attribute automatically wraps a function in a span.

### Basic Usage

```rust
use tracing::{info, instrument};

#[instrument]
fn process_data(input: &str) -> String {
    info!("Processing...");  // Automatically includes function name
    format!("Processed: {}", input)
}
```

### Customizing Instrumentation

```rust
use tracing::{info, instrument, Level};
use tracing::field::{debug, display, Empty};

// Custom name and level
#[instrument(name = "my_custom_name", level = Level::DEBUG)]
fn process(data: &str) {}

// Skip verbose arguments
#[instrument(skip(large_data))]
fn process(large_data: Vec<u8>, user_id: u64) {
    // large_data won't be logged (could be huge)
    // user_id will be logged
}

// Custom field formatting
#[instrument(fields(
    user_id = %user.id,
    email = %user.email,
    config = debug(&config)
))]
fn process_user(user: User, config: Config) {}

// Add fields that aren't function parameters
#[instrument(fields(request_id = Empty))]
async fn handle_request() {
    let request_id = generate_id();
    Span::current().record("request_id", request_id);
    // Now request_id is available to all child spans
}

// Async function support
#[instrument]
async fn fetch_data(id: u64) -> Result<Data, Error> {
    // Works perfectly with async
}

// Methods
#[instrument(skip(self))]
impl Service {
    #[instrument(fields(service = self.name()))]
    async fn process(&self, data: Data) -> Result<()> {
        // self.name() is recorded as a field
    }
}
```

### Return Value Instrumentation

```rust
use tracing::{info, instrument, Level};

// Log return value
#[instrument(ret)]
fn compute(x: i32) -> i32 {
    x * 2
}

// Log at specific level
#[instrument(ret(level = Level::DEBUG))]
fn get_config() -> Config {
    Config::default()
}

// Log error on failure
#[instrument(err)]
fn fallible_operation() -> Result<(), Error> {
    // On error, logs: error=...
    Ok(())
}

// Custom result formatting
#[instrument(
    ret,
    ret.level = Level::DEBUG,
    fields(return_length = result.map(|r| r.len()).unwrap_or(0))
)]
fn fetch_data() -> Result<Vec<u8>, Error> {
    Ok(vec![1, 2, 3])
}
```

---

## Context Propagation

### The Problem

In async/multi-threaded code, spans can be lost when tasks cross thread boundaries.

```rust
// WRONG - span context lost
#[instrument]
async fn handle_request() {
    spawn(async {
        // This span has NO connection to parent!
        nested_function().await
    });
}
```

### Manual Context Propagation

```rust
use tracing::Span;
use tokio::task;

#[instrument]
async fn handle_request() {
    // Capture current span
    let span = Span::current();

    task::spawn(async move {
        // Enter the span in the new task
        let _enter = span.enter();
        nested_function().await
    });
}
```

### Using `in_current_span`

```rust
use tracing::instrument::Instrument;

#[instrument]
async fn handle_request() {
    // Automatically propagates current span
    tokio::spawn(nested_function().in_current_span()).await
}
```

### Propagating Across Services

For distributed tracing, you need to propagate context via headers:

```rust
use tracing_opentelemetry::{OpenTelemetryLayer, OpenTelemetrySpanExt};
use opentelemetry::{global, Context, HeaderMap, HeaderValue};

// Extract context from incoming request
fn extract_context(headers: &HeaderMap) -> Context {
    let extractor = OtelHeaderExtractor(headers);
    global::get_text_map_propagator(|propagator| {
        propagator.extract(&extractor)
    })
}

// Inject context into outgoing request
fn inject_context(headers: &mut HeaderMap) {
    let injector = OtelHeaderInjector(headers);
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&tracing::Span::current().context(), &injector)
    });
}

struct OtelHeaderExtractor<'a>(&'a HeaderMap);
impl<'a> opentelemetry::propagation::Extractor for OtelHeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}
```

---

## Production Patterns

### Pattern 1: Request ID Tracking

```rust
use tracing::{info, instrument, Span};
use tracing::field::Empty;
use uuid::Uuid;

#[derive(Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub user_id: Option<u64>,
    pub start_time: std::time::Instant,
}

impl RequestContext {
    pub fn new(user_id: Option<u64>) -> Self {
        Self {
            request_id: Uuid::new_v4().to_string(),
            user_id,
            start_time: std::time::Instant::now(),
        }
    }

    /// Create a span from this context
    pub fn span(&self) -> tracing::Span {
        info_span!(
            "request",
            request_id = self.request_id.as_str(),
            user_id = self.user_id,
            duration_ms = Empty
        )
    }
}

#[instrument(skip(ctx), fields(ctx.request_id = %ctx.request_id))]
async fn handle_request(ctx: RequestContext) -> Result<Response> {
    let start = std::time::Instant::now();

    // All nested spans inherit request_id
    let user = fetch_user(ctx.user_id).await?;
    let response = process(user).await?;

    // Record duration on the span
    Span::current().record("duration_ms", start.elapsed().as_millis());

    info!(
        request_id = %ctx.request_id,
        duration_ms = start.elapsed().as_millis(),
        "Request completed"
    );

    Ok(response)
}
```

### Pattern 2: Error Context

```rust
use tracing::{error, instrument, field};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("External service error: {service} - {source}")]
    ExternalService {
        service: String,
        #[source]
        source: reqwest::Error,
    },
}

impl AppError {
    /// Add tracing context to the error
    pub fn with_context(self, ctx: &str) -> Self {
        tracing::error!(
            error = %self,
            context = ctx,
            "Operation failed"
        );
        self
    }
}

#[instrument(err, fields(error.context = Empty))]
async fn fetch_user_data(user_id: u64) -> Result<UserData, AppError> {
    let result = sqlx::query_as!("SELECT * FROM users WHERE id = ?", user_id)
        .fetch_one(&pool)
        .await
        .map_err(AppError::from);

    // Add context to error if it occurs
    let data = result.map_err(|e| {
        tracing::Span::current().record("error.context", "database_lookup");
        e
    })?;

    Ok(data)
}
```

### Pattern 3: Performance Measurement

```rust
use tracing::{info, instrument, Span};
use std::time::{Duration, Instant};

/// Guard that records timing when dropped
pub struct TimingGuard {
    span: Span,
    operation: &'static str,
    start: Instant,
}

impl TimingGuard {
    pub fn new(span: Span, operation: &'static str) -> Self {
        Self {
            span,
            operation,
            start: Instant::now(),
        }
    }
}

impl Drop for TimingGuard {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        self.span.record("duration_ms", elapsed.as_millis() as i64);
        self.span.record("operation", self.operation);

        if elapsed > Duration::from_millis(1000) {
            tracing::warn!(
                operation = self.operation,
                duration_ms = elapsed.as_millis(),
                "Slow operation detected"
            );
        }
    }
}

#[instrument]
async fn complex_operation() -> Result<()> {
    let _timing = TimingGuard::new(Span::current(), "complex_operation");

    // Do work...
    // Duration automatically recorded on drop
}
```

### Pattern 4: Structured Error Reporting

```rust
use tracing::{error, field};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct ErrorContext {
    pub operation: &'static str,
    pub component: &'static str,
    pub user_id: Option<u64>,
    pub request_id: String,
}

pub trait TracedResult<T, E> {
    fn trace_error(self, ctx: ErrorContext) -> Result<T, E>;
}

impl<T, E> TracedResult<T, E> for Result<T, E>
where
    E: std::fmt::Display + std::fmt::Debug,
{
    fn trace_error(self, ctx: ErrorContext) -> Result<T, E> {
        self.map_err(|err| {
            error!(
                error = %err,
                error.debug = ?err,
                operation = ctx.operation,
                component = ctx.component,
                user_id = ctx.user_id,
                request_id = ctx.request_id,
                "Operation failed"
            );
            err
        })
    }
}

// Usage
async fn process_payment(user_id: u64, amount: Decimal) -> Result<()> {
    let ctx = ErrorContext {
        operation: "process_payment",
        component: "payment_service",
        user_id: Some(user_id),
        request_id: generate_request_id(),
    };

    charge_card(amount)
        .await
        .trace_error(ctx)
}
```

### Pattern 5: Conditional Logging Based on Environment

```rust
use tracing::{info, debug, instrument};

/// Log verbose details only in debug builds
#[cfg(debug_assertions)]
macro_rules! debug_verbose {
    ($($t:tt)*) => { tracing::debug!($($t)*) };
}

#[cfg(not(debug_assertions))]
macro_rules! debug_verbose {
    ($($t:tt)*) => {};
}

#[instrument]
async fn process_batch(items: Vec<Item>) -> Result<()> {
    // Always logged
    info!(count = items.len(), "Processing batch");

    // Only in debug builds
    debug_verbose!(?items, "Batch contents");

    for item in items {
        debug_verbose!(?item, "Processing item");
        process_item(item).await?;
    }

    info!("Batch complete");
    Ok(())
}
```

---

## Integrating with Existing Code

### Migrating from `log` Crate

The `tracing` crate is compatible with `log` macros:

```rust
// Cargo.toml
[dependencies]
tracing = "0.1"
tracing-log = "0.2"

// Setup
use tracing_log::LogTracer;

fn init() {
    LogTracer::init().expect("Failed to set logger");
    // Now log::info!() events appear as tracing events
}
```

### Layering for Different Outputs

```rust
use tracing_subscriber::{layer::SubscriberExt, Registry, filter};

// Send errors to stderr
let error_layer = tracing_subscriber::fmt::layer()
    .with_writer(std::io::stderr)
    .with_filter(filter::LevelFilter::ERROR);

// Send info+ to stdout
let info_layer = tracing_subscriber::fmt::layer()
    .with_writer(std::io::stdout)
    .with_filter(filter::LevelFilter::INFO);

// Send debug to file
let file_appender = tracing_appender::rolling::daily("/var/log", "debug.log");
let debug_layer = tracing_subscriber::fmt::layer()
    .json()
    .with_writer(file_appender)
    .with_filter(filter::LevelFilter::DEBUG);

let subscriber = Registry::default()
    .with(error_layer)
    .with(info_layer)
    .with(debug_layer);

tracing::subscriber::set_global_default(subscriber)?;
```

### Testing with Tracing

```rust
// Cargo.toml
[dev-dependencies]
tracing-test = "0.2"

// Test code
use tracing_test::traced_test;

#[test]
#[traced_test]
fn test_with_logging() {
    info!("This log appears in test output");
    assert!(logs_contain("This log appears"));
    assert!(logs_contains("info"));
}

// Or capture traces for assertions
use tracing_subscriber::{layer::SubscriberExt, Registry};
use tracing_subscriber::layer::Layer;
use tracing::subscriber::with_default;

#[derive(Default)]
struct CaptureLayer {
    events: Mutex<Vec<String>>,
}

impl<S: tracing::Subscriber> Layer<S> for CaptureLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing::subscribe::Context<'_>) {
        // Capture event for assertion
    }
}

#[test]
fn test_captures_traces() {
    let capture = CaptureLayer::default();
    let subscriber = Registry::default().with(capture);

    with_default(subscriber, || {
        // Your test code
        info!("Test event");
    });

    // Assert on captured traces
    assert!(capture.events.contains(&"Test event".to_string()));
}
```

---

## Performance Considerations

### Enabled/Disabled Checks

```rust
// Expensive operations only when tracing is enabled
if tracing::enabled!(Level::DEBUG) {
    let debug_data = expensive_debug_computation();
    debug!(?debug_data, "Debug info");
}

// Or use lazy evaluation
debug!(lazy = || expensive_debug_computation(), "Debug info");
```

### Field Evaluation

```rust
// DON'T - always evaluated even if disabled
debug!(data = format!("{:?}", expensive_debug_data()));

// DO - only evaluated if enabled
if tracing::enabled!(Level::DEBUG) {
    debug!(data = ?expensive_debug_data());
}

// Or use tracing's lazy evaluation
debug!(data = tracing::field::debug(&expensive_debug_data()));
```

### Span Overhead

```rust
// Minimal overhead - fields are only formatted if enabled
#[instrument]
fn minimal_overhead(x: i32) {}

// Higher overhead - formats all fields
#[instrument(fields(data = format!("{:?}", expensive_data)))]
fn higher_overhead() {}

// Use skip for large data
#[instrument(skip(large_data))]
fn process(large_data: Vec<u8>) {}
```

### Async Instrumentation

```rust
// Instrument async functions normally
#[instrument]
async fn handle() {}

// For spawned tasks, use in_current_span
tokio::spawn(async {
    work().await
}.instrument(tracing::info_span!("spawned_task")));
```

---

## Quick Reference

### Common Patterns

```rust
// Simple logging
info!("Message");
info!(key = value, "Message with field");
info!(?value, "Message with debug");
info!(%value, "Message with display");

// Create span
let span = info_span!("name", field = value);
let _enter = span.enter();

// Instrument function
#[instrument(skip(self))]
fn method(&self, arg: i32) {}

#[instrument(ret, err)]
async fn async_fn() -> Result<T, E> {}

// Record field on span
span.record("field", &value);
Span::current().record("field", &value);

// Check if enabled
if tracing::enabled!(Level::DEBUG) {
    // expensive operation
}
```

### Subscriber Setup Template

```rust
use tracing_subscriber::{fmt, prelude::*, EnvFilter, Registry};

fn init_tracing() {
    let subscriber = Registry::default()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().pretty());

    tracing::subscriber::set_global_default(subscriber)
        .expect("Failed to set subscriber");
}
```

---

## Resources

- [tracing crate documentation](https://docs.rs/tracing)
- [tracing-subscriber documentation](https://docs.rs/tracing-subscriber)
- [The `tracing` Book](https://tokio.rs/tokio/topics/tracing)
- [OpenTelemetry Rust](https://github.com/open-telemetry/opentelemetry-rust)
- [EWE Platform trace crate](../crates/trace/Cargo.toml)
