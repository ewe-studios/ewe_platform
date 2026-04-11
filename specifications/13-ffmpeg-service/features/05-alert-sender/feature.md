---
feature: "Alert Sender"
description: "packages/alert-sender Rust library: Rust models mirroring AlertTaskRequest/AlertEntryPayload, AlertSender that POSTs to the data-alerts HTTP endpoint best-effort at failure boundaries in lambda_worker"
status: "pending"
priority: "medium"
depends_on: ["03-lambda-worker"]
estimated_effort: "small"
created: 2026-04-10
last_updated: 2026-04-10
author: "Ewetumo Alexander"
tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---

# Feature 05: Alert Sender

## Goal

Build a small `packages/alert-sender` library crate that:

1. Defines Rust structs mirroring `AlertTaskRequest` / `AlertEntryPayload` from the Python
   `clover-data-models` package — serialising to the exact same JSON the data-alerts service expects
2. Provides an `AlertSender` that reads `DATA_ALERTS_URL` from the environment and POSTs alerts
   via HTTP
3. Is integrated into `bin/lambda_worker` so that conversion failures are reported to the
   data-alerts service best-effort — errors are logged as `warn!` and never propagate

The library knows nothing about conversions, file stores, or Lambda. Any binary that needs to
report alerts adds it as a dependency and calls `AlertSender::send_alerts`.

---

## Workspace Structure

```
packages/alert-sender/
├── Cargo.toml
└── src/
    ├── lib.rs       # pub use AlertSender, AlertEntry, AlertSeverity, SenderError, error_stack
    ├── error.rs     # SenderError — derive_more + serde
    ├── models.rs    # AlertEntry, AlertTaskRequest, AlertSeverity — serde-exact mirror of Python
    └── sender.rs    # AlertSender struct + send_alerts impl
```

---

## Python Model Reference

The JSON the data-alerts service expects (produced by `model_dump_json(by_alias=True)`):

```json
{
  "type": "alert_task_request",
  "trace_id": "a1b2c3d4-...",
  "context": {},
  "task": {
    "type": "alert_entry",
    "trace_id": "a1b2c3d4-...",
    "severity": 4,
    "group": "call-quality-ffmpeg",
    "title": "Conversion Failed",
    "message": "failed to convert 's3://bucket/call.opus' to wave",
    "service": "call-quality-ffmpeg",
    "event": "ConversionFailed",
    "data": { "target_file": "s3://...", "output_format": "wave" },
    "config": {},
    "request_type": "",
    "when": "2026-04-10T12:00:00Z",
    "broken": false,
    "correlation_1_key": "target_file",
    "correlation_1_value": "s3://bucket/calls/call-001.opus",
    "correlation_2_key": "output_format",
    "correlation_2_value": "wave",
    "correlation_3_key": "request_id",
    "correlation_3_value": "lambda-request-id"
  }
}
```

`AlertSeverity` maps to Python `AlertSeverity(IntEnum)`:

| Rust variant | Python | int |
|---|---|---|
| `Low` | `LOW` | 1 |
| `Medium` | `MEDIUM` | 2 |
| `High` | `HIGH` | 3 |
| `Critical` | `CRITICAL` | 4 |
| `Info` | `INFO` | 5 |

---

## Task Breakdown

- [ ] **T01** — Create `packages/alert-sender/Cargo.toml`:
  ```toml
  [package]
  name = "alert-sender"
  version = "0.1.0"
  edition.workspace = true
  rust-version.workspace = true
  license.workspace = true
  authors.workspace = true
  repository.workspace = true

  [dependencies]
  reqwest     = { version = "0.12", features = ["json"] }
  serde       = { version = "1",    features = ["derive"] }
  serde_json  = "1"
  uuid        = { version = "1",    features = ["v4", "serde"] }
  chrono      = { version = "0.4",  features = ["serde"] }
  derive_more = { version = "1",    features = ["display", "error", "from"] }
  error-stack = "0.5"
  tracing     = "0.1"
  tokio       = { version = "1",    features = ["rt"] }
  ```

