---
feature: "Lambda Worker"
description: "bin/lambda_worker AWS Lambda binary: parses task/delivery event, resolves FileStore from URL scheme, converts via OpusConverter, delivers result to SQS"
status: "pending"
priority: "high"
depends_on: ["00-library-implementation"]
estimated_effort: "medium"
created: 2026-04-10
last_updated: 2026-04-10
author: "Ewetumo Alexander"
tasks:
  completed: 0
  uncompleted: 9
  total: 9
  completion_percentage: 0%
---

# Feature 03: Lambda Worker

## Goal

Implement `bin/lambda_worker` as an AWS Lambda binary using `lambda_runtime` that:

1. Receives a JSON event split into `task` (conversion details) and `delivery` (where to send results)
2. Resolves the appropriate `FileStore` from the URL scheme of `task.target_file`
3. Converts the file using `OpusConverter`
4. Uploads the result using the resolved `FileStore`
5. Sends the result (success or structured failure) to the SQS queue URL from `delivery.sqs_url`
6. Returns the conversion response to the Lambda runtime for observability

---

## Event Schema

### Input Event

The event is split into two top-level namespaces: `task` describes the conversion work and `delivery` describes where the result should be sent.

```json
{
  "task": {
    "target_file": "s3://my-bucket/calls/call-001.opus",
    "input_format": "opus",
    "output_format": "wave",
    "output_path": "s3://my-bucket/converted/call-001.wav"
  },
  "delivery": {
    "type": "sqs",
    "sqs_url": "https://sqs.us-east-1.amazonaws.com/123456789012/call-results"
  }
}
```

#### `task` fields

| Field | Type | Required | Description |
|---|---|---|---|
| `target_file` | string | yes | URL to the source file. Scheme determines FileStore. |
| `input_format` | string | yes | Input audio format (currently always `"opus"`) |
| `output_format` | string | yes | Target format: `"wave"` or `"mp4"` |
| `output_path` | string | no | Explicit output URL. Defaults to same bucket/prefix with new extension. |

#### `delivery` fields

| Field | Type | Required | Description |
|---|---|---|---|
| `type` | string | yes | Delivery mechanism. Currently only `"sqs"` is supported. |
| `sqs_url` | string | yes (when type = sqs) | Full SQS queue URL. The Lambda must have `sqs:SendMessage` on this queue. |

The `delivery` block is structured as a tagged union keyed on `type` — adding a new delivery mechanism (e.g. SNS, EventBridge) only requires adding a new variant. The Lambda is expected to have been granted the appropriate IAM permissions for whichever delivery target is specified; no credentials are passed in the event.

### URL Scheme → FileStore Mapping

| Scheme | FileStore |
|---|---|
| `s3://` | `S3FileStore` |
| `gs://` or `gcs://` | `GcsFileStore` |
| `/` or `./` (local path) | `LocalFileStore` |

### Delivery Payload (sent to SQS on success)

After a successful conversion the Lambda sends a JSON message to the specified SQS queue.
The original `task` is echoed back so the consumer can attribute the result without any
secondary lookup:

```json
{
  "status": "success",
  "task": {
    "target_file": "s3://my-bucket/calls/call-001.opus",
    "input_format": "opus",
    "output_format": "wave",
    "output_path": "s3://my-bucket/converted/call-001.wav"
  },
  "output_file": "s3://my-bucket/converted/call-001.wav"
}
```

### Delivery Payload (sent to SQS on conversion failure)

If the conversion fails the Lambda still sends a message to SQS so the consumer can react.
The `error` block contains both a short human-readable `message` (the domain error variant's
Display string) and a `details` string with the full error-stack trace including all
`attach_printable` lines and file/line/column annotations:

```json
{
  "status": "failed",
  "error": {
    "message": "failed to convert 's3://my-bucket/calls/call-001.opus' to Wave",
    "details": "ffmpeg encoding failed (exit code -9)\nat src/main.rs:88:14\nnote: output path: s3://my-bucket/converted/call-001.wav"
  }
}
```

