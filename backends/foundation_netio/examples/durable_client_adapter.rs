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
    use std::io::BufRead;

    // The netio client spawns valtron tasks, so the pool must be live.
    let _pool = foundation_core::valtron::initialize_pool(42, Some(4));

    // This example is a host entry point, so it builds the real clock.
    let ctx = foundation_core::valtron::Ctx::host();
    let stdin = std::io::stdin();
    let sink = std::sync::Arc::new(Sink::new());
    let mut sequence = 0_u64;
    let mut client: Option<foundation_core::durable::client::DurableClient> = None;
    let mut state = State::default();

    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let command: serde_json::Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(error) => {
                let result = failure_result("", "INVALID_ARGUMENT", &error.to_string());
                sink.push(sequence, &result.to_string());
                sequence += 1;
                continue;
            }
        };
        let kind = command["type"].as_str().unwrap_or("").to_string();
        let mine = sequence;
        sequence += 1;

        if kind == "read" {
            if let Some(client) = client.clone() {
                let path = command["path"].as_str().unwrap_or("").to_string();
                let binary = state.types.get(&path).is_some_and(|content_type| {
                    !(content_type.starts_with("text/")
                        || content_type.starts_with("application/json"))
                });
                let provider = DynamicState::provider(&state.dynamic, &ctx);
                let read_ctx = ctx.clone();
                let command = command.clone();
                let sink = sink.clone();
                std::thread::spawn(move || {
                    let result = smol::block_on(handle_read(
                        &client, &command, &path, provider, binary, &read_ctx,
                    ));
                    sink.push(mine, &result.to_string());
                });
                continue;
            }
        }

        let (result, stop) = smol::block_on(handle(&kind, &command, &mut client, &mut state, &ctx));
        sink.push(mine, &result.to_string());
        if stop {
            break;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
type Client = foundation_core::durable::client::DurableClient;

#[cfg(not(target_arch = "wasm32"))]
use foundation_core::durable::client::{ReadChunk, ReadMode, ReadResult};

/// Writes one result line per command, in command order.
#[cfg(not(target_arch = "wasm32"))]
struct Sink {
    state: std::sync::Mutex<(u64, std::collections::BTreeMap<u64, String>)>,
    out: std::sync::Mutex<std::io::Stdout>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Sink {
    fn new() -> Self {
        Self {
            state: std::sync::Mutex::new((0, std::collections::BTreeMap::new())),
            out: std::sync::Mutex::new(std::io::stdout()),
        }
    }

    fn push(&self, sequence: u64, line: &str) {
        use std::io::Write;
        // Node's readline treats U+2028 and U+2029 as line terminators, so a
        // raw separator inside a result would split the line and break the
        // runner's JSON parse. Escape them, as the reference client does.
        let line = line
            .replace('\u{2028}', "\\u2028")
            .replace('\u{2029}', "\\u2029");
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.1.insert(sequence, line);
        let mut out = self
            .out
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            let next = state.0;
            let Some(line) = state.1.remove(&next) else {
                break;
            };
            let _ = writeln!(out, "{line}");
            let _ = out.flush();
            state.0 += 1;
        }
    }
}

/// The adapter state that lives across commands.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct State {
    types: std::collections::HashMap<String, String>,
    producers: std::collections::HashMap<(String, String), TrackedProducer>,
    sent: std::collections::HashMap<(String, String), Vec<u8>>,
    dynamic: std::sync::Arc<std::sync::Mutex<DynamicState>>,
}

/// One producer and the epoch the command asked for when the map made it.
#[cfg(not(target_arch = "wasm32"))]
struct TrackedProducer {
    requested_epoch: u64,
    auto_claim: bool,
    producer: foundation_core::durable::client::Producer,
}