- [ ] **T02** — Define `SenderError` in `src/error.rs`:
  ```rust
  use derive_more::{Display, Error};
  use serde::{Deserialize, Serialize};

  #[derive(Debug, Display, Error, Serialize, Deserialize)]
  pub enum SenderError {
      /// DATA_ALERTS_URL is not set in the environment.
      #[display("missing required environment variable 'DATA_ALERTS_URL'")]
      MissingConfig,

      /// The HTTP POST to the data-alerts endpoint failed or returned non-2xx.
      /// `endpoint` is the URL attempted; the full error is attached via attach_printable.
      #[display("failed to send alert to '{endpoint}'")]
      SendFailed { endpoint: String },
  }
  ```

- [ ] **T03** — Define Rust models in `src/models.rs` that serialise to the exact JSON the
  data-alerts service expects. The `type_` → `"type"` alias is the only serde customisation needed:

  ```rust
  use std::collections::HashMap;
  use chrono::{DateTime, Utc};
  use serde::{Deserialize, Serialize};
  use uuid::Uuid;

  /// Mirror of Python AlertSeverity(IntEnum). Serialised as integer.
  #[derive(Debug, Clone, Copy, Serialize, Deserialize)]
  #[serde(into = "u8", from = "u8")]
  pub enum AlertSeverity {
      Low      = 1,
      Medium   = 2,
      High     = 3,
      Critical = 4,
      Info     = 5,
  }

  impl From<AlertSeverity> for u8 {
      fn from(s: AlertSeverity) -> u8 { s as u8 }
  }
  impl From<u8> for AlertSeverity {
      fn from(v: u8) -> Self {
          match v { 1 => Self::Low, 2 => Self::Medium, 3 => Self::High,
                    5 => Self::Info, _ => Self::Critical }
      }
  }

  /// Mirror of Python AlertEntryPayload. Every field name matches the JSON key exactly.
  /// `type_` uses rename so it serialises as "type" (matching Python's by_alias=True).
  #[derive(Debug, Clone, Serialize, Deserialize)]
  pub struct AlertEntry {
      #[serde(rename = "type")]
      pub type_: &'static str,          // always "alert_entry"

      pub trace_id:      Uuid,
      pub severity:      AlertSeverity,
      pub group:         String,
      pub title:         String,
      pub message:       String,
      pub service:       String,
      pub event:         String,
      pub data:          HashMap<String, serde_json::Value>,
      pub config:        HashMap<String, serde_json::Value>,
      pub request_type:  String,        // always ""
      pub when:          DateTime<Utc>,
      pub broken:        bool,          // always false

      pub correlation_1_key:   Option<String>,
      pub correlation_1_value: Option<String>,
      pub correlation_2_key:   Option<String>,
      pub correlation_2_value: Option<String>,
      pub correlation_3_key:   Option<String>,
      pub correlation_3_value: Option<String>,
  }

  impl AlertEntry {
      /// Convenience constructor with sensible defaults for call-quality-ffmpeg.
      pub fn new(
          severity: AlertSeverity,
          title: impl Into<String>,
          message: impl Into<String>,
          event: impl Into<String>,
          data: HashMap<String, serde_json::Value>,
      ) -> Self {
          Self {
              type_:         "alert_entry",
              trace_id:      Uuid::new_v4(),
              severity,
              group:         "call-quality-ffmpeg".into(),
              title:         title.into(),
              message:       message.into(),
              service:       "call-quality-ffmpeg".into(),
              event:         event.into(),
              data,
              config:        HashMap::new(),
              request_type:  String::new(),
              when:          Utc::now(),
              broken:        false,
              correlation_1_key:   None,
              correlation_1_value: None,
              correlation_2_key:   None,
              correlation_2_value: None,
              correlation_3_key:   None,
              correlation_3_value: None,
          }
      }
  }

  /// Mirror of Python AlertTaskRequest(CloudEventPayload).
  #[derive(Debug, Serialize, Deserialize)]
  pub struct AlertTaskRequest {
      #[serde(rename = "type")]
      pub type_:    &'static str,   // always "alert_task_request"
      pub trace_id: Uuid,
      pub context:  HashMap<String, serde_json::Value>,
      pub task:     AlertEntry,
  }

  impl AlertTaskRequest {
      pub fn from_entry(entry: AlertEntry) -> Self {
          let trace_id = entry.trace_id;
          Self {
              type_:    "alert_task_request",
              trace_id,
              context:  HashMap::new(),
              task:     entry,
          }
      }
  }
  ```