### Lambda Return Value

The Lambda returns the same success payload to the runtime after the SQS message is sent.
This is primarily for observability (CloudWatch, X-Ray) — the real result lives in the SQS message.

Lambda runtime propagates conversion and delivery errors as Lambda error responses via the `lambda_runtime` error handling.

---

## Task Breakdown

- [ ] **T01** — Update `bin/lambda_worker/Cargo.toml`:
  ```toml
  [dependencies]
  ffmpeg-converter = { path = "../../packages/ffmpeg-converter" }
  lambda_runtime = "0.13"
  tokio = { version = "1", features = ["full"] }
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
  derive_more = { version = "1", features = ["display", "error", "from"] }
  error-stack = "0.5"
  tracing = "0.1"
  tracing-subscriber = { version = "0.3", features = ["env-filter"] }
  aws-sdk-sqs = "1"
  aws-config = { version = "1", features = ["behavior-version-latest"] }
  slack-alerter = { path = "../../packages/slack-alerter" }
  ```

- [ ] **T02** — Define event and response types in `src/event.rs`:

  The event is split into `task` (what to convert) and `delivery` (where to send the result).
  `DeliveryConfig` is a serde-tagged enum keyed on `"type"` — new delivery mechanisms are added
  as new variants without changing `ConversionEvent`.

  ```rust
  use serde::{Deserialize, Serialize};

  /// Top-level Lambda input event.
  #[derive(Deserialize)]
  pub struct ConversionEvent {
      pub task: TaskDetails,
      pub delivery: DeliveryConfig,
  }

  /// Describes the audio file to convert.
  ///
  /// Derives both Serialize and Deserialize so it can be echoed back in delivery payloads.
  #[derive(Deserialize, Serialize, Clone)]
  pub struct TaskDetails {
      pub target_file: String,
      pub input_format: String,
      pub output_format: String,
      /// Explicit output URL. When absent, derived from target_file by replacing the extension.
      pub output_path: Option<String>,
  }

  /// Where the conversion result should be delivered.
  ///
  /// Keyed on the `"type"` field in JSON:
  ///   `{"type": "sqs", "sqs_url": "https://sqs…"}`
  #[derive(Deserialize)]
  #[serde(tag = "type", rename_all = "snake_case")]
  pub enum DeliveryConfig {
      Sqs { sqs_url: String },
  }

  /// Payload serialized to JSON and sent to the delivery target on success.
  /// Echoes the originating `task` so the consumer can attribute the result
  /// without a secondary lookup.
  #[derive(Serialize)]
  pub struct ConversionResponse {
      pub status: String,       // always "success"
      pub task: TaskDetails,    // echoed from the input event
      pub output_file: String,
  }

  /// Payload serialized to JSON and sent to the delivery target on conversion failure.
  #[derive(Serialize)]
  pub struct ConversionFailure {
      pub status: String,       // always "failed"
      pub error: ErrorDetail,
  }

  /// Structured error block carried in `ConversionFailure`.
  #[derive(Serialize)]
  pub struct ErrorDetail {
      /// Domain-level summary: the Display string of the current error-stack context.
      /// Example: "failed to convert 's3://bucket/call.opus' to Wave"
      pub message: String,
      /// Full error-stack trace including all attach_printable lines and file/line/column.
      /// Produced by `format!("{report}")` (Display on Report).
      pub details: String,
  }
  ```

