---
feature: "Slack Alerter"
description: "packages/slack-alerter library crate: SlackAlerter with send_alert(message, payload) for reporting failures to Slack via incoming webhook; integrated into lambda_worker"
status: "pending"
priority: "medium"
depends_on: ["03-lambda-worker"]
estimated_effort: "small"
created: 2026-04-10
last_updated: 2026-04-10
author: "Ewetumo Alexander"
tasks:
  completed: 0
  uncompleted: 7
  total: 7
  completion_percentage: 0%
---

# Feature 05: Slack Alerter

## Goal

Build a small, reusable `packages/slack-alerter` library crate that provides a single
`SlackAlerter::send_alert(message, payload)` method for posting structured failure notices to a
Slack channel via an incoming webhook.

Integrate it into `bin/lambda_worker` so that conversion failures are reported to Slack in addition
to the SQS failure delivery already implemented in feature 03.

The library is intentionally minimal — it knows nothing about conversions, file stores, or Lambda.
Any binary that needs to alert Slack adds it as a dependency and calls `send_alert`.

---

## Workspace Structure

```
packages/slack-alerter/
├── Cargo.toml
└── src/
    ├── lib.rs       # pub use SlackAlerter, AlerterError, error_stack
    ├── error.rs     # AlerterError — derive_more + serde
    └── alerter.rs   # SlackAlerter struct + send_alert impl
```

---

## Public API

```rust
use std::collections::HashMap;

pub struct SlackAlerter { /* private fields */ }

impl SlackAlerter {
    /// Reads SLACK_WEBHOOK_URL and SLACK_CHANNEL from environment.
    ///
    /// Returns Ok(Some(alerter)) when both vars are present.
    /// Returns Ok(None) when neither is set — alerting is gracefully disabled.
    /// Returns Err when exactly one var is set — that is a misconfiguration.
    pub fn from_env() -> error_stack::Result<Option<Self>, AlerterError>;

    /// Post a message to the configured Slack channel.
    ///
    /// `message` — human-readable one-line summary shown as the notification text.
    /// `payload` — key/value pairs rendered as Slack attachment fields below the summary.
    ///
    /// Returns Ok(()) on HTTP 2xx. Any network error or non-2xx response is returned
    /// as AlerterError::SendFailed with the HTTP status attached via attach_printable.
    pub async fn send_alert(
        &self,
        message: &str,
        payload: &HashMap<String, String>,
    ) -> error_stack::Result<(), AlerterError>;
}
```

---

## Task Breakdown

- [ ] **T01** — Create `packages/slack-alerter/Cargo.toml`:
  ```toml
  [package]
  name = "slack-alerter"
  version = "0.1.0"
  edition.workspace = true
  rust-version.workspace = true
  license.workspace = true
  authors.workspace = true
  repository.workspace = true

  [dependencies]
  reqwest      = { version = "0.12", features = ["json"] }
  serde        = { version = "1",    features = ["derive"] }
  serde_json   = "1"
  derive_more  = { version = "1",    features = ["display", "error", "from"] }
  error-stack  = "0.5"
  tracing      = "0.1"
  tokio        = { version = "1",    features = ["rt"] }
  ```

  > `reqwest` is the only new dependency introduced by this crate. It uses the `json` feature for
  > serialising the Slack payload; it does not add TLS features — the Lambda and CLI environments
  > already have system TLS available.

- [ ] **T02** — Define `AlerterError` in `src/error.rs`:

  ```rust
  use derive_more::{Display, Error};
  use serde::{Deserialize, Serialize};

  #[derive(Debug, Display, Error, Serialize, Deserialize)]
  pub enum AlerterError {
      /// Exactly one of the two required env vars is set — both must be present or both absent.
      /// `var` names the missing one so the operator can see exactly what is wrong.
      #[display("missing required environment variable '{var}'")]
      MissingConfig { var: String },

      /// The Slack webhook request failed — network error or non-2xx HTTP response.
      /// `message` is the alert text that was attempted, for correlation in logs.
      #[display("failed to send Slack alert for: '{message}'")]
      SendFailed { message: String },
  }
  ```

