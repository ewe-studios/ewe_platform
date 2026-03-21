# Tracing Examples - EWE Platform

Practical examples of using `tracing` in real-world scenarios within the EWE Platform codebase.

---

## Table of Contents

1. [Basic Examples](#basic-examples)
2. [Async Examples](#async-examples)
3. [Error Handling](#error-handling)
4. [HTTP/WebSocket Examples](#httpwebsocket-examples)
5. [Database Examples](#database-examples)
6. [Testing Examples](#testing-examples)

---

## Basic Examples

### Simple Function Instrumentation

```rust
use tracing::{info, instrument, debug};

/// Basic instrumentation - automatic span creation
#[instrument]
fn process_data(input: &str) -> String {
    info!("Processing data");
    format!("Processed: {}", input)
}

// Usage
let result = process_data("hello");
// Output shows:
// NEW process_data
// Processing data
// CLOSE process_data
```

### Custom Span Fields

```rust
use tracing::{instrument, Level, info};
use tracing::field::{debug, display};

#[instrument(
    name = "user_operation",
    level = Level::INFO,
    fields(
        user_id = %user.id,
        username = %user.name,
        operation = "create"
    )
)]
fn create_user(user: &User) -> Result<UserId> {
    info!("Creating user");
    // user_id and username automatically included in all child spans
    Ok(UserId::new())
}
```

### Conditional Logging

```rust
use tracing::{debug, info, enabled, Level};

fn expensive_operation(data: &[u8]) {
    // Only format debug info if debug level is enabled
    if enabled!(Level::DEBUG) {
        debug!(data_len = data.len(), first_byte = data[0], "Processing data");
    }

    info!("Operation complete");
}
```

---

## Async Examples

### Async Function Instrumentation

```rust
use tracing::{instrument, info};

#[instrument]
async fn fetch_user_data(user_id: u64) -> Result<UserData> {
    info!("Fetching user data");

    // Simulate async operation
    tokio::time::sleep(Duration::from_millis(100)).await;

    Ok(UserData { id: user_id })
}
```

### Spawning Tasks with Context

```rust
use tracing::{info, Instrument, info_span};
use tokio::task;

#[instrument]
async fn handle_request(request: Request) {
    let request_id = generate_id();

    // Spawn background task with span context
    let span = info_span!("background_task", %request_id);

    task::spawn(
        async move {
            info!("Processing in background");
            process_background(request).await;
        }
        .instrument(span)
    );

    info!("Request handled, background task spawned");
}
```

### Parallel Processing with Spans

```rust
use tracing::{instrument, info_span, Instrument};
use futures::future::join_all;

#[instrument]
async fn process_batch(items: Vec<Item>) -> Vec<Result<Output>> {
    let futures = items.into_iter().map(|item| {
        let span = info_span!("process_item", item_id = %item.id);
        async move {
            process_single(item).await
        }
        .instrument(span)
    });

    join_all(futures).await
}
```

---

## Error Handling

### Instrument with Error Logging

```rust
use tracing::{instrument, error};
use thiserror::Error;

#[derive(Error, Debug)]
enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
}

// Automatically log errors
#[instrument(err)]
fn fallible_operation() -> Result<(), AppError> {
    // On error, automatically logs: error=...
    Err(AppError::NotFound("item".to_string()))
}

// Custom error logging
#[instrument]
async fn fetch_with_context(id: u64) -> Result<Data, AppError> {
    match fetch_data(id).await {
        Ok(data) => Ok(data),
        Err(e) => {
            error!(
                error = %e,
                item_id = id,
                error_type = std::any::type_name_of_val(&e),
                "Failed to fetch item"
            );
            Err(e)
        }
    }
}
```

### Error Context Chain

```rust
use tracing::{error, warn, info, Span};

pub trait WithTracingContext<T, E> {
    fn with_context(self, operation: &'static str) -> Result<T, E>;
    fn with_warn_context(self, operation: &'static str) -> Result<T, E>;
}

impl<T, E> WithTracingContext<T, E> for Result<T, E>
where
    E: std::fmt::Display + std::fmt::Debug,
{
    fn with_context(self, operation: &'static str) -> Result<T, E> {
        self.map_err(|err| {
            error!(
                error = %err,
                error.debug = ?err,
                operation,
                "Operation failed"
            );
            err
        })
    }

    fn with_warn_context(self, operation: &'static str) -> Result<T, E> {
        self.map_err(|err| {
            warn!(
                error = %err,
                operation,
                "Operation completed with warning"
            );
            err
        })
    }
}

// Usage
async fn process_pipeline() -> Result<()> {
    fetch_data()
        .await
        .with_context("fetch_data")
        .and_then(|data| {
            transform(data).with_context("transform")
        })
        .and_then(|result| {
            save(result).with_context("save")
        })
}
```

---

## HTTP/WebSocket Examples

### HTTP Request Tracing

```rust
use tracing::{instrument, info, warn};
use tracing::field::{debug, display};

#[instrument(
    skip(request, bytes),
    fields(
        method = %request.method(),
        uri = %request.uri(),
        content_length = request.headers().get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok()),
        request_id = %uuid::Uuid::new_v4()
    )
)]
async fn handle_http_request(
    request: hyper::Request<hyper::Body>,
    bytes: Vec<u8>,
) -> Result<hyper::Response<hyper::Body>> {
    info!("Handling HTTP request");

    // Process request...

    let response = hyper::Response::builder()
        .status(200)
        .body(hyper::Body::from("OK"))?;

    Ok(response)
}
```

### WebSocket Connection Tracing

```rust
use tracing::{instrument, info, warn, error, Span};
use tokio::sync::mpsc;

#[instrument(
    skip(stream, tx),
    fields(
        peer_addr = %stream.peer_addr().unwrap_or_else(|_| "unknown".parse().unwrap()),
        connection_id = %uuid::Uuid::new_v4()
    )
)]
async fn handle_websocket<S>(
    stream: S,
    tx: mpsc::Sender<Message>,
) -> Result<()>
where
    S: WebSocketStream + Send + 'static,
{
    let (mut write, mut read) = stream.split();
    let connection_id = Span::current()
        .record("connection_id")
        .unwrap_or_else(|| "unknown".to_string());

    info!(connection_id = %connection_id, "WebSocket connection established");

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                info!(%text, "Received text message");
                tx.send(Message::Text(text)).await?;
            }
            Ok(Message::Binary(data)) => {
                info!(data_len = data.len(), "Received binary message");
            }
            Ok(Message::Close(frame)) => {
                info!(?frame, "Received close frame");
                break;
            }
            Ok(Message::Ping(data)) => {
                debug!(data_len = data.len(), "Received ping");
                write.send(Message::Pong(data)).await?;
            }
            Err(e) => {
                error!(error = %e, "WebSocket error");
                break;
            }
            _ => {}
        }
    }

    info!("WebSocket connection closed");
    Ok(())
}
```

### SSE (Server-Sent Events) Tracing

```rust
use tracing::{instrument, info, debug};
use futures::stream::Stream;

#[instrument(skip(stream), fields(client_id = %client_id))]
async fn stream_events<S>(
    client_id: String,
    mut stream: S,
) -> Result<Vec<Event>>
where
    S: Stream<Item = Result<Event>> + Unpin,
{
    let mut events = Vec::new();
    let mut count = 0;

    while let Some(event) = stream.next().await {
        count += 1;

        match event {
            Ok(e) => {
                debug!(event_type = %e.event, data_len = e.data.len(), "Received event");
                events.push(e);
            }
            Err(e) => {
                warn!(error = %e, count, "Stream error");
                break;
            }
        }
    }

    info!(count, "Stream completed");
    Ok(events)
}
```

---

## Database Examples

### SQL Query Tracing

```rust
use tracing::{instrument, debug, info};
use sqlx::{PgPool, FromRow};

#[derive(FromRow, Debug)]
struct User {
    id: i64,
    name: String,
    email: String,
}

#[instrument(
    skip(pool),
    fields(
        query = "SELECT * FROM users WHERE id = $1",
        user_id = id
    )
)]
async fn get_user_by_id(pool: &PgPool, id: i64) -> Result<User> {
    let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await?;

    debug!(?user, "User fetched");
    Ok(user)
}

#[instrument(skip(pool, users))]
async fn bulk_insert_users(pool: &PgPool, users: Vec<User>) -> Result<u64> {
    let count = users.len();
    info!(count, "Inserting users");

    let mut tx = pool.begin().await?;

    for user in &users {
        sqlx::query("INSERT INTO users (name, email) VALUES ($1, $2)")
            .bind(&user.name)
            .bind(&user.email)
            .execute(&mut *tx)
            .await?;
    }

    tx.commit().await?;
    info!(inserted = count, "Users inserted");
    Ok(count as u64)
}
```

### Transaction Tracing

```rust
use tracing::{instrument, info, Span};

#[instrument(skip(pool), fields(transaction_id = Empty))]
async fn execute_transaction(pool: &PgPool, operations: Vec<Operation>) -> Result<()> {
    let tx_id = uuid::Uuid::new_v4();
    Span::current().record("transaction_id", &tx_id.to_string());

    info!("Starting transaction");

    let mut tx = pool.begin().await?;

    for (i, op) in operations.iter().enumerate() {
        let op_span = info_span!("transaction_operation", index = i, operation = %op.name());

        op_span.in_scope(|| {
            info!("Executing operation");
        });

        op.execute(&mut tx).await?;
    }

    tx.commit().await?;
    info!("Transaction committed");

    Ok(())
}
```

---

## Testing Examples

### Unit Tests with Tracing

```rust
use tracing_test::traced_test;

#[test]
#[traced_test]
fn test_with_logging() {
    info!("This log appears in test output");

    let result = process_data("test");

    assert_eq!(result, "Processed: test");

    // Assert on log output
    assert!(logs_contain("Processing data"));
}

#[tokio::test]
#[traced_test]
async fn test_async_with_logging() {
    info!("Starting async test");

    let result = fetch_data(123).await;

    assert!(result.is_ok());
    assert!(logs_contain("Fetching"));
}
```

### Integration Test with Capture

```rust
use tracing_subscriber::{layer::SubscriberExt, Registry};
use tracing::subscriber::with_default;
use std::sync::{Arc, Mutex};

#[derive(Default, Clone)]
struct LogCapture {
    logs: Arc<Mutex<Vec<String>>>,
}

impl<S: tracing::Subscriber> tracing_subscriber::layer::Layer<S> for LogCapture {
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_>) {
        let mut logs = self.logs.lock().unwrap();
        logs.push(format!("{:?}", event));
    }
}

#[test]
fn test_captures_logs() {
    let capture = LogCapture::default();
    let subscriber = Registry::default().with(capture.clone());

    with_default(subscriber, || {
        info!("Test message");
        process_data("hello");
    });

    let logs = capture.logs.lock().unwrap();
    assert!(logs.iter().any(|l| l.contains("Test message")));
    assert!(logs.iter().any(|l| l.contains("Processing")));
}
```

---

## EWE Platform Specific Examples

### Using ewe_trace Macros

```rust
// Using the project's tracing abstraction
use ewe_trace::{info, debug, warn, error};

#[cfg(feature = "log_info")]
fn info_logging() {
    info!("This appears when log_info feature is enabled");
}

#[cfg(feature = "log_debug")]
fn debug_logging() {
    debug!("This appears when log_debug feature is enabled");
}

// In release builds without features, these are no-ops
#[cfg(not(feature = "log_debug"))]
fn no_debug_in_release() {
    debug!("This is compiled out in release");
}
```

### Foundation Core Executor Tracing

```rust
// Example from foundation_core executors
use tracing::{instrument, debug, info_span};

#[instrument(skip(task), fields(task_id = Empty))]
fn execute_task<T>(task: T) -> Result<Output>
where
    T: TaskIterator,
{
    let task_id = generate_task_id();
    tracing::Span::current().record("task_id", &task_id);

    let span = info_span!("task_execution", %task_id);
    span.in_scope(|| {
        debug!("Starting task execution");

        let result = task.run();

        debug!("Task execution complete");
        result
    })
}
```

### WASM-Compatible Tracing

```rust
// Tracing that works in both WASM and native
use tracing::{instrument, info};

#[instrument]
fn wasm_safe_operation(data: &[u8]) -> Result<Vec<u8>> {
    // All tracing works normally in WASM
    info!(data_len = data.len(), "Processing");

    // Result automatically logged on return
    Ok(data.to_vec())
}
```

---

## Quick Reference Card

```rust
// ===== BASIC USAGE =====
info!("Message");
debug!("Debug: {}", value);
warn!("Warning: {}", value);
error!("Error: {}", error);

// ===== FIELDS =====
info!(key = value, "Message");
info!(?value, "Debug format");
info!(%value, "Display format");
info!(field1 = a, field2 = b, "Message");

// ===== SPANS =====
let span = info_span!("name", field = value);
let _enter = span.enter();
span.record("field", &new_value);

// ===== INSTRUMENT =====
#[instrument]
fn my_function(arg: i32) {}

#[instrument(skip(self))]
impl MyStruct {
    #[instrument(fields(custom = self.value))]
    fn method(&self) {}
}

#[instrument(ret, err)]
async fn async_fn() -> Result<T, E> {}

// ===== ASYNC =====
tokio::spawn(async { work() }.in_current_span());

// ===== TESTING =====
#[test]
#[traced_test]
fn test_logs() {
    assert!(logs_contain("expected text"));
}
```

---

## Related Documentation

- [Beyond Logging](./beyond-logging.md) - Core concepts and patterns
- [Production Setup](./production-setup.md) - Production configuration
- [Tracing Crate](../crates/trace/Cargo.toml) - EWE Platform's trace crate