- [ ] **T03** — Define `LambdaError`, `Severity`, and implement URL scheme resolver in `src/store_resolver.rs`:
  ```rust
  use derive_more::{Display, Error};
  use serde::{Deserialize, Serialize};

  #[derive(Debug, Display, Error, Serialize, Deserialize)]
  pub enum LambdaError {
      /// The scheme and full URL are captured — the error message itself tells you
      /// exactly what was rejected without needing to read attach_printable.
      #[display("unknown URL scheme '{scheme}' in '{url}'")]
      UnknownScheme { scheme: String, url: String },

      /// The backend (s3/gcs/local) and bucket name identify the specific store that
      /// failed to initialise.
      #[display("failed to initialise {backend} store for bucket '{bucket}'")]
      StoreInitFailed { backend: String, bucket: String },

      /// Both source and target format embedded — CloudWatch log line is self-contained.
      #[display("failed to convert '{source}' to {output_format}")]
      ConversionFailed { source: String, output_format: String },

      /// The raw format string is embedded so the log shows exactly what was rejected.
      #[display("unsupported output format '{format}' requested")]
      UnsupportedFormat { format: String },

      /// The delivery target and a short reason are captured — the full SDK error is
      /// added via attach_printable so the CloudWatch log is self-contained.
      #[display("failed to deliver result to '{destination}'")]
      DeliveryFailed { destination: String },
  }

  /// Opaque severity metadata attached to error reports for monitoring systems.
  #[derive(Debug)]
  pub enum Severity { Critical, Warning }

  pub enum StoreKind {
      Local(LocalFileStore),
      S3(S3FileStore),
      Gcs(GcsFileStore),
  }

  pub fn resolve_store(url: &str)
      -> error_stack::Result<StoreKind, LambdaError>
  {
      let scheme = extract_scheme(url);   // "s3", "gs", "gcs", or ""
      let bucket = extract_bucket(url);

      match scheme {
          "s3" => build_s3_store(&bucket)
              .change_context(LambdaError::StoreInitFailed {
                  backend: "s3".into(),
                  bucket: bucket.clone(),
              })
              .attach_printable("check AWS_REGION and IAM permissions for the bucket"),

          "gs" | "gcs" => build_gcs_store(&bucket)
              .change_context(LambdaError::StoreInitFailed {
                  backend: "gcs".into(),
                  bucket: bucket.clone(),
              })
              .attach_printable("check GOOGLE_APPLICATION_CREDENTIALS and bucket IAM policy"),

          "" => Ok(StoreKind::Local(LocalFileStore)),

          _ => Err(report!(LambdaError::UnknownScheme {
              scheme: scheme.into(),
              url: url.into(),
          }))
          .attach_printable("supported schemes: s3://, gs://, gcs://, or a local path"),
      }
  }
  ```

- [ ] **T04** — Implement default output path derivation in `src/store_resolver.rs`:
  - Replace source file extension with the output format extension
  - Preserve the rest of the URL (bucket, prefix)
  - Example: `s3://bucket/calls/call.opus` → `s3://bucket/calls/call.wav`

- [ ] **T05** — Implement SQS delivery in `src/delivery.rs`:

  Construct an SQS client from the ambient AWS config (IAM role / env vars) and send a single
  JSON message to the queue URL specified in the event. The Lambda is assumed to have
  `sqs:SendMessage` permission on the target queue — no credentials are accepted in the event.

  ```rust
  use aws_sdk_sqs::Client as SqsClient;
  use serde::Serialize;

  /// Send a JSON-serializable payload to an SQS queue URL.
  ///
  /// Returns `DeliveryFailed` if the SDK call fails. The raw SDK error string is
  /// attached via `attach_printable` so it appears in CloudWatch logs.
  #[tracing::instrument(skip(client, payload), fields(%queue_url))]
  pub async fn send_to_sqs<T: Serialize>(
      client: &SqsClient,
      queue_url: &str,
      payload: &T,
  ) -> error_stack::Result<(), LambdaError> {
      tracing::debug!("serializing delivery payload");

      let body = serde_json::to_string(payload)
          .change_context(LambdaError::DeliveryFailed {
              destination: queue_url.to_string(),
          })
          .attach_printable("failed to serialize delivery payload to JSON")?;

      tracing::debug!("sending SQS message");

      client
          .send_message()
          .queue_url(queue_url)
          .message_body(body)
          .send()
          .await
          .change_context(LambdaError::DeliveryFailed {
              destination: queue_url.to_string(),
          })
          .attach_printable_lazy(|| format!("sqs_url: {queue_url}"))?;

      tracing::info!("SQS message sent");
      Ok(())
  }
  ```

  Build the client once at handler startup (outside the hot path) and share it via `Arc` or by
  passing it into `process`. `aws_config::load_from_env().await` resolves credentials from the
  Lambda execution role automatically.

