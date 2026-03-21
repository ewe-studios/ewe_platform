# Production Tracing Setup Guide

This guide covers setting up `tracing` for production environments with proper log aggregation, performance monitoring, and debugging capabilities.

---

## Table of Contents

1. [Production Subscriber Setup](#production-subscriber-setup)
2. [Log Aggregation Integration](#log-aggregation-integration)
3. [Metrics and Alerting](#metrics-and-alerting)
4. [Distributed Tracing](#distributed-tracing)
5. [Security Considerations](#security-considerations)
6. [Troubleshooting](#troubleshooting)

---

## Production Subscriber Setup

### Complete Production Configuration

```rust
// src/tracing/production.rs
use anyhow::Context;
use tracing_appender::{non_blocking, rolling};
use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    prelude::*,
    registry,
    EnvFilter,
};
use std::sync::Arc;

pub struct ProductionTracingConfig {
    pub service_name: String,
    pub log_directory: String,
    pub max_log_files: usize,
    pub json_logs: bool,
}

impl Default for ProductionTracingConfig {
    fn default() -> Self {
        Self {
            service_name: "application".to_string(),
            log_directory: "/var/log/application".to_string(),
            max_log_files: 7,
            json_logs: true,
        }
    }
}

pub fn init_production_tracing(config: ProductionTracingConfig) -> anyhow::Result<TracingGuard> {
    // Ensure log directory exists
    std::fs::create_dir_all(&config.log_directory)
        .context("Failed to create log directory")?;

    // Rotating file appender (daily rotation)
    let file_appender = rolling::daily(&config.log_directory, "application.log");

    // Non-blocking writer with overflow handling
    let (non_blocking, guard) = non_blocking(file_appender);

    // JSON layer for log aggregation
    let json_layer = fmt::layer()
        .json()
        .with_writer(non_blocking)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .with_current_span(true)
        .with_span_list(true);

    // Optional console output for debugging
    let console_layer = std::env::var("ENABLE_CONSOLE_LOGS")
        .ok()
        .filter(|v| v == "true" || v == "1")
        .map(|_| {
            fmt::layer()
                .pretty()
                .with_ansi(true)
                .with_target(false)
                .with_thread_ids(false)
                .with_thread_names(false)
        });

    // Environment-based filtering with sensible defaults
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            EnvFilter::new(format!(
                "info,{}=debug,hyper=warn,reqwest=warn,sqlx=warn,tower=warn",
                config.service_name
            ))
        });

    // Build subscriber
    let subscriber = registry::Registry::default()
        .with(env_filter)
        .with(json_layer);

    let subscriber = if let Some(console) = console_layer {
        subscriber.with(console).boxed()
    } else {
        subscriber.boxed()
    };

    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set global tracing subscriber")?;

    // Return guard to keep non_blocking writer alive
    Ok(TracingGuard { _guard: guard })
}

/// Guard that keeps tracing resources alive
pub struct TracingGuard {
    _guard: tracing_appender::non_blocking::WorkerGuard,
}

impl Drop for TracingGuard {
    fn drop(&mut self) {
        // Ensure all logs are flushed
        tracing::info!("Tracing guard dropped, flushing logs");
    }
}
```

### Usage in Application

```rust
// src/main.rs
mod tracing;

use anyhow::Context;
use tracing::{error, info};

fn main() -> anyhow::Result<()> {
    // Initialize production tracing
    let config = tracing::ProductionTracingConfig {
        service_name: "my_service".to_string(),
        log_directory: std::env::var("LOG_DIR")
            .unwrap_or_else(|_| "/var/log/my_service".to_string()),
        ..Default::default()
    };

    let _guard = tracing::init_production_tracing(config)?;

    info!(
        service = "my_service",
        version = env!("CARGO_PKG_VERSION"),
        "Service starting"
    );

    // Your application code
    if let Err(e) = run_application() {
        error!(error = %e, "Application failed");
        return Err(e);
    }

    info!("Service shutdown complete");
    Ok(())
}
```

---

## Log Aggregation Integration

### Structured Logging for Logstash/Fluentd

```rust
use serde::Serialize;
use tracing::field::Visit;
use tracing_subscriber::fmt::{format, JsonStorageFields};

#[derive(Serialize)]
pub struct LogEntry {
    pub timestamp: String,
    pub level: String,
    pub service: String,
    pub message: String,
    pub span: Option<SpanContext>,
    pub fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Serialize)]
pub struct SpanContext {
    pub trace_id: String,
    pub span_id: String,
    pub parent_span_id: Option<String>,
}

// Use with tracing-subscriber's JSON formatter
let subscriber = Registry::default()
    .with(EnvFilter::from_default_env())
    .with(
        fmt::layer()
            .json()
            .with_writer(std::io::stdout)
            .with_current_span(true)
            .with_span_list(true)
    );
```

### Datadog Integration

```rust
use tracing_subscriber::{fmt, layer::SubscriberExt, Registry, EnvFilter};

// Datadog expects specific field names
let datadog_layer = fmt::layer()
    .json()
    .with_writer(std::io::stdout)
    .flatten_event(true)
    .with_current_span(true)
    .with_span_list(true);

// Add Datadog-specific fields via a custom layer
use tracing_core::{Event, Subscriber};
use tracing_subscriber::layer::{Context, Layer};

pub struct DatadogLayer {
    service: String,
    version: String,
    environment: String,
}

impl<S: Subscriber> Layer<S> for DatadogLayer {
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_>) {
        // Datadog expects: service, version, env
        // These can be added as additional fields
    }
}

let subscriber = Registry::default()
    .with(EnvFilter::from_default_env())
    .with(DatadogLayer {
        service: std::env::var("DD_SERVICE").unwrap_or_else(|_| "app".to_string()),
        version: std::env::var("DD_VERSION").unwrap_or_else(|_| "unknown".to_string()),
        environment: std::env::var("DD_ENV").unwrap_or_else(|_| "production".to_string()),
    })
    .with(datadog_layer);
```

### ELK Stack (Elasticsearch, Logstash, Kibana)

```rust
// For ECS (Elastic Common Schema) compatibility
use tracing_subscriber::fmt::format::JsonFields;

let ecs_layer = fmt::layer()
    .json()
    .with_writer(std::io::stdout)
    .with_target(false)  // ECS uses different field structure
    .with_thread_ids(true)
    .with_thread_names(true);

// Add ECS-compatible fields
use tracing::field::{debug, display};

info!(
    ecs.version = "1.6.0",
    service.name = "my-service",
    event.action = "request_processed",
    http.request.method = "GET",
    http.response.status_code = 200,
    "Request processed"
);
```

---

## Metrics and Alerting

### Integrating with Prometheus

```rust
use std::sync::Arc;
use metrics_exporter_prometheus::PrometheusBuilder;
use tracing::info;
use tracing_subscriber::layer::SubscriberExt;

// Set up metrics
let recorder = PrometheusBuilder::new()
    .set_quantiles(&[0.0, 0.5, 0.9, 0.95, 0.99, 1.0])
    .unwrap()
    .build();

metrics::set_global_recorder(recorder).unwrap();

// Create histogram for span durations
let duration_histogram = metrics::histogram!("span_duration_seconds");

// Custom layer to record metrics
use tracing_subscriber::layer::{Context, Layer};
use tracing::{span, Id, Subscriber};
use std::time::Instant;
use dashmap::DashMap;

pub struct MetricsLayer {
    span_starts: Arc<DashMap<Id, Instant>>,
}

impl<S: Subscriber> Layer<S> for MetricsLayer {
    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &Id, ctx: Context<'_>) {
        self.span_starts.insert(id.clone(), Instant::now());
    }

    fn on_close(&self, id: Id, ctx: Context<'_>) {
        if let Some(start) = self.span_starts.remove(&id) {
            let duration = start.1.elapsed();
            duration_histogram.record(duration.as_secs_f64());
        }
    }
}
```

### Alerting on Error Rates

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

pub struct ErrorRateTracker {
    error_count: AtomicU64,
    window_start: Instant,
    window_duration: Duration,
    threshold: u64,
}

impl ErrorRateTracker {
    pub fn new(window_duration: Duration, threshold: u64) -> Self {
        Self {
            error_count: AtomicU64::new(0),
            window_start: Instant::now(),
            window_duration,
            threshold,
        }
    }

    pub fn record_error(&self) {
        let count = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;

        // Check if we exceeded threshold
        if count >= self.threshold {
            let elapsed = self.window_start.elapsed();
            if elapsed < self.window_duration {
                // ALERT: Error rate exceeded!
                tracing::error!(
                    error_rate_alert = true,
                    errors = count,
                    window_seconds = elapsed.as_secs(),
                    threshold = self.threshold,
                    "Error rate exceeded threshold!"
                );
            }
        }

        // Reset window if expired
        if self.window_start.elapsed() > self.window_duration {
            self.error_count.store(0, Ordering::Relaxed);
            self.window_start = Instant::now();
        }
    }
}

// Use in error handling
lazy_static! {
    static ref ERROR_TRACKER: ErrorRateTracker =
        ErrorRateTracker::new(Duration::from_secs(60), 100);
}

fn handle_request() -> Result<()> {
    process().map_err(|e| {
        ERROR_TRACKER.record_error();
        e
    })
}
```

---

## Distributed Tracing

### OpenTelemetry Setup

```rust
use opentelemetry::{global, sdk::trace as sdktrace, Context, KeyValue};
use opentelemetry_otlp::{OtlpTracePipeline, WithExportConfig};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, Registry};

pub fn init_otel_tracing(service_name: &str) -> anyhow::Result<()> {
    // Create OTLP pipeline
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(
            opentelemetry_otlp::new_exporter()
                .tonic()
                .with_endpoint("http://localhost:4317")
        )
        .with_trace_config(
            sdktrace::config()
                .with_sampler(sdktrace::Sampler::AlwaysOn)
                .with_resource(opentelemetry::sdk::Resource::new(vec![
                    KeyValue::new("service.name", service_name),
                    KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
                ]))
        )
        .install_batch(opentelemetry::runtime::Tokio)?;

    let otel_layer = OpenTelemetryLayer::new(tracer);

    let subscriber = Registry::default()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(otel_layer);

    tracing::subscriber::set_global_default(subscriber)?;

    Ok(())
}

// Don't forget to shutdown on exit
pub fn shutdown() {
    global::shutdown_tracer_provider();
}
```

### Propagating Context in HTTP Requests

```rust
use tracing_opentelemetry::OpenTelemetrySpanExt;
use reqwest::header::HeaderMap;

// Extract from incoming request
fn extract_trace_context(headers: &HeaderMap) -> Context {
    let extractor = HeaderExtractor(headers);
    global::get_text_map_propagator(|propagator| {
        propagator.extract(&extractor)
    })
}

// Inject into outgoing request
async fn make_request(client: &reqwest::Client, url: &str) -> Result<String> {
    let mut req = client.get(url);

    // Inject current span context into headers
    let context = Span::current().context();
    global::get_text_map_propagator(|propagator| {
        propagator.inject_context(&context, &mut HeaderInjector(&mut req.headers_mut()?));
    });

    let response = req.send().await?;
    Ok(response.text().await?)
}

struct HeaderExtractor<'a>(&'a HeaderMap);
impl<'a> opentelemetry::propagation::Extractor for HeaderExtractor<'a> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|v| v.to_str().ok())
    }
    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|k| k.as_str()).collect()
    }
}