- [ ] **T04** — Implement `AlertSender` in `src/sender.rs`:

  ```rust
  use reqwest::Client;
  use crate::error::SenderError;
  use crate::models::{AlertEntry, AlertTaskRequest};

  pub struct AlertSender {
      client:   Client,
      endpoint: String,
  }

  impl AlertSender {
      pub fn new(endpoint: String) -> Self {
            Some(Self { client: Client::new(), endpoint })
      }
      
      /// Returns Ok(Some(sender)) when DATA_ALERTS_URL is set.
      /// Returns Ok(None) when the variable is absent — alerting is gracefully disabled.
      pub fn from_env() -> error_stack::Result<Option<Self>, SenderError> {
          match std::env::var("DATA_ALERTS_URL") {
              Ok(endpoint) => {
                  tracing::info!(%endpoint, "alert sender initialised");
                  Ok(Some(Self { client: Client::new(), endpoint }))
              }
              Err(_) => {
                  tracing::info!("DATA_ALERTS_URL not set — alert sending disabled");
                  Ok(None)
              }
          }
      }

      /// POST each entry to the data-alerts endpoint as an AlertTaskRequest.
      ///
      /// All entries are sent sequentially. If any send fails, the error is
      /// returned; the caller is responsible for logging and not propagating it.
      #[tracing::instrument(skip(self, entries), fields(endpoint = %self.endpoint, count = entries.len()))]
      pub async fn send_alerts(
          &self,
          entries: Vec<AlertEntry>,
      ) -> error_stack::Result<(), SenderError> {
          use error_stack::ResultExt as _;

          tracing::debug!(count = entries.len(), "sending alerts to data-alerts service");

          for entry in entries {
              let event = entry.event.clone();
              let request = AlertTaskRequest::from_entry(entry);

              let response = self.client
                  .post(&self.endpoint)
                  .json(&request)
                  .send()
                  .await
                  .change_context(SenderError::SendFailed { endpoint: self.endpoint.clone() })
                  .attach_printable_lazy(|| format!("event: {event}"))?;

              let status = response.status();
              if !status.is_success() {
                  return Err(error_stack::report!(SenderError::SendFailed {
                      endpoint: self.endpoint.clone(),
                  }))
                  .attach_printable(format!("HTTP {status}"))
                  .attach_printable(format!("event: {event}"));
              }

              tracing::debug!(%event, "alert sent");
          }

          tracing::info!("all alerts sent");
          Ok(())
      }
  }
  ```

- [ ] **T05** — Wire `src/lib.rs`:
  ```rust
  pub mod error;
  pub mod models;
  mod sender;

  pub use error::SenderError;
  pub use models::{AlertEntry, AlertSeverity, AlertTaskRequest};
  pub use sender::AlertSender;
  pub use error_stack;
  ```

- [ ] **T06** — Integrate `alert-sender` into `bin/lambda_worker`:

  Add to `bin/lambda_worker/Cargo.toml`:
  ```toml
  alert-sender = { path = "../../packages/alert-sender" }
  ```

  Build the sender once in `main` alongside the SQS client:
  ```rust
  let alert_sender: Option<AlertSender> = AlertSender::from_env()
      .unwrap_or_else(|report| {
          tracing::warn!(err = %report, "alert sender init failed — alerting disabled");
          None
      });
  let alert_sender = Arc::new(alert_sender);
  ```

  Pass `request_id` and `alert_sender` into `process`. In the failure arm, after the
  SQS failure delivery, build and dispatch the alert best-effort:

  ```rust
  Err(report) => {
      tracing::error!(err = %report, "conversion failed");

      // SQS failure delivery (feature 03)
      // ...

      // Alert service — best-effort; never propagate
      if let Some(ref sender) = *alert_sender {
          let mut data = serde_json::Map::new();
          data.insert("target_file".into(),   task.target_file.clone().into());
          data.insert("output_format".into(), task.output_format.clone().into());
          data.insert("output_path".into(),   output_path.clone().into());
          data.insert("error".into(), report.current_context().to_string().into());

          let mut entry = AlertEntry::new(
              AlertSeverity::Critical,
              "Conversion Failed",
              report.current_context().to_string(),
              "ConversionFailed",
              data.into_iter().collect(),
          );
          // Correlations for filtering and context in the handler
          entry.correlation_1_key   = Some("target_file".into());
          entry.correlation_1_value = Some(task.target_file.clone());
          entry.correlation_2_key   = Some("output_format".into());
          entry.correlation_2_value = Some(task.output_format.clone());
          entry.correlation_3_key   = Some("request_id".into());
          entry.correlation_3_value = Some(request_id.to_string());

          if let Err(e) = sender.send_alerts(vec![entry]).await {
              tracing::warn!(err = %e, "best-effort alert delivery did not complete");
          }
      }

      Err(report)
  }
  ```

  Alert failure must never replace or mask the original conversion error.