- [ ] **T06** — Implement the Lambda handler and inner `process` function in `src/main.rs`:

  `handler` attaches correlation IDs to every error report and dispatches to `process`.
  `process` runs the conversion then calls `send_to_sqs` with either a success or failure payload.
  The SQS client is built once in `main` and passed through.

  ```rust
  type LambdaRuntimeError = Box<dyn std::error::Error + Send + Sync>;

  // The span captures request_id and function_arn so every child event (including
  // those from OpusConverter inside the library) automatically carries them in
  // structured CloudWatch logs without being threaded through as arguments.
  #[tracing::instrument(
      skip(event, sqs),
      fields(
          request_id  = tracing::field::Empty,
          function_arn = tracing::field::Empty,
      )
  )]
  async fn handler(
      event: LambdaEvent<ConversionEvent>,
      sqs: Arc<SqsClient>,
  ) -> Result<ConversionResponse, LambdaRuntimeError>
  {
      let (evt, ctx) = event.into_parts();

      // Record correlation IDs on the span so every child event carries them.
      tracing::Span::current()
          .record("request_id",   &ctx.request_id.as_str())
          .record("function_arn", &ctx.invoked_function_arn.as_str());

      tracing::info!("lambda invocation started");

      let result = process(&evt, &sqs).await
          .attach_printable(format!("request_id: {}", ctx.request_id))
          .attach_printable(format!("function_arn: {}", ctx.invoked_function_arn))
          .map_err(|r| Box::new(LambdaErrorReport(r)) as LambdaRuntimeError);

      match &result {
          Ok(_)  => tracing::info!("lambda invocation completed"),
          Err(e) => tracing::error!(err = %e, "lambda invocation failed"),
      }

      result
  }

  #[tracing::instrument(
      skip(sqs, evt),
      fields(
          target_file   = %evt.task.target_file,
          output_format = %evt.task.output_format,
      )
  )]
  async fn process(
      evt: &ConversionEvent,
      sqs: &SqsClient,
  ) -> error_stack::Result<ConversionResponse, LambdaError> {
      let task = &evt.task;

      let format = parse_output_format(&task.output_format)
          .change_context(LambdaError::UnsupportedFormat {
              format: task.output_format.clone(),
          })
          .attach_printable("supported values: wave, mp4")?;

      let output_format_label = format!("{format:?}");
      let store_kind = resolve_store(&task.target_file)?;

      let output_path = task.output_path.clone()
          .unwrap_or_else(|| derive_output_path(&task.target_file, &format));

      tracing::debug!(%output_path, "resolved output path");
      tracing::info!("conversion started");

      // Obtain source stream and output writer from the resolved store, then pass
      // both directly to OpusConverter::convert — the converter is unaware of paths.
      let conversion_result = {
          let (source, writer) = match &store_kind {
              StoreKind::Local(s) => (s.get_stream(&task.target_file)?, s.upload(&output_path)?),
              StoreKind::S3(s)    => (s.get_stream(&task.target_file)?, s.upload(&output_path)?),
              StoreKind::Gcs(s)   => (s.get_stream(&task.target_file)?, s.upload(&output_path)?),
          };
          OpusConverter::convert(source, writer, format)
              .change_context(LambdaError::ConversionFailed {
                  source: task.target_file.clone(),
                  output_format: output_format_label,
              })
              .attach_printable(format!("output path: {output_path}"))
              .attach_opaque(Severity::Critical)
      };

      // Resolve the delivery target from the event.
      let DeliveryConfig::Sqs { sqs_url } = &evt.delivery;

      match conversion_result {
          Ok(()) => {
              tracing::info!(%output_path, "conversion completed");

              let response = ConversionResponse {
                  status: "success".into(),
                  task: task.clone(),     // echoed for consumer attribution
                  output_file: output_path,
              };
              send_to_sqs(sqs, sqs_url, &response).await?;
              Ok(response)
          }
          Err(report) => {
              tracing::error!(err = %report, "conversion failed");

              // Best-effort: attempt to deliver a failure notice to SQS before
              // propagating the error. If SQS delivery itself fails, the original
              // conversion error is still returned to the Lambda runtime.
              let failure = ConversionFailure {
                  status: "failed".into(),
                  error: ErrorDetail {
                      message: report.current_context().to_string(),
                      details: format!("{report}"),
                  },
              };
              if let Err(delivery_err) = send_to_sqs(sqs, sqs_url, &failure).await {
                  tracing::warn!(
                      err = %delivery_err,
                      %sqs_url,
                      "best-effort SQS failure delivery did not complete",
                  );
              }
              Err(report)
          }
      }
  }
  ```

  **Layering strategy**:
  - `change_context(LambdaError::X)` — sets the domain error variant (high-level, returned to caller, JSON-serializable)
  - `.attach_printable("...")` — adds human-readable technical context (visible in logs, not in the API response)
  - `.attach_opaque(Severity::Critical)` — adds typed metadata for monitoring systems that inspect the `Report`
  - `.attach_printable(format!("request_id: ..."))` at the `handler` level — ensures every report carries the correlation ID regardless of which `process` step failed
  - On conversion failure: a `ConversionFailure` message is sent to SQS best-effort before the error propagates, so the consumer is always notified