struct HeaderInjector<'a>(&'a mut HeaderMap);
impl<'a> opentelemetry::propagation::Injector for HeaderInjector<'a> {
    fn set(&mut self, key: &str, value: String) {
        if let Ok(v) = value.parse() {
            self.0.insert(key, v);
        }
    }
}
```

### W3C Trace Context Support

```rust
// Cargo.toml
[dependencies]
opentelemetry = { version = "0.21", features = ["trace"] }
opentelemetry-http = "0.10"

// The default propagator supports W3C trace context
use opentelemetry::global;
use opentelemetry::sdk::propagation::TraceContextPropagator;

fn init() {
    global::set_text_map_propagator(TraceContextPropagator::new());
}

// Trace context headers are automatically handled:
// - traceparent: 00-{trace-id}-{parent-id}-{trace-flags}
// - tracestate: vendor-specific trace info
```

---

## Security Considerations

### Filtering Sensitive Data

```rust
use tracing::field::{debug, display};
use std::fmt;

/// Wrapper that redacts sensitive data for logging
pub struct Redacted<T>(pub T);

impl<T: fmt::Debug> fmt::Debug for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

impl<T: fmt::Display> fmt::Display for Redacted<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[REDACTED]")
    }
}

// Usage
let password = "secret123";
info!(password = debug(&Redacted(password)), "Processing login");