- [ ] **T03** — Implement `SlackAlerter` in `src/alerter.rs`:

  ```rust
  use std::collections::HashMap;
  use error_stack::ResultExt as _;
  use reqwest::Client;
  use serde_json::json;
  use crate::error::AlerterError;

  pub struct SlackAlerter {
      client:      Client,
      webhook_url: String,
      channel:     String,
  }

  impl SlackAlerter {
      pub fn from_env() -> error_stack::Result<Option<Self>, AlerterError> {
          let webhook = std::env::var("SLACK_WEBHOOK_URL").ok();
          let channel = std::env::var("SLACK_CHANNEL").ok();

          match (webhook, channel) {
              (Some(webhook_url), Some(channel)) => {
                  tracing::info!(%channel, "Slack alerter initialised");
                  Ok(Some(Self { client: Client::new(), webhook_url, channel }))
              }
              (None, None) => {
                  tracing::info!("Slack alerting disabled (SLACK_WEBHOOK_URL and SLACK_CHANNEL not set)");
                  Ok(None)
              }
              (Some(_), None) => {
                  Err(error_stack::report!(AlerterError::MissingConfig {
                      var: "SLACK_CHANNEL".into()
                  }))
                  .attach_printable("SLACK_WEBHOOK_URL is set — SLACK_CHANNEL must also be provided")
              }
              (None, Some(_)) => {
                  Err(error_stack::report!(AlerterError::MissingConfig {
                      var: "SLACK_WEBHOOK_URL".into()
                  }))
                  .attach_printable("SLACK_CHANNEL is set — SLACK_WEBHOOK_URL must also be provided")
              }
          }
      }

      #[tracing::instrument(skip(self, payload), fields(channel = %self.channel, %message))]
      pub async fn send_alert(
          &self,
          message: &str,
          payload: &HashMap<String, String>,
      ) -> error_stack::Result<(), AlerterError> {
          tracing::debug!("constructing Slack payload");

          // Render key/value pairs as Slack attachment fields.
          let fields: Vec<_> = payload.iter().map(|(k, v)| {
              json!({ "title": k, "value": v, "short": true })
          }).collect();

          let body = json!({
              "channel":     self.channel,
              "username":    "call-quality-ffmpeg",
              "text":        message,
              "attachments": [{ "color": "danger", "fields": fields }],
          });

          tracing::debug!("sending Slack alert");

          let response = self.client
              .post(&self.webhook_url)
              .json(&body)
              .send()
              .await
              .change_context(AlerterError::SendFailed { message: message.to_string() })
              .attach_printable("reqwest send failed")?;

          let status = response.status();
          if !status.is_success() {
              return Err(error_stack::report!(AlerterError::SendFailed {
                  message: message.to_string(),
              }))
              .attach_printable(format!("HTTP {status}"));
          }

          tracing::info!("Slack alert sent");
          Ok(())
      }
  }
  ```

- [ ] **T04** — Wire `lib.rs`:
  ```rust
  pub mod error;
  mod alerter;

  pub use alerter::SlackAlerter;
  pub use error::AlerterError;
  pub use error_stack;
  ```

- [ ] **T05** — Integrate `slack-alerter` into `bin/lambda_worker`:

  Add to `bin/lambda_worker/Cargo.toml`:
  ```toml
  slack-alerter = { path = "../../packages/slack-alerter" }
  ```

  Build the alerter once in `main` alongside the SQS client:
  ```rust
  let alerter: Option<SlackAlerter> = SlackAlerter::from_env()
      .unwrap_or_else(|report| {
          tracing::warn!(err = %report, "Slack alerter misconfigured — alerting disabled");
          None
      });
  let alerter = Arc::new(alerter);
  ```

  Pass it into `handler` → `process` and call it best-effort in the failure arm of
  `process`, after the SQS failure delivery:

  ```rust
  Err(report) => {
      tracing::error!(err = %report, "conversion failed");

      // SQS failure delivery (existing — from feature 03)
      let failure = ConversionFailure { /* … */ };
      if let Err(e) = send_to_sqs(sqs, sqs_url, &failure).await {
          tracing::warn!(err = %e, "best-effort SQS failure delivery did not complete");
      }

      // Slack alert — best-effort; alerter is None when env vars are not set
      if let Some(ref alerter) = *alerter {
          let mut slack_payload = HashMap::new();
          slack_payload.insert("target_file".into(),   task.target_file.clone());
          slack_payload.insert("output_format".into(), task.output_format.clone());
          slack_payload.insert("output_path".into(),   output_path.clone());
          slack_payload.insert("error".into(), report.current_context().to_string());

          let summary = format!(
              ":x: Conversion failed: {} → {}",
              task.target_file, task.output_format,
          );
          if let Err(e) = alerter.send_alert(&summary, &slack_payload).await {
              tracing::warn!(err = %e, "best-effort Slack alert did not complete");
          }
      }

      Err(report)
  }
  ```

  **Contract**: `send_alert` is always called best-effort — its failure must never replace or mask
  the original conversion error. The `let _ =` / `if let Err(e)` pattern ensures the conversion
  error is always returned to the Lambda runtime regardless of Slack delivery outcome.