- [ ] **T07** — Constants: define `SERVICE_NAME`, `ALERT_GROUP`, and event name in a `src/constants.rs`
  (or top of `models.rs`) in the lambda_worker crate:

  ```rust
  pub const SERVICE_NAME:  &str = "call-quality-ffmpeg";
  pub const ALERT_GROUP:   &str = "call-quality-ffmpeg";
  pub const EVENT_CONVERSION_FAILED: &str = "ConversionFailed";
  ```

  These strings must exactly match the `service_name` and `event` the Python handler filters on
  (see feature 06). Changing them without updating the Python handler breaks alert routing.

- [ ] **T08** — Verify JSON serialisation in a unit test:

  ```rust
  #[test]
  fn alert_task_request_serialises_correctly() {
      let entry = AlertEntry::new(
          AlertSeverity::Critical,
          "Conversion Failed",
          "ffmpeg encoding failed (exit code -9)",
          "ConversionFailed",
          HashMap::new(),
      );
      let request = AlertTaskRequest::from_entry(entry);
      let json = serde_json::to_value(&request).unwrap();

      assert_eq!(json["type"],       "alert_task_request");
      assert_eq!(json["task"]["type"],    "alert_entry");
      assert_eq!(json["task"]["severity"], 4);             // Critical = 4
      assert_eq!(json["task"]["service"],  "call-quality-ffmpeg");
      assert_eq!(json["task"]["event"],    "ConversionFailed");
      assert_eq!(json["task"]["group"],    "call-quality-ffmpeg");
  }
  ```

---

## Tracing Reference

| Level | Location | Event | Key fields |
|---|---|---|---|
| `info` | `AlertSender::from_env` | `"alert sender initialised"` | `endpoint` |
| `info` | `AlertSender::from_env` | `"DATA_ALERTS_URL not set — alert sending disabled"` | — |
| `debug` | `send_alerts` | `"sending alerts to data-alerts service"` | `endpoint`, `count` (span) |
| `debug` | `send_alerts` (loop) | `"alert sent"` | `event` |
| `info` | `send_alerts` | `"all alerts sent"` | `endpoint`, `count` (span) |
| `warn` | `process` (lambda) | `"best-effort alert delivery did not complete"` | `err` |
| `warn` | `main` (lambda) | `"alert sender init failed — alerting disabled"` | `err` |

---

## Environment Variables

| Variable | Required | Description |
|---|---|---|
| `DATA_ALERTS_URL` | Optional | Full URL of the data-alerts HTTP endpoint (e.g. `http://data-alerts:8082/events`). When absent, alerting is silently disabled. |

---

## Success Criteria

- [ ] `cargo build -p alert-sender` compiles clean with no warnings
- [ ] `cargo clippy -p alert-sender -- -D warnings` passes
- [ ] `AlertTaskRequest` serialises to JSON that exactly matches the Python `model_dump_json(by_alias=True)` output (verified by T08 unit test)
- [ ] `AlertSeverity::Critical` serialises as integer `4`
- [ ] `AlertSender::from_env()` returns `None` when `DATA_ALERTS_URL` is absent — Lambda starts successfully
- [ ] Non-2xx HTTP responses return `SenderError::SendFailed` with the status code attached
- [ ] Alert sender failure never propagates from the lambda handler — only `warn!` is emitted
- [ ] `service`, `event`, and `group` constants in the lambda_worker exactly match the Python handler filter values (feature 06)
- [ ] `request_id` is always included as `correlation_3_value` for cross-service correlation