// Or use a custom type
pub struct Credential {
    pub username: String,
    pub password: String,
}

impl fmt::Debug for Credential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credential")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}
```

### Field Filtering by Span

```rust
use tracing_subscriber::{
    filter::filter_fn,
    layer::SubscriberExt,
    Registry,
};

// Filter out spans containing sensitive data
let safe_layer = tracing_subscriber::fmt::layer()
    .with_filter(filter_fn::with_max_level(tracing::Level::INFO))
    .with_filter(filter_fn::exclude_any(vec![
        filter_fn::has_field("password"),
        filter_fn::has_field("token"),
        filter_fn::has_field("secret"),
        filter_fn::has_field("api_key"),
    ]));

let subscriber = Registry::default().with(safe_layer);
```

### Audit Logging

```rust
use tracing::instrument;
use chrono::Utc;

#[instrument(
    skip(password),
    fields(
        username = %username,
        source_ip = %source_ip,
        timestamp = %Utc::now(),
        event_type = "authentication_attempt"
    )
)]
fn audit_login(username: &str, password: &str, source_ip: &str) -> bool {
    let success = verify_credentials(username, password);

    if success {
        info!(event_type = "authentication_success");
    } else {
        warn!(event_type = "authentication_failure");
    }

    success
}
```

---

## Troubleshooting

### Diagnosing Missing Logs

```rust
// Check if subscriber is set
if tracing::subscriber::get_default().is::<tracing::subscriber::NoSubscriber>() {
    eprintln!("WARNING: No tracing subscriber configured!");
}