- [ ] **T07** — Implement `main` with `lambda_runtime` bootstrap and JSON structured logging:
  ```rust
  #[tokio::main]
  async fn main() -> Result<(), LambdaRuntimeError> {
      // JSON logs → CloudWatch → parseable by Datadog / ELK / CloudWatch Insights
      tracing_subscriber::fmt()
          .with_env_filter(EnvFilter::from_default_env())
          .json()
          .init();

      // Build the SQS client once outside the hot path; shared across invocations
      // via the Lambda execution environment reuse mechanism.
      let aws_config = aws_config::load_from_env().await;
      let sqs = Arc::new(aws_sdk_sqs::Client::new(&aws_config));

      lambda_runtime::run(service_fn(move |event| {
          let sqs = Arc::clone(&sqs);
          async move { handler(event, sqs).await }
      })).await?;
      Ok(())
  }
  ```
  When a handler error is returned, `lambda_runtime` calls `Display` on the boxed error — which delegates to `LambdaErrorReport::fmt`, printing the full `error-stack` report including all `attach_printable` lines and the `request_id` attached at handler level. This output appears in CloudWatch structured logs.

- [ ] **T08** — Implement `LambdaErrorReport` adapter in `src/error.rs`:
  ```rust
  use std::fmt;
  use serde::Serialize;

  /// Bridges error_stack::Report<E> → Box<dyn std::error::Error + Send + Sync>
  /// required by lambda_runtime's handler signature.
  ///
  /// Display output (used by lambda_runtime for CloudWatch logging) includes:
  ///   - The high-level domain error message (from derive_more Display on E)
  ///   - All attach_printable lines (technical context, paths, codec args)
  ///   - The request_id and function ARN attached at handler level
  ///   - File/line/column for every change_context/attach_printable call site
  pub struct LambdaErrorReport<E>(pub error_stack::Report<E>)
  where
      E: fmt::Debug + fmt::Display + std::error::Error + Serialize + 'static;

  impl<E: fmt::Debug + fmt::Display + std::error::Error + Serialize + 'static>
      fmt::Debug for LambdaErrorReport<E>
  {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
          fmt::Debug::fmt(&self.0, f)  // full debug report
      }
  }

  impl<E: fmt::Debug + fmt::Display + std::error::Error + Serialize + 'static>
      fmt::Display for LambdaErrorReport<E>
  {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
          fmt::Display::fmt(&self.0, f)  // includes all attach_printable lines
      }
  }

  impl<E: fmt::Debug + fmt::Display + std::error::Error + Serialize + 'static>
      std::error::Error for LambdaErrorReport<E> {}
  ```