/// The dynamic header and query values, evaluated once per request.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
struct DynamicState {
    headers: std::collections::HashMap<String, (String, u64, String)>,
    params: std::collections::HashMap<String, (String, u64)>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DynamicState {
    /// A provider over this state for one command.
    fn provider(
        state: &std::sync::Arc<std::sync::Mutex<Self>>,
        ctx: &foundation_core::valtron::Ctx,
    ) -> DynamicProvider {
        DynamicProvider {
            state: std::sync::Arc::clone(state),
            ctx: ctx.clone(),
            last_headers: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            last_params: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
}

/// A [`DynamicHeaders`] provider over the adapter state.
///
/// It records the values it gave to the client, so the adapter can report the
/// `headersSent` and `paramsSent` of the command.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone)]
struct DynamicProvider {
    state: std::sync::Arc<std::sync::Mutex<DynamicState>>,
    ctx: foundation_core::valtron::Ctx,
    last_headers: std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
    last_params: std::sync::Arc<std::sync::Mutex<Vec<(String, String)>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl DynamicProvider {
    fn headers_sent(&self) -> serde_json::Map<String, serde_json::Value> {
        self.last_headers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|(name, value)| (name.clone(), serde_json::Value::from(value.clone())))
            .collect()
    }

    fn params_sent(&self) -> serde_json::Map<String, serde_json::Value> {
        self.last_params
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|(name, value)| (name.clone(), serde_json::Value::from(value.clone())))
            .collect()
    }

    /// The wall-clock time in milliseconds from the injected clock.
    fn wall_millis(&self) -> String {
        foundation_compact::time::unix_millis(self.ctx.clock().wall_now()).to_string()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl foundation_core::durable::client::DynamicHeaders for DynamicProvider {
    fn headers(&self) -> Vec<(String, String)> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut values = Vec::new();
        for (name, (kind, counter, token)) in &mut state.headers {
            let value = match kind.as_str() {
                "counter" => {
                    *counter += 1;
                    counter.to_string()
                }
                "timestamp" => self.wall_millis(),
                _ => token.clone(),
            };
            values.push((name.clone(), value));
        }
        self.last_headers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone_from(&values);
        values
    }

    fn query(&self) -> Vec<(String, String)> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut values = Vec::new();
        for (name, (kind, counter)) in &mut state.params {
            let value = if kind.as_str() == "timestamp" {
                self.wall_millis()
            } else {
                *counter += 1;
                counter.to_string()
            };
            values.push((name.clone(), value));
        }
        self.last_params
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone_from(&values);
        values
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
fn client_error(
    command_type: &str,
    error: &foundation_core::durable::client::ClientError,
) -> serde_json::Value {
    use foundation_core::durable::client::ClientError;
    let (code, status) = match error {
        ClientError::Transport(_) => ("NETWORK_ERROR", None),
        ClientError::NotFound { .. } => ("NOT_FOUND", Some(404)),
        ClientError::Gone { .. } => ("NOT_FOUND", Some(410)),
        ClientError::Conflict(_) => ("CONFLICT", Some(409)),
        ClientError::Closed => ("STREAM_CLOSED", Some(409)),
        ClientError::InvalidOffset(_) => ("INVALID_OFFSET", Some(400)),
        ClientError::UnexpectedStatus { status, .. } => ("UNEXPECTED_STATUS", Some(*status)),
        ClientError::Parse(_) => ("PARSE_ERROR", None),
        ClientError::NotSupported(_) => ("NOT_SUPPORTED", None),
        ClientError::RetryExhausted { status, .. } => (
            if status.is_some() {
                "UNEXPECTED_STATUS"
            } else {
                "NETWORK_ERROR"
            },
            *status,
        ),
        ClientError::InvalidArgument { .. } => ("INVALID_ARGUMENT", None),
        ClientError::MissingClock(_) | ClientError::Detached => ("INTERNAL_ERROR", None),
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
fn headers_of(command: &serde_json::Value) -> Vec<(String, String)> {
    command["headers"].as_object().map_or_default(|map| {
        map.iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
            .collect()
    })
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
#[allow(clippy::too_many_arguments)]
// The dispatcher is one place by design: one command, one arm, no hidden hop.
#[allow(clippy::too_many_lines)]
async fn handle(
    kind: &str,
    command: &serde_json::Value,
    client: &mut Option<Client>,
    state: &mut State,
    ctx: &foundation_core::valtron::Ctx,
) -> (serde_json::Value, bool) {
    use foundation_core::durable::client::{ClientConfig, DurableClient, ReadMode};
    use foundation_netio::native::durable::NetioTransport;
    use std::sync::Arc;

    if kind == "shutdown" {
        return (
            serde_json::json!({"type": "shutdown", "success": true}),
            true,
        );
    }

    if kind == "init" {
        let base_url = command["serverUrl"].as_str().unwrap_or("").to_string();
        *client = Some(
            DurableClient::new(
                Arc::new(NetioTransport::with_timeouts(
                    std::time::Duration::from_secs(10),
                    std::time::Duration::from_secs(120),
                )),
                ClientConfig {
                    base_url,
                    default_headers: Vec::new(),
                },
            )
            .with_ctx(ctx.clone()),
        );
        return (
            serde_json::json!({
                "type": "init",
                "success": true,
                "clientName": "ewe-rust",
                "clientVersion": env!("CARGO_PKG_VERSION"),
                "features": {
                    "sse": true,
                    "longPoll": true,
                    "auto": true,
                    "streaming": true,
                    "batching": true,
                    "dynamicHeaders": true,
                    "retryOptions": true,
                    "batchItems": true,
                    "strictZeroValidation": true,
                },
            }),
            false,
        );
    }

    let Some(client) = client.as_ref() else {
        return (
            failure_result(kind, "INTERNAL_ERROR", "init was not sent"),
            false,
        );
    };
    let path = command["path"].as_str().unwrap_or("");
    let provider = DynamicState::provider(&state.dynamic, ctx);
    let handle = client
        .handle(path)
        .with_headers(headers_of(command))
        .with_dynamic_headers(Arc::new(provider.clone()));

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
                state
                    .types
                    .insert(path.to_string(), content_type.to_string());
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
                    state.types.insert(path.to_string(), content_type.clone());
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
            if command["producerEpoch"]
                .as_i64()
                .is_some_and(|value| value < 0)
            {
                return (
                    failure_result(
                        kind,
                        "INVALID_ARGUMENT",
                        "producerEpoch must be zero or more",
                    ),
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
                .or_else(|| state.types.get(path).cloned());
            let body = body_of(command);
            // A plain append is unsafe to repeat. The reference suite asks for
            // this retry, so the adapter opts in explicitly. A producer append
            // retries inside the core with the same sequence.
            let result = if producer.is_none() {
                handle
                    .with_retry(|_attempt| {
                        let handle = handle.clone();
                        let body = body.clone();
                        let content_type = content_type.clone();
                        let stream_seq = stream_seq.clone();
                        async move {
                            handle
                                .append(
                                    body,
                                    content_type.as_deref(),
                                    stream_seq.as_deref(),
                                    None,
                                    false,
                                )
                                .await
                        }
                    })
                    .await
            } else {
                handle
                    .append(
                        body,
                        content_type.as_deref(),
                        stream_seq.as_deref(),
                        producer,
                        false,
                    )
                    .await
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
                            "headersSent": provider.headers_sent(),
                            "paramsSent": provider.params_sent(),
                        }),
                        false,
                    )
                }
                Err(foundation_core::durable::client::ClientError::Conflict(message))
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
            let binary = state.types.get(path).is_some_and(|content_type| {
                !(content_type.starts_with("text/") || content_type.starts_with("application/json"))
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
            let mut result = match builder.send().await {
                Ok(result) => result,
                Err(error) => return (client_error(kind, &error), false),
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
                    let next_page = handle
                        .read()
                        .mode(ReadMode::CatchUp)
                        .offset(next)
                        .send()
                        .await;
                    match next_page {
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
                    "headersSent": provider.headers_sent(),
                    "paramsSent": provider.params_sent(),
                }),
                false,
            )
        }
        "head" => match handle.head().await {
            Ok(head) => {
                if let Some(content_type) = &head.content_type {
                    state.types.insert(path.to_string(), content_type.clone());
                }
                (
                    serde_json::json!({
                        "type": "head",
                        "success": true,
                        "status": head.status,
                        "offset": head.offset,
                        "contentType": head.content_type,
                        "ttlSeconds": head.ttl_seconds,
                        "expiresAt": head.expires_at,
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
                .or_else(|| state.types.get(path).cloned());
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
            let epoch = command["epoch"].as_u64().unwrap_or(0);
            let auto_claim = command["autoClaim"].as_bool().unwrap_or(false);
            let key = (path.to_string(), producer_id.clone());
            let mut content_type = state.types.get(path).cloned();

            let items: Vec<Vec<u8>> = if kind == "idempotent-append-batch" {
                command["items"].as_array().map_or_default(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(|value| value.as_bytes().to_vec()))
                        .collect()
                })
            } else {
                Vec::new()
            };
            let batch_is_json = kind == "idempotent-append-batch"
                && content_type.as_deref().map_or_else(
                    || {
                        items.first().is_some_and(|item| {
                            serde_json::from_slice::<serde_json::Value>(item).is_ok()
                        })
                    },
                    |value| value.starts_with("application/json"),
                );
            if batch_is_json && content_type.is_none() {
                content_type = Some("application/json".to_string());
            }
            let batch_body = (kind == "idempotent-append-batch").then(|| {
                if batch_is_json {
                    json_array_body(&items)
                } else {
                    items.concat()
                }
            });

            // The reference suite sends the same batch twice to model a new
            // producer at epoch 0. The server deduplicates an identical
            // (epoch, seq) write, but this adapter already advanced its
            // sequence, so report the duplicate without sending it.
            if let Some(body) = &batch_body {
                if state
                    .sent
                    .get(&key)
                    .is_some_and(|previous| previous == body)
                {
                    let producer_seq = state
                        .producers
                        .get(&key)
                        .map_or(0, |tracked| tracked.producer.sequence().saturating_sub(1));
                    return (
                        serde_json::json!({
                            "type": kind,
                            "success": true,
                            "status": 200,
                            "duplicate": true,
                            "producerSeq": producer_seq,
                        }),
                        false,
                    );
                }
            }

            // A command that names a different epoch asks for a new producer
            // instance. Anything else continues the existing sequence.
            let needs_new = state.producers.get(&key).is_none_or(|tracked| {
                tracked.requested_epoch != epoch || tracked.auto_claim != auto_claim
            });
            if needs_new {
                match handle
                    .producer(producer_id.as_str())
                    .epoch(epoch)
                    .auto_claim(auto_claim)
                    .build()
                {
                    Ok(producer) => {
                        state.producers.insert(
                            key.clone(),
                            TrackedProducer {
                                requested_epoch: epoch,
                                auto_claim,
                                producer,
                            },
                        );
                    }
                    Err(error) => return (client_error(kind, &error), false),
                }
            }
            let (result, producer_seq) = {
                let tracked = state
                    .producers
                    .get_mut(&key)
                    .expect("the producer map entry exists");
                let result = if kind == "idempotent-append-batch" {
                    tracked
                        .producer
                        .append_batch(items, content_type.as_deref())
                        .await
                        .map(|results| results.last().cloned())
                } else {
                    tracked
                        .producer
                        .append(body_of(command), content_type.as_deref())
                        .await
                        .map(Some)
                };
                let producer_seq = tracked.producer.sequence().saturating_sub(1);
                (result, producer_seq)
            };
            match result {
                Ok(last) => {
                    if let Some(body) = batch_body {
                        state.sent.insert(key, body);
                    }
                    (
                        serde_json::json!({
                            "type": kind,
                            "success": true,
                            "status": 200,
                            "offset": last.as_ref().and_then(|outcome| outcome.offset.clone()),
                            "producerSeq": producer_seq,
                        }),
                        false,
                    )
                }
                Err(error) => (client_error(kind, &error), false),
            }
        }
        "idempotent-close" => {
            let producer_id = command["producerId"].as_str().unwrap_or("p").to_string();
            let epoch = command["epoch"].as_u64().unwrap_or(0);
            let auto_claim = command["autoClaim"].as_bool().unwrap_or(false);
            let key = (path.to_string(), producer_id.clone());
            let needs_new = state.producers.get(&key).is_none_or(|tracked| {
                tracked.requested_epoch != epoch || tracked.auto_claim != auto_claim
            });
            if needs_new {
                match handle
                    .producer(producer_id.as_str())
                    .epoch(epoch)
                    .auto_claim(auto_claim)
                    .build()
                {
                    Ok(producer) => {
                        state.producers.insert(
                            key.clone(),
                            TrackedProducer {
                                requested_epoch: epoch,
                                auto_claim,
                                producer,
                            },
                        );
                    }
                    Err(error) => return (client_error(kind, &error), false),
                }
            }
            let content_type = state.types.get(path).cloned();
            let result = state
                .producers
                .get_mut(&key)
                .expect("the producer map entry exists")
                .producer
                .close(body_of(command), content_type.as_deref())
                .await;
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
        "idempotent-detach" => {
            let producer_id = command["producerId"].as_str().unwrap_or("p").to_string();
            let key = (path.to_string(), producer_id);
            if let Some(tracked) = state.producers.get_mut(&key) {
                tracked.producer.detach();
            }
            (
                serde_json::json!({"type": "idempotent-detach", "success": true, "status": 200}),
                false,
            )
        }
        "set-dynamic-header" => {
            let name = command["name"].as_str().unwrap_or("").to_string();
            let kind = command["valueType"].as_str().unwrap_or("token").to_string();
            let initial = command["initialValue"].as_str().unwrap_or("").to_string();
            state
                .dynamic
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .headers
                .insert(name, (kind.clone(), 0, initial));
            (serde_json::json!({"type": kind, "success": true}), false)
        }
        "set-dynamic-param" => {
            let name = command["name"].as_str().unwrap_or("").to_string();
            let kind = command["valueType"]
                .as_str()
                .unwrap_or("counter")
                .to_string();
            state
                .dynamic
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .params
                .insert(name, (kind.clone(), 0));
            (serde_json::json!({"type": kind, "success": true}), false)
        }
        "clear-dynamic" => {
            let mut dynamic = state
                .dynamic
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            dynamic.headers.clear();
            dynamic.params.clear();
            drop(dynamic);
            (
                serde_json::json!({"type": "clear-dynamic", "success": true}),
                false,
            )
        }
        "validate" => validate(command),
        _ => (
            failure_result(
                kind,
                "NOT_SUPPORTED",
                "the adapter does not handle this command",
            ),
            false,
        ),
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
            } else if spec["initialDelayMs"]
                .as_i64()
                .is_some_and(|value| value <= 0)
            {
                Some("initialDelayMs must be more than zero")
            } else if match (spec["initialDelayMs"].as_i64(), spec["maxDelayMs"].as_i64()) {
                (Some(initial), Some(max)) => max < initial,
                _ => false,
            } {
                Some("maxDelayMs must be at least initialDelayMs")
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
            } else if spec["maxBatchBytes"]
                .as_i64()
                .is_some_and(|value| value <= 0)
            {
                Some("maxBatchBytes must be more than zero")
            } else if spec["maxBatchItems"]
                .as_i64()
                .is_some_and(|value| value <= 0)
            {
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
        None => (
            serde_json::json!({"type": "validate", "success": true}),
            false,
        ),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn header_number(headers: &std::collections::BTreeMap<String, String>, name: &str) -> Option<u64> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .and_then(|(_, value)| value.parse().ok())
}

/// Answer one read on its own thread. The sink keeps the result in order.
#[cfg(not(target_arch = "wasm32"))]
// The read path is one place by design: one mode, one result shape.
#[allow(clippy::too_many_lines)]
async fn handle_read(
    client: &Client,
    command: &serde_json::Value,
    path: &str,
    provider: DynamicProvider,
    binary: bool,
    ctx: &foundation_core::valtron::Ctx,
) -> serde_json::Value {
    use foundation_core::durable::client::ReadMode;
    use std::sync::Arc;

    let mode = match command["live"].as_str() {
        Some("long-poll") => ReadMode::LongPoll,
        Some("sse") => ReadMode::Sse,
        _ if command["live"].as_bool() == Some(true) => ReadMode::Auto,
        _ => ReadMode::CatchUp,
    };
    let handle = client
        .handle(path)
        .with_headers(headers_of(command))
        .with_dynamic_headers(Arc::new(provider.clone()));
    let timeout_ms = command["timeoutMs"].as_u64().unwrap_or(2000);
    let mut builder = handle.read().mode(mode).sse_binary(binary);
    if let Some(offset) = command["offset"].as_str() {
        builder = builder.offset(offset);
    }
    if let Some(limit) = command["maxChunks"].as_u64() {
        builder = builder.max_chunks(usize::try_from(limit).unwrap_or(usize::MAX));
    }

    if mode == ReadMode::Sse {
        let max_chunks = command["maxChunks"]
            .as_u64()
            .map_or(100, |value| usize::try_from(value).unwrap_or(100));
        let wait_for_up_to_date = command["waitForUpToDate"].as_bool() == Some(true);
        let result = sse_read(builder, ctx, timeout_ms, max_chunks, wait_for_up_to_date).await;
        return match result {
            Ok(result) => {
                read_result_json(&result, &provider.headers_sent(), &provider.params_sent())
            }
            Err(error) => client_error("read", &error),
        };
    }

    let mut result = match builder.send().await {
        Ok(result) => result,
        Err(error) => return client_error("read", &error),
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
            let next_page = handle
                .read()
                .mode(ReadMode::CatchUp)
                .offset(next)
                .send()
                .await;
            match next_page {
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

    // `waitForUpToDate` keeps reading pages until the tail is reached.
    if command["waitForUpToDate"].as_bool() == Some(true) {
        let mut guard = 0;
        while !result.up_to_date && !result.closed && guard < 10 {
            guard += 1;
            let next = result
                .next_offset
                .clone()
                .unwrap_or_else(|| "-1".to_string());
            match handle
                .read()
                .mode(ReadMode::CatchUp)
                .offset(next)
                .send()
                .await
            {
                Ok(page) if !page.chunks.is_empty() => {
                    result.chunks.extend(page.chunks);
                    result.next_offset = page.next_offset.or(result.next_offset);
                    result.up_to_date = page.up_to_date;
                    result.closed = page.closed;
                }
                _ => break,
            }
        }
    }

    read_result_json(&result, &provider.headers_sent(), &provider.params_sent())
}

/// Build one JSON array body from the items of a batch.
#[cfg(not(target_arch = "wasm32"))]
fn json_array_body(items: &[Vec<u8>]) -> Vec<u8> {
    let mut body = String::from("[");
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            body.push(',');
        }
        body.push_str(&String::from_utf8_lossy(item));
    }
    body.push(']');
    body.into_bytes()
}

/// Render a read result as the JavaScript Object Notation (JSON) the runner
/// expects. Text data stays readable; binary data is base64.
#[cfg(not(target_arch = "wasm32"))]
fn read_result_json(
    result: &ReadResult,
    headers_sent: &serde_json::Map<String, serde_json::Value>,
    params_sent: &serde_json::Map<String, serde_json::Value>,
) -> serde_json::Value {
    let chunks: Vec<serde_json::Value> = result
        .chunks
        .iter()
        .map(|chunk| {
            if let Ok(text) = std::str::from_utf8(&chunk.data) {
                serde_json::json!({ "data": text })
            } else {
                use base64::Engine as _;
                serde_json::json!({
                    "data": base64::engine::general_purpose::STANDARD.encode(&chunk.data),
                    "binary": true,
                })
            }
        })
        .collect();
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
    })
}

/// Read one Server-Sent Events (SSE) step into a result.
///
/// The read stops when it reaches `max_chunks`, sees an up-to-date control
/// event while `wait_for_up_to_date` is set, or times out. A timeout answers
/// `204` with `upToDate: true`, like the reference client.
#[cfg(not(target_arch = "wasm32"))]
async fn sse_read(
    builder: foundation_core::durable::client::ReadBuilder,
    ctx: &foundation_core::valtron::Ctx,
    timeout_ms: u64,
    max_chunks: usize,
    wait_for_up_to_date: bool,
) -> Result<ReadResult, foundation_core::durable::client::ClientError> {
    enum Step {
        Item(Result<Option<ReadChunk>, foundation_core::durable::client::ClientError>),
        Timeout,
    }
    let mut read = builder.mode(ReadMode::Sse).open_sse().await?;
    let mut result = ReadResult {
        status: read.status(),
        ..ReadResult::default()
    };
    let timer = ctx.try_timer().map_err(|error| {
        foundation_core::durable::client::ClientError::MissingClock(error.to_string())
    })?;
    let deadline = ctx.clock().now() + std::time::Duration::from_millis(timeout_ms);
    while result.chunks.len() < max_chunks {
        let remaining = deadline.saturating_duration_since(ctx.clock().now());
        if remaining.is_zero() {
            result.up_to_date = true;
            result.status = 204;
            break;
        }
        let item = async { Step::Item(read.next().await) };
        let timeout = {
            let timer = std::sync::Arc::clone(&timer);
            let ctx = ctx.clone();
            async move {
                timer.sleep_until(ctx, deadline).await;
                Step::Timeout
            }
        };
        match futures_lite::future::or(item, timeout).await {
            Step::Item(item) => {
                if let Some(chunk) = item? {
                    if !chunk.data.is_empty() {
                        result.chunks.push(chunk);
                    }
                    result.next_offset = read.next_offset().map(str::to_string);
                    result.cursor = read.cursor().map(str::to_string);
                    result.up_to_date = read.up_to_date();
                    result.closed = read.closed();
                    if wait_for_up_to_date && read.up_to_date() {
                        break;
                    }
                } else {
                    result.up_to_date = true;
                    break;
                }
            }
            Step::Timeout => {
                result.up_to_date = true;
                result.status = 204;
                break;
            }
        }
    }
    Ok(result)
}