// Enable all logs temporarily for debugging
std::env::set_var("RUST_LOG", "trace");

// Reinitialize with verbose output
use tracing_subscriber::{fmt, EnvFilter};

let subscriber = Registry::default()
    .with(EnvFilter::new("trace"))
    .with(fmt::layer().pretty());

tracing::subscriber::set_default(subscriber);
```

### Checking Span Hierarchy

```rust
use tracing_subscriber::{
    fmt::format::FmtSpan,
    layer::SubscriberExt,
    Registry,
};

// Enable span lifecycle logging
let layer = fmt::layer()
    .pretty()
    .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
    .with_filter(EnvFilter::from_default_env());

let subscriber = Registry::default().with(layer);
tracing::subscriber::set_global_default(subscriber)?;
```

### Performance Profiling

```rust
// Add timing layer
use tracing_subscriber::fmt::format::FmtSpan;

let timing_layer = fmt::layer()
    .pretty()
    .with_span_events(FmtSpan::CLOSE)  // Log span duration on close
    .with_filter(EnvFilter::new("info"));

// Or use a custom timing layer
pub struct TimingLayer;

impl<S: Subscriber> Layer<S> for TimingLayer {
    fn on_close(&self, id: Id, ctx: Context<'_>) {
        if let Some(span) = ctx.span(&id) {
            let metadata = span.metadata();
            tracing::info!(
                span_name = metadata.name(),
                span_duration_ms = span.extensions()
                    .get::<Instant>()
                    .map(|i| i.elapsed().as_millis())
                    .unwrap_or(0),
                "Span closed"
            );
        }
    }
}
```

### Common Issues

| Issue | Solution |
|-------|----------|
| Logs not appearing | Check `RUST_LOG` environment variable |
| Spans not nested | Ensure `span.enter()` or `in_current_span()` is used |
| Missing fields in logs | Verify fields are recorded before span closes |
| Performance degradation | Use `if enabled!()` checks for expensive operations |
| Logs not flushed | Keep `WorkerGuard` alive, call `shutdown_tracer_provider()` |

---

## Resources

- [tracing-appender](https://docs.rs/tracing-appender) - Non-blocking appenders
- [tracing-opentelemetry](https://docs.rs/tracing-opentelemetry) - OpenTelemetry integration
- [tracing-subscriber](https://docs.rs/tracing-subscriber) - Subscriber implementations
- [OpenTelemetry Rust](https://github.com/open-telemetry/opentelemetry-rust)