- [ ] **T09** — Verify structured tracing output in CloudWatch:

  With `RUST_LOG=debug` set in the Lambda environment, confirm the following events appear in
  CloudWatch Logs Insights as structured JSON fields (because `tracing_subscriber::fmt().json()`
  is used):

  ```json
  { "level": "INFO",  "message": "lambda invocation started",  "request_id": "…", "function_arn": "…" }
  { "level": "INFO",  "message": "conversion started",         "target_file": "…", "output_format": "…" }
  { "level": "DEBUG", "message": "resolved output path",       "output_path": "…" }
  { "level": "INFO",  "message": "conversion completed",       "output_path": "…" }
  { "level": "DEBUG", "message": "sending SQS message",        "queue_url": "…" }
  { "level": "INFO",  "message": "SQS message sent",           "queue_url": "…" }
  { "level": "INFO",  "message": "lambda invocation completed" }
  ```

  On failure:
  ```json
  { "level": "ERROR", "message": "conversion failed",          "err": "…", "target_file": "…" }
  { "level": "WARN",  "message": "best-effort SQS failure delivery did not complete", "err": "…", "sqs_url": "…" }
  { "level": "ERROR", "message": "lambda invocation failed",   "err": "…" }
  ```

  All events inside `handler` and `process` automatically carry `request_id` and `function_arn`
  as span fields without being repeated at every call site.

---

## Environment Variables

| Variable | Required | Description |
|---|---|---|
| `AWS_REGION` | For S3 / SQS | AWS region — used by both the S3FileStore and the SQS client |
| `AWS_ACCESS_KEY_ID` | For S3/SQS (local dev) | AWS credentials; IAM execution role is preferred in Lambda |
| `AWS_SECRET_ACCESS_KEY` | For S3/SQS (local dev) | AWS credentials |
| `GOOGLE_APPLICATION_CREDENTIALS` | For GCS | Path to GCP service account JSON |
| `RUST_LOG` | No | Log level (e.g., `info`, `debug`) |

> **SQS permissions**: the Lambda execution role must have `sqs:SendMessage` on every queue URL that may appear in `delivery.sqs_url`. No SQS credentials are accepted in the event payload.

---

## Notes on Store Dispatch

`OpusConverter::convert` is a plain function with no generic parameters — it takes `impl io::Read + Send` and `impl io::Write + Send`. The `match store_kind` block resolves streams from the concrete store type; after that, `convert` is called once with no knowledge of which backend is in use. No dynamic dispatch and no per-store monomorphisation of the converter.

The `match` arms must destructure `source` and `writer` together because each arm produces a different concrete `Writer` type. If the match body grows, extract a helper that accepts `(impl io::Read + Send, impl io::Write + Send)`.

---

## Tracing Reference

Subscriber is initialised in `main` with `.json()` format (CloudWatch → Logs Insights / Datadog).
Level is controlled by `RUST_LOG` environment variable.

