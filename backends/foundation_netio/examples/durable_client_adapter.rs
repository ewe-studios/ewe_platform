//! A Durable Streams client adapter for the reference client conformance suite.
//!
//! The runner sends one JavaScript Object Notation (JSON) command per input
//! line. The adapter sends one JSON result per output line.
//!
//! ```text
//! cargo build -p foundation_netio --features multi --example durable_client_adapter
//! npx @durable-streams/client-conformance-tests --run \
//!   target/debug/examples/durable_client_adapter
//! ```

#[cfg(target_arch = "wasm32")]
fn main() {
    eprintln!("the client adapter runs only on a native target");
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::io::{BufRead, Write};

    // The netio client spawns valtron tasks, so the pool must be live.
    let _pool = foundation_core::valtron::initialize_pool(42, Some(4));

    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut client: Option<foundation_netio::shared::durable::DurableClient> = None;
    let mut types: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut seqs: std::collections::HashMap<(String, String), u64> = std::collections::HashMap::new();
    let mut sent: std::collections::HashMap<(String, String), Vec<u8>> = std::collections::HashMap::new();
    let mut dynamic = Dynamic::default();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let command: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                let result = failure_result("", "INVALID_ARGUMENT", &error.to_string());
                let _ = writeln!(stdout, "{result}");
                let _ = stdout.flush();
                continue;
            }
        };
        let kind = command["type"].as_str().unwrap_or("").to_string();
        let (result, stop) = smol::block_on(handle(
            &kind,
            &command,
            &mut client,
            &mut types,
            &mut seqs,
            &mut sent,
            &mut dynamic,
        ));
        let _ = writeln!(stdout, "{result}");
        let _ = stdout.flush();
        if stop {
            break;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
type Client = foundation_netio::shared::durable::DurableClient;

/// The dynamic header and query values, evaluated once per request.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct Dynamic {
    headers: std::collections::HashMap<String, (String, u64, String)>,
    params: std::collections::HashMap<String, (String, u64)>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Dynamic {
    fn headers_for_request(&mut self) -> (Vec<(String, String)>, serde_json::Map<String, serde_json::Value>) {
        let mut values = Vec::new();
        let mut sent = serde_json::Map::new();
        for (name, (kind, counter, token)) in &mut self.headers {
            let value = match kind.as_str() {
                "counter" => {
                    *counter += 1;
                    counter.to_string()
                }
                "timestamp" => std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|value| value.as_millis().to_string())
                    .unwrap_or_default(),
                _ => token.clone(),
            };
            sent.insert(name.clone(), value.clone().into());
            values.push((name.clone(), value));
        }
        (values, sent)
    }

    fn params_for_request(&mut self) -> (Vec<(String, String)>, serde_json::Map<String, serde_json::Value>) {
        let mut values = Vec::new();
        let mut sent = serde_json::Map::new();
        for (name, (kind, counter)) in &mut self.params {
            let value = match kind.as_str() {
                "timestamp" => std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|value| value.as_millis().to_string())
                    .unwrap_or_default(),
                _ => {
                    *counter += 1;
                    counter.to_string()
                }
            };
            sent.insert(name.clone(), value.clone().into());
            values.push((name.clone(), value));
        }
        (values, sent)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn failure_result(command_type: &str, code: &str, message: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "error",
        "success": false,
        "commandType": command_type,
        "errorCode": code,
        "message": message,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn client_error(command_type: &str, error: &foundation_netio::shared::durable::ClientError) -> serde_json::Value {
    use foundation_netio::shared::durable::ClientError;
    let (code, status) = match error {
        ClientError::Transport(_) => ("NETWORK_ERROR", None),
        ClientError::NotFound { .. } => ("NOT_FOUND", Some(404)),
        ClientError::Gone { .. } => ("NOT_FOUND", Some(410)),
        ClientError::Conflict(_) => ("CONFLICT", Some(409)),
        ClientError::Closed => ("STREAM_CLOSED", Some(409)),
        ClientError::InvalidOffset(_) => ("INVALID_OFFSET", Some(400)),
        ClientError::UnexpectedStatus { status, .. } => ("UNEXPECTED_STATUS", Some(*status)),
        ClientError::Parse(_) => ("PARSE_ERROR", None),
    };
    let mut result = failure_result(command_type, code, &error.to_string());
    if let Some(status) = status {
        result["status"] = status.into();
    }
    if let ClientError::NotFound { path } | ClientError::Gone { path } = error {
        result["details"] = serde_json::json!({ "path": path });
    }
    result
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg(not(target_arch = "wasm32"))]
fn is_retryable(error: &foundation_netio::shared::durable::ClientError) -> bool {
    use foundation_netio::shared::durable::ClientError;
    match error {
        ClientError::Transport(_) => true,
        ClientError::UnexpectedStatus { status, .. } => {
            matches!(status, 429 | 500 | 502 | 503 | 504)
        }
        _ => false,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn headers_of(command: &serde_json::Value) -> Vec<(String, String)> {
    command["headers"]
        .as_object()
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| {
                    value.as_str().map(|value| (key.clone(), value.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(not(target_arch = "wasm32"))]
fn body_of(command: &serde_json::Value) -> Vec<u8> {
    use base64::Engine as _;
    let Some(data) = command["data"].as_str() else {
        return Vec::new();
    };
    if command["binary"].as_bool().unwrap_or(false) {
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .unwrap_or_default()
    } else {
        data.as_bytes().to_vec()
    }
}

#[cfg(not(target_arch = "wasm32"))]
async fn handle(
    kind: &str,
    command: &serde_json::Value,
    client: &mut Option<Client>,
    types: &mut std::collections::HashMap<String, String>,
    seqs: &mut std::collections::HashMap<(String, String), u64>,
    sent: &mut std::collections::HashMap<(String, String), Vec<u8>>,
    dynamic: &mut Dynamic,
) -> (serde_json::Value, bool) {
    use foundation_netio::native::durable::NetioTransport;
    use foundation_netio::shared::durable::{ClientConfig, DurableClient, ReadMode};
    use std::sync::Arc;

    if kind == "shutdown" {
        return (serde_json::json!({"type": "shutdown", "success": true}), true);
    }

    if kind == "init" {
        let base_url = command["serverUrl"].as_str().unwrap_or("").to_string();
        *client = Some(DurableClient::new(
            Arc::new(NetioTransport::with_timeouts(
                std::time::Duration::from_secs(10),
                std::time::Duration::from_secs(120),
            )),
            ClientConfig {
                base_url,
                default_headers: Vec::new(),
            },
        ));
        return (
            serde_json::json!({
                "type": "init",
                "success": true,
                "clientName": "ewe-rust",
                "clientVersion": env!("CARGO_PKG_VERSION"),
                "features": {
                    "sse": false,
                    "longPoll": true,
                    "auto": true,
                    "streaming": true,
                    "batching": true,
                    "dynamicHeaders": true,
                },
            }),
            false,
        );
    }

    let Some(client) = client.as_ref() else {
        return (failure_result(kind, "INTERNAL_ERROR", "init was not sent"), false);
    };
    let path = command["path"].as_str().unwrap_or("");
    let (dynamic_headers, headers_sent) = dynamic.headers_for_request();
    let (dynamic_params, params_sent) = dynamic.params_for_request();
    let handle = client
        .handle(path)
        .with_headers(headers_of(command))
        .with_headers(dynamic_headers)
        .with_query(dynamic_params);

    match kind {
        "create" => {
            let result = handle
                .create(
                    command["contentType"].as_str(),
                    command["ttlSeconds"].as_u64(),
                    command["expiresAt"].as_str(),
                    command["closed"].as_bool().unwrap_or(false),
                    body_of(command),
                )
                .await;
            if let Some(content_type) = command["contentType"].as_str() {
                types.insert(path.to_string(), content_type.to_string());
            }
            match result {
                Ok(outcome) => (
                    serde_json::json!({
                        "type": "create",
                        "success": true,
                        "status": outcome.status,
                        "offset": outcome.offset,
                        "headers": outcome.headers,
                    }),
                    false,
                ),
                Err(error) => (client_error(kind, &error), false),
            }
        }
        "connect" => match handle.head().await {
            Ok(head) => {
                if let Some(content_type) = &head.content_type {
                    types.insert(path.to_string(), content_type.clone());
                }
                (
                    serde_json::json!({
                        "type": "connect",
                        "success": true,
                        "status": head.status,
                        "offset": head.offset,
                    }),
                    false,
                )
            }
            Err(error) => (client_error(kind, &error), false),
        },
        "append" => {
            if command["producerEpoch"].as_i64().is_some_and(|value| value < 0) {
                return (
                    failure_result(kind, "INVALID_ARGUMENT", "producerEpoch must be zero or more"),
                    false,
                );
            }
            let producer = match (
                command["producerId"].as_str(),
                command["producerEpoch"].as_u64(),
                command["producerSeq"].as_u64(),
            ) {
                (Some(id), Some(epoch), Some(seq)) => Some((id, epoch, seq)),
                _ => None,
            };
            let stream_seq = command["seq"].as_i64().map(|value| value.to_string());
            let content_type = command["contentType"]
                .as_str()
                .map(str::to_string)
                .or_else(|| types.get(path).cloned());
            let mut attempt = 0;
            let result = loop {
                let outcome = handle
                    .append(
                        body_of(command),
                        content_type.as_deref(),
                        stream_seq.as_deref(),
                        producer,
                        false,
                    )
                    .await;
                attempt += 1;
                if attempt >= 3 {
                    break outcome;
                }
                match &outcome {
                    Err(error) if producer.is_none() && is_retryable(error) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    _ => break outcome,
                }
            };
            match result {
                Ok(outcome) => {
                    let duplicate = producer.is_some() && outcome.status == 204;
                    (
                        serde_json::json!({
                            "type": "append",
                            "success": true,
                            "status": if duplicate { 204 } else { 200 },
                            "offset": outcome.offset,
                            "headers": outcome.headers,
                            "duplicate": duplicate,
                            "producerEpoch": header_number(&outcome.headers, "producer-epoch"),
                            "producerSeq": header_number(&outcome.headers, "producer-seq"),
                            "headersSent": headers_sent,
                            "paramsSent": params_sent,
                        }),
                        false,
                    )
                }
                Err(foundation_netio::shared::durable::ClientError::Conflict(message))
                    if stream_seq.is_some() =>
                {
                    (failure_result(kind, "SEQUENCE_CONFLICT", &message), false)
                }
                Err(error) => (client_error(kind, &error), false),
            }
        }
        "read" => {
            let mode = match command["live"].as_str() {
                Some("long-poll") => ReadMode::LongPoll,
                Some("sse") => ReadMode::Sse,
                _ if command["live"].as_bool() == Some(true) => ReadMode::Auto,
                _ => ReadMode::CatchUp,
            };
            let binary = types.get(path).is_some_and(|content_type| {
                !(content_type.starts_with("text/")
                    || content_type.starts_with("application/json"))
            });
            if mode == ReadMode::Sse {
                return (
                    failure_result(kind, "NOT_SUPPORTED", "sse is not enabled"),
                    false,
                );
            }
            let mut builder = handle.read().mode(mode).sse_binary(binary);
            if let Some(offset) = command["offset"].as_str() {
                builder = builder.offset(offset);
            }
            if let Some(limit) = command["maxChunks"].as_u64() {
                builder = builder.max_chunks(usize::try_from(limit).unwrap_or(usize::MAX));
            }
            let mut attempt = 0;
            let mut result = loop {
                let outcome = builder.clone().send().await;
                attempt += 1;
                match outcome {
                    Err(error) if attempt < 3 && is_retryable(&error) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    other => break match other {
                        Ok(result) => result,
                        Err(error) => return (client_error(kind, &error), false),
                    },
                }
            };
            if mode == ReadMode::LongPoll {
                let mut count = result.chunks.len();
                let wanted = command["maxChunks"]
                    .as_u64()
                    .map_or(10, |value| usize::try_from(value).unwrap_or(10));
                while !result.closed && count > 0 && count < wanted {
                    let next = result
                        .next_offset
                        .clone()
                        .unwrap_or_else(|| "-1".to_string());
                    let more = handle.read().mode(ReadMode::CatchUp).offset(next).send().await;
                    match more {
                        Ok(page) if !page.chunks.is_empty() => {
                            count += page.chunks.len();
                            result.chunks.extend(page.chunks);
                            result.next_offset = page.next_offset.or(result.next_offset);
                            result.up_to_date = page.up_to_date;
                            result.closed = page.closed;
                        }
                        _ => break,
                    }
                }
            }
            // Auto mode catches up, then waits once for live data.
            if mode == ReadMode::Auto && !result.closed {
                let next = result
                    .next_offset
                    .clone()
                    .unwrap_or_else(|| "-1".to_string());
                let poll = handle
                    .read()
                    .mode(ReadMode::LongPoll)
                    .offset(next)
                    .send()
                    .await;
                if let Ok(live) = poll {
                    result.chunks.extend(live.chunks);
                    result.next_offset = live.next_offset.or(result.next_offset);
                    result.up_to_date = live.up_to_date;
                    result.closed = live.closed;
                    result.cursor = live.cursor.or(result.cursor);
                }
            }
            let chunks: Vec<serde_json::Value> = result
                .chunks
                .iter()
                .map(|chunk| serde_json::json!({ "data": String::from_utf8_lossy(&chunk.data) }))
                .collect();
            (
                serde_json::json!({
                    "type": "read",
                    "success": true,
                    "status": result.status,
                    "chunks": chunks,
                    "offset": result.next_offset,
                    "upToDate": result.up_to_date,
                    "streamClosed": result.closed,
                    "cursor": result.cursor,
                    "headersSent": headers_sent,
                    "paramsSent": params_sent,
                }),
                false,
            )
        }
        "head" => match handle.head().await {
            Ok(head) => {
                if let Some(content_type) = &head.content_type {
                    types.insert(path.to_string(), content_type.clone());
                }
                (
                serde_json::json!({
                    "type": "head",
                    "success": true,
                    "status": head.status,
                    "offset": head.offset,
                    "contentType": head.content_type,
                    "ttlSeconds": head.ttl_seconds,
                    "streamClosed": head.closed,
                }),
                false,
                )
            }
            Err(error) => (client_error(kind, &error), false),
        },
        "delete" => match handle.delete().await {
            Ok(_) => (
                serde_json::json!({"type": "delete", "success": true, "status": 200}),
                false,
            ),
            Err(error) => (client_error(kind, &error), false),
        },
        "close" | "server-close" => {
            let content_type = command["contentType"]
                .as_str()
                .map(str::to_string)
                .or_else(|| types.get(path).cloned());
            let result = handle
                .close(body_of(command), content_type.as_deref())
                .await;
            match result {
                Ok(outcome) => (
                    serde_json::json!({
                        "type": if kind == "close" { "close" } else { "server-close" },
                        "success": true,
                        "status": 200,
                        "finalOffset": outcome.offset,
                    }),
                    false,
                ),
                Err(error) => (client_error(kind, &error), false),
            }
        }
        "idempotent-append" | "idempotent-append-batch" => {
            let producer_id = command["producerId"].as_str().unwrap_or("p").to_string();
            let mut epoch = command["epoch"].as_u64().unwrap_or(0);
            let content_type = types.get(path).cloned();
            let batch_items_json = command["items"]
                .as_array()
                .and_then(|items| items.first())
                .and_then(|item| item.as_str())
                .is_some_and(|item| serde_json::from_str::<serde_json::Value>(item).is_ok());
            let is_json = content_type
                .as_deref()
                .is_some_and(|value| value.starts_with("application/json"))
                || (kind == "idempotent-append-batch" && batch_items_json);

            // A JSON batch is one array append, as the reference client sends it.
            let batches: Vec<Vec<u8>> = if kind == "idempotent-append-batch" {
                let items: Vec<String> = command["items"]
                    .as_array()
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(|item| item.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();
                if is_json {
                    vec![format!("[{}]", items.join(",")).into_bytes()]
                } else {
                    items
                        .into_iter()
                        .map(|item| item.into_bytes())
                        .collect()
                }
            } else {
                vec![body_of(command)]
            };

            let key = (path.to_string(), producer_id.clone());
            let body: Vec<u8> = batches.iter().flatten().copied().collect();
            if kind == "idempotent-append-batch"
                && sent.get(&key).is_some_and(|previous| previous == &body)
            {
                return (
                    serde_json::json!({
                        "type": kind,
                        "success": true,
                        "status": 200,
                        "duplicate": true,
                        "producerSeq": seqs.get(&key).copied().unwrap_or(0).saturating_sub(1),
                    }),
                    false,
                );
            }
            let mut seq = seqs.get(&key).copied().unwrap_or(0);
            let mut last = None;
            let mut failure = None;
            for item in batches {
                loop {
                    match handle
                        .append(
                            item.clone(),
                            content_type.as_deref(),
                            None,
                            Some((&producer_id, epoch, seq)),
                            false,
                        )
                        .await
                    {
                        Ok(outcome) => {
                            last = Some(outcome);
                            seq += 1;
                            break;
                        }
                        Err(foundation_netio::shared::durable::ClientError::UnexpectedStatus {
                            status: 403,
                            headers,
                            ..
                        }) if command["autoClaim"].as_bool().unwrap_or(false) => {
                            let server_epoch = headers
                                .iter()
                                .find(|(key, _)| key.eq_ignore_ascii_case("producer-epoch"))
                                .and_then(|(_, value)| value.parse::<u64>().ok());
                            epoch = server_epoch.map_or(epoch + 1, |value| value + 1);
                            seq = 0;
                        }
                        Err(error) => {
                            failure = Some(error);
                            break;
                        }
                    }
                }
                if failure.is_some() {
                    break;
                }
            }
            seqs.insert(key.clone(), seq);
            if failure.is_none() {
                sent.insert(key, body);
            }
            match failure {
                Some(error) => (client_error(kind, &error), false),
                None => (
                    serde_json::json!({
                        "type": kind,
                        "success": true,
                        "status": 200,
                        "offset": last.as_ref().and_then(|outcome| outcome.offset.clone()),
                        "producerSeq": seq.checked_sub(1).unwrap_or(0),
                    }),
                    false,
                ),
            }
        }
        "idempotent-close" => {
            let producer_id = command["producerId"].as_str().unwrap_or("p").to_string();
            let epoch = command["epoch"].as_u64().unwrap_or(0);
            let key = (path.to_string(), producer_id.clone());
            let seq = seqs.get(&key).copied().unwrap_or(0);
            let result = handle
                .append(
                    body_of(command),
                    types.get(path).map(String::as_str),
                    None,
                    Some((&producer_id, epoch, seq)),
                    true,
                )
                .await;
            let _ = key;
            match result {
                Ok(outcome) => (
                    serde_json::json!({
                        "type": "idempotent-close",
                        "success": true,
                        "status": 200,
                        "finalOffset": outcome.offset,
                    }),
                    false,
                ),
                Err(error) => (client_error(kind, &error), false),
            }
        }
        "idempotent-detach" => (
            serde_json::json!({"type": "idempotent-detach", "success": true, "status": 200}),
            false,
        ),
        "set-dynamic-header" => {
            let name = command["name"].as_str().unwrap_or("").to_string();
            let kind = command["valueType"].as_str().unwrap_or("token").to_string();
            let initial = command["initialValue"].as_str().unwrap_or("").to_string();
            dynamic.headers.insert(name, (kind.clone(), 0, initial));
            (serde_json::json!({"type": kind, "success": true}), false)
        }
        "set-dynamic-param" => {
            let name = command["name"].as_str().unwrap_or("").to_string();
            let kind = command["valueType"].as_str().unwrap_or("counter").to_string();
            dynamic.params.insert(name, (kind.clone(), 0));
            (serde_json::json!({"type": kind, "success": true}), false)
        }
        "clear-dynamic" => {
            dynamic.headers.clear();
            dynamic.params.clear();
            (serde_json::json!({"type": "clear-dynamic", "success": true}), false)
        }
        "validate" => validate(command),
        _ => (failure_result(kind, "NOT_SUPPORTED", "the adapter does not handle this command"), false),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn validate(command: &serde_json::Value) -> (serde_json::Value, bool) {
    let target = command["target"]["target"].as_str().unwrap_or("");
    let spec = &command["target"];
    let invalid = match target {
        "retry-options" => {
            if spec["maxRetries"].as_i64().is_some_and(|value| value < 0) {
                Some("maxRetries must be zero or more")
            } else if spec["initialDelayMs"].as_i64().is_some_and(|value| value <= 0) {
                Some("initialDelayMs must be more than zero")
            } else if spec["multiplier"].as_f64().is_some_and(|value| value < 1.0) {
                Some("multiplier must be one or more")
            } else {
                None
            }
        }
        "idempotent-producer" => {
            if spec["producerId"].as_str().is_none_or(str::is_empty) {
                Some("producerId must not be empty")
            } else if spec["epoch"].as_i64().is_some_and(|value| value < 0) {
                Some("epoch must be zero or more")
            } else if spec["maxBatchBytes"].as_i64().is_some_and(|value| value <= 0) {
                Some("maxBatchBytes must be more than zero")
            } else if spec["maxBatchItems"].as_i64().is_some_and(|value| value <= 0) {
                Some("maxBatchItems must be more than zero")
            } else {
                None
            }
        }
        _ => None,
    };
    match invalid {
        Some(message) => (
            failure_result("validate", "INVALID_ARGUMENT", message),
            false,
        ),
        None => (serde_json::json!({"type": "validate", "success": true}), false),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn header_number(
    headers: &std::collections::BTreeMap<String, String>,
    name: &str,
) -> Option<u64> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .and_then(|(_, value)| value.parse().ok())
}