- [ ] **T06** — Add Slack payload content spec:

  The Slack message for a conversion failure should include:

  | Field in payload | Value source |
  |---|---|
  | `target_file` | `task.target_file` |
  | `output_format` | `task.output_format` |
  | `output_path` | derived or explicit output path |
  | `error` | `report.current_context().to_string()` — domain-level summary |
  | `request_id` | `ctx.request_id` — correlation with CloudWatch |

  The `message` (top-level Slack notification text) uses the format:
  ```
  :x: Conversion failed: <target_file> → <output_format>
  ```

  This gives operators an immediate at-a-glance summary in Slack, with the full detail available
  in the attachment fields and CloudWatch logs via `request_id`.

- [ ] **T07** — Handle `request_id` availability in `process`:

  `request_id` is extracted in `handler`, not `process`. Pass it into `process` as a parameter
  so it can be included in the Slack payload without restructuring the span model:

  ```rust
  async fn process(
      evt: &ConversionEvent,
      sqs: &SqsClient,
      alerter: &Option<SlackAlerter>,
      request_id: &str,          // ← added for Slack payload correlation
  ) -> error_stack::Result<ConversionResponse, LambdaError>
  ```

  Then in the Slack payload:
  ```rust
  slack_payload.insert("request_id".into(), request_id.to_string());
  ```

---

## Slack Webhook Format

The JSON body sent to `SLACK_WEBHOOK_URL`:

```json
{
  "channel": "#data-engineering-alerts",
  "username": "call-quality-ffmpeg",
  "text": ":x: Conversion failed: s3://my-bucket/calls/call-001.opus → wave",
  "attachments": [
    {
      "color": "danger",
      "fields": [
        { "title": "target_file",   "value": "s3://my-bucket/calls/call-001.opus", "short": true },
        { "title": "output_format", "value": "wave",                               "short": true },
        { "title": "output_path",   "value": "s3://my-bucket/converted/call-001.wav", "short": true },
        { "title": "error",         "value": "ffmpeg encoding failed (exit code -9)",  "short": false },
        { "title": "request_id",    "value": "abc-123-def-456",                    "short": true }
      ]
    }
  ]
}
```

---

## Tracing Reference

| Level | Location | Event | Key fields |
|---|---|---|---|
| `info` | `SlackAlerter::from_env` | `"Slack alerter initialised"` | `channel` |
| `info` | `SlackAlerter::from_env` | `"Slack alerting disabled …"` | — |
| `debug` | `send_alert` | `"constructing Slack payload"` | `channel`, `message` (span) |
| `debug` | `send_alert` | `"sending Slack alert"` | `channel`, `message` (span) |
| `info` | `send_alert` | `"Slack alert sent"` | `channel`, `message` (span) |
| `warn` | `process` (lambda) | `"best-effort Slack alert did not complete"` | `err` |
| `warn` | `main` (lambda) | `"Slack alerter misconfigured — alerting disabled"` | `err` |

`send_alert` uses `#[tracing::instrument]` with `skip(self, payload)` and explicit `channel` and
`message` fields on the span, so both appear on every child event without being re-stated.

---

## Environment Variables

| Variable | Required | Description |
|---|---|---|
| `SLACK_WEBHOOK_URL` | Optional (both or neither) | Slack incoming webhook URL |
| `SLACK_CHANNEL` | Optional (both or neither) | Target channel, e.g. `#data-engineering-alerts` |

Both variables must be set together or not at all. Setting only one is treated as a misconfiguration
(logged as `warn` and alerting disabled) — it does not fail Lambda startup.

---

## Success Criteria

- [ ] `cargo build -p slack-alerter` compiles clean with no warnings
- [ ] `cargo clippy -p slack-alerter -- -D warnings` passes
- [ ] `SlackAlerter::from_env()` returns `Ok(None)` when neither env var is set
- [ ] `SlackAlerter::from_env()` returns `Err(MissingConfig)` when only one env var is set
- [ ] `send_alert` sends a POST to `SLACK_WEBHOOK_URL` with correct JSON body (channel, text, attachments)
- [ ] Non-2xx HTTP responses return `AlerterError::SendFailed` with the status code in `attach_printable`
- [ ] Slack alert failure never propagates as the returned error — only `warn!` is emitted
- [ ] `AlerterError` variants are JSON-serializable via `serde_json::to_string`
- [ ] `request_id` is included in every Slack payload sent from the lambda_worker
- [ ] Lambda starts successfully when `SLACK_WEBHOOK_URL`/`SLACK_CHANNEL` are absent — alerting silently disabled
- [ ] Lambda logs `warn` (not `error`) when Slack alerter is misconfigured or a Slack send fails