| Level | Location | Event | Key fields |
|---|---|---|---|
| `info` | `handler` | `"lambda invocation started"` | `request_id`, `function_arn` (span) |
| `info` | `process` | `"conversion started"` | `target_file`, `output_format` (span) |
| `debug` | `process` | `"resolved output path"` | `output_path` |
| `info` | `process` | `"conversion completed"` | `output_path` |
| `error` | `process` | `"conversion failed"` | `err` (Display) |
| `debug` | `send_to_sqs` | `"serializing delivery payload"` | `queue_url` (span) |
| `debug` | `send_to_sqs` | `"sending SQS message"` | `queue_url` (span) |
| `info` | `send_to_sqs` | `"SQS message sent"` | `queue_url` (span) |
| `warn` | `process` | `"best-effort SQS failure delivery did not complete"` | `err`, `sqs_url` |
| `info` | `handler` | `"lambda invocation completed"` | `request_id`, `function_arn` (span) |
| `error` | `handler` | `"lambda invocation failed"` | `err`, `request_id`, `function_arn` (span) |
| `trace` | (library) | packet-level transcode loop events | inherited from `process` span |

`request_id` and `function_arn` are recorded on the `handler` span via `tracing::Span::current().record(…)`
after event parsing — not as static fields — so they appear on every child event without being
passed as function arguments.

---

## Error Layering Summary

| Layer | Tool | What goes here |
|---|---|---|
| Domain variant | `change_context(LambdaError::X)` | High-level what-went-wrong (returned to caller, serializable) |
| Technical context | `.attach_printable("...")` | Paths, URLs, codec args, exit codes (logged, not returned to caller) |
| Correlation | `.attach_printable(format!("request_id: {}"))` | Added at handler level; appears in every error regardless of failure site |
| Severity | `.attach_opaque(Severity::Critical)` | Typed metadata for monitoring systems that inspect the Report |

---

## Success Criteria

- [ ] `cargo build -p lambda_worker` compiles clean
- [ ] `cargo clippy -p lambda_worker -- -D warnings` passes
- [ ] Input event deserialises correctly with `task` and `delivery` namespaces
- [ ] `DeliveryConfig` deserialises `{"type": "sqs", "sqs_url": "..."}` into `DeliveryConfig::Sqs`
- [ ] Handler correctly parses all three URL schemes into the right `FileStore`
- [ ] Default output path is derived correctly when `task.output_path` is not provided
- [ ] On success: SQS message body contains `status`, `task` (echoed input), and `output_file`
- [ ] On conversion failure: SQS message body contains `status: "failed"` and an `error` block with both `message` (domain summary) and `details` (full error-stack trace)
- [ ] `ConversionFailure` is sent best-effort — if SQS delivery itself fails the original conversion error still propagates to the Lambda runtime
- [ ] Lambda return value matches `ConversionResponse` schema (with `task` echoed) on success
- [ ] Lambda error output includes `request_id` and `function_arn` from context
- [ ] `LambdaErrorReport::Display` includes all `attach_printable` lines (verified in unit test)
- [ ] `tracing` JSON structured logging is initialised in `main` and emits to CloudWatch
- [ ] `request_id` and `function_arn` appear as fields on every event in the `handler` span
- [ ] `target_file` and `output_format` appear as fields on every event in the `process` span
- [ ] `"conversion started"` and `"conversion completed"` are `info` events; `"resolved output path"` is `debug`
- [ ] `"conversion failed"` is an `error` event with the structured `err` field
- [ ] `"best-effort SQS failure delivery did not complete"` is a `warn` event (not an `error` — it does not block error propagation)
- [ ] `"SQS message sent"` is an `info` event; intermediate SQS steps (`"serializing"`, `"sending"`) are `debug`
- [ ] `LambdaError` variants (including `DeliveryFailed`) are JSON-serializable via `serde_json::to_string`
- [ ] SQS client is built once in `main` and reused across Lambda invocations (not rebuilt per event)
- [ ] `SlackAlerter` is built once in `main` via `from_env()` and passed through; misconfiguration logs `warn` and disables alerting without failing startup
- [ ] On conversion failure, Slack alert is attempted best-effort after SQS failure delivery; Slack failure never masks the original conversion error
- [ ] See feature 05 for the full Slack alerter spec and payload format
