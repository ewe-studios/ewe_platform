//! The control API's client of the sandbox Durable Objects (DOs).
//!
//! The demo Worker holds the DO classes of `infrastructure/sandbox_worker`.
//! This module sends the Connect JSON calls that the classes answer and turns
//! their streamed frames into plain text for the page and the live test. It
//! never reads a whole stream into memory.

use std::pin::Pin;
use std::task::{Context, Poll};

use foundation_sandbox::shared::ids::SandboxId;
use foundation_sandbox::shared::rpc::messages::{
    CreateEvent, CreateRequest, ExecEventKind, ExecEventMsg, FileChunk, FileReadRequest,
    FileWriteBeginRequest, FileWriteChunkRequest, FileWriteCommitRequest, ListResponse, SandboxRef,
    Scoped,
};
use foundation_sandbox::shared::spec::{RootFs, SandboxSpec};
use futures::{Stream, StreamExt};
use serde::de::DeserializeOwned;
use worker::{Env, Request, Response};

use crate::config::DemoConfig;

/// The binding of the sandbox DO.
pub const SANDBOX_BINDING: &str = "SANDBOX";
/// The binding of the index DO.
pub const INDEX_BINDING: &str = "SANDBOX_INDEX";
/// The internal header that names the sandbox of a scoped call.
pub const SANDBOX_ID_HEADER: &str = "x-ewe-sandbox-id";
/// The `SandboxService` prefix.
const SANDBOX_PREFIX: &str = "/ewe.sandbox.control.v1.SandboxService/";
/// The Connect streaming content type.
const CONNECT_JSON: &str = "application/connect+json";
/// The Connect unary content type.
const JSON: &str = "application/json";

/// One sandbox of the index, as the page shows it.
#[derive(Clone, Debug)]
pub struct Sandbox {
    /// The identifier.
    pub id: SandboxId,
    /// The state text.
    pub state: String,
    /// When the sandbox was created (Unix milliseconds).
    pub created_unix_ms: u64,
}

/// The sandboxes of the index, in creation order.
///
/// # Errors
/// The text of a failed call.
pub async fn list(env: &Env, config: &DemoConfig) -> Result<Vec<Sandbox>, String> {
    let response = forward(
        env,
        INDEX_BINDING,
        "index",
        &format!("{SANDBOX_PREFIX}List"),
        JSON,
        b"{}",
        None,
    )
    .await?;
    let response = unary_value(response, config.error_text_max_bytes).await?;
    let listed: ListResponse = serde_json::from_value(response)
        .map_err(|error| format!("the List answer is not a sandbox list: {error}"))?;
    let mut sandboxes: Vec<Sandbox> = listed
        .sandboxes
        .into_iter()
        .filter_map(|info| {
            let id = info.id.parse::<SandboxId>().ok()?;
            Some(Sandbox {
                id,
                state: state_text(&info.state),
                created_unix_ms: info.created_unix_ms,
            })
        })
        .collect();
    sandboxes.sort_by(|left, right| {
        left.created_unix_ms
            .cmp(&right.created_unix_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(sandboxes)
}

/// One new container sandbox. It returns when the sandbox is ready.
///
/// # Errors
/// The text of a failed create.
pub async fn create(env: &Env, config: &DemoConfig) -> Result<SandboxId, String> {
    let id = SandboxId::generate(&crate::ctx());
    let spec = SandboxSpec::new(RootFs::ContainerImage("sandbox".to_string()));
    let body = serde_json::to_vec(&CreateRequest { spec })
        .map_err(|error| format!("the create body: {error}"))?;
    let response = forward(
        env,
        SANDBOX_BINDING,
        id.as_str(),
        &format!("{SANDBOX_PREFIX}Create"),
        CONNECT_JSON,
        &body,
        Some(&id),
    )
    .await?;
    if response.status_code() != 200 {
        return Err(format!(
            "the create answered status {}: {}",
            response.status_code(),
            bounded_body(response, config.error_text_max_bytes).await
        ));
    }
    let mut frames = Box::pin(ConnectFrames::new(response)?);
    while let Some(item) = frames.next().await {
        let value = item?;
        match serde_json::from_value::<CreateEvent>(value) {
            Ok(CreateEvent::Progress(_)) => {}
            Ok(CreateEvent::Created(_)) => return Ok(id),
            Err(error) => return Err(format!("the create event: {error}")),
        }
    }
    Err("the create ended before the sandbox was ready".to_string())
}

/// One command in one sandbox, as a stream of standard output and standard
/// error text. The last line names the exit code.
///
/// # Errors
/// The text of a failed call.
pub async fn exec(
    env: &Env,
    id: &SandboxId,
    argv: Vec<String>,
    config: &DemoConfig,
) -> Result<Response, String> {
    let exec_id = foundation_sandbox::shared::ids::ExecId::generate(&crate::ctx()).to_string();
    let body = serde_json::to_vec(&Scoped {
        sandbox_id: id.to_string(),
        inner: foundation_sandbox::shared::rpc::messages::ExecRequest {
            exec_id,
            argv,
            env: Vec::new(),
            cwd: None,
            stdin: false,
            timeout_ms: Some(config.exec_timeout_ms),
            user: None,
        },
    })
    .map_err(|error| format!("the exec body: {error}"))?;
    let response = forward(
        env,
        SANDBOX_BINDING,
        id.as_str(),
        &format!("{SANDBOX_PREFIX}Exec"),
        CONNECT_JSON,
        &body,
        Some(id),
    )
    .await?;
    if response.status_code() != 200 {
        return Err(format!(
            "the exec answered status {}: {}",
            response.status_code(),
            bounded_body(response, config.error_text_max_bytes).await
        ));
    }
    let frames = ConnectFrames::new(response)?;
    let stream = frames.map(|item| -> Result<Vec<u8>, worker::Error> {
        match item {
            Ok(value) => Ok(exec_text(value)),
            Err(message) => Ok(format!("\n[error: {message}]\n").into_bytes()),
        }
    });
    text_response(stream)
}

/// Write one small file in one sandbox.
///
/// # Errors
/// The text of a failed call.
pub async fn write_file(
    env: &Env,
    id: &SandboxId,
    path: &str,
    data: Vec<u8>,
    config: &DemoConfig,
) -> Result<u64, String> {
    let begin: serde_json::Value = unary_typed(
        env,
        id,
        "FileWriteBegin",
        &Scoped {
            sandbox_id: id.to_string(),
            inner: FileWriteBeginRequest {
                path: path.to_string(),
                mode: 0o644,
            },
        },
        config,
    )
    .await?;
    let upload_id = begin
        .get("upload_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "the FileWriteBegin answer has no upload_id".to_string())?
        .to_string();
    let _: serde_json::Value = unary_typed(
        env,
        id,
        "FileWriteChunk",
        &Scoped {
            sandbox_id: id.to_string(),
            inner: FileWriteChunkRequest {
                upload_id: upload_id.clone(),
                offset: 0,
                data: data.clone(),
            },
        },
        config,
    )
    .await?;
    let committed: serde_json::Value = unary_typed(
        env,
        id,
        "FileWriteCommit",
        &Scoped {
            sandbox_id: id.to_string(),
            inner: FileWriteCommitRequest {
                upload_id,
                size: u64::try_from(data.len()).unwrap_or(u64::MAX),
            },
        },
        config,
    )
    .await?;
    Ok(committed
        .get("size")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0))
}

/// Read one file in one sandbox, as a stream of bytes.
///
/// # Errors
/// The text of a failed call.
pub async fn read_file(
    env: &Env,
    id: &SandboxId,
    path: &str,
    config: &DemoConfig,
) -> Result<Response, String> {
    let body = serde_json::to_vec(&Scoped {
        sandbox_id: id.to_string(),
        inner: FileReadRequest {
            path: path.to_string(),
        },
    })
    .map_err(|error| format!("the file read body: {error}"))?;
    let response = forward(
        env,
        SANDBOX_BINDING,
        id.as_str(),
        &format!("{SANDBOX_PREFIX}FileRead"),
        CONNECT_JSON,
        &body,
        Some(id),
    )
    .await?;
    if response.status_code() != 200 {
        return Err(format!(
            "the file read answered status {}: {}",
            response.status_code(),
            bounded_body(response, config.error_text_max_bytes).await
        ));
    }
    let frames = ConnectFrames::new(response)?;
    let stream = frames.map(|item| -> Result<Vec<u8>, worker::Error> {
        match item {
            Ok(value) => match serde_json::from_value::<FileChunk>(value) {
                Ok(chunk) => Ok(chunk.data),
                Err(error) => Ok(format!("\n[decode error: {error}]\n").into_bytes()),
            },
            Err(message) => Ok(format!("\n[error: {message}]\n").into_bytes()),
        }
    });
    text_response(stream)
}

/// Stop one sandbox and destroy its container.
///
/// # Errors
/// The text of a failed call.
pub async fn stop(env: &Env, id: &SandboxId, config: &DemoConfig) -> Result<(), String> {
    let body = serde_json::to_vec(&SandboxRef {
        sandbox_id: id.to_string(),
    })
    .map_err(|error| format!("the stop body: {error}"))?;
    let response = forward(
        env,
        SANDBOX_BINDING,
        id.as_str(),
        &format!("{SANDBOX_PREFIX}Stop"),
        JSON,
        &body,
        Some(id),
    )
    .await?;
    let status = response.status_code();
    let body = bounded_body(response, config.error_text_max_bytes).await;
    if (200..300).contains(&status) {
        Ok(())
    } else {
        Err(format!("the stop answered status {status}: {body}"))
    }
}

/// One streamed response with a text content type.
pub(crate) fn text_response<S>(stream: S) -> Result<Response, String>
where
    S: Stream<Item = Result<Vec<u8>, worker::Error>> + 'static,
{
    stream_response(stream, "text/plain; charset=utf-8")
}

/// One streamed response with a status of 200.
fn stream_response<S>(stream: S, content_type: &str) -> Result<Response, String>
where
    S: Stream<Item = Result<Vec<u8>, worker::Error>> + 'static,
{
    let response = Response::from_stream(stream)
        .map_err(|error| format!("the response stream: {error}"))?
        .with_status(200);
    let headers = worker::Headers::new();
    headers
        .set("content-type", content_type)
        .map_err(|error| format!("the content type: {error}"))?;
    Ok(response.with_headers(headers))
}

/// The text of one `Exec` event.
fn exec_text(value: serde_json::Value) -> Vec<u8> {
    let Ok(message) = serde_json::from_value::<ExecEventMsg>(value) else {
        return b"\n[decode error: the exec event]\n".to_vec();
    };
    match message.event {
        ExecEventKind::Started { .. } => Vec::new(),
        ExecEventKind::Stdout { data } | ExecEventKind::Stderr { data } => data,
        ExecEventKind::Exit { code, signal } => {
            let exit = code.map_or_else(
                || {
                    signal.map_or_else(
                        || "unknown".to_string(),
                        |signal| format!("signal {signal}"),
                    )
                },
                |code| code.to_string(),
            );
            format!("\n[exit {exit}]\n").into_bytes()
        }
    }
}

/// One unary call with a JSON request and a JSON answer.
async fn unary_typed<S, T>(
    env: &Env,
    id: &SandboxId,
    method: &str,
    body: &S,
    config: &DemoConfig,
) -> Result<T, String>
where
    S: serde::Serialize,
    T: DeserializeOwned,
{
    let bytes = serde_json::to_vec(body).map_err(|error| format!("the {method} body: {error}"))?;
    let response = forward(
        env,
        SANDBOX_BINDING,
        id.as_str(),
        &format!("{SANDBOX_PREFIX}{method}"),
        JSON,
        &bytes,
        Some(id),
    )
    .await?;
    let value = unary_value(response, config.error_text_max_bytes).await?;
    serde_json::from_value(value).map_err(|error| format!("the {method} answer: {error}"))
}

/// One unary response as a JSON value, with a status check.
async fn unary_value(response: Response, max: usize) -> Result<serde_json::Value, String> {
    let status = response.status_code();
    let body = bounded_body(response, max).await;
    if !(200..300).contains(&status) {
        return Err(format!("the call answered status {status}: {body}"));
    }
    serde_json::from_str(&body).map_err(|error| format!("the JSON answer: {error} ({body})"))
}

/// The bounded text of a small response body.
async fn bounded_body(mut response: Response, max: usize) -> String {
    match response.bytes().await {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes).into_owned();
            text.chars().take(max).collect()
        }
        Err(error) => format!("(the body could not be read: {error})"),
    }
}

/// Sends one request to a DO and returns its response.
async fn forward(
    env: &Env,
    binding: &str,
    name: &str,
    path: &str,
    content_type: &str,
    body: &[u8],
    sandbox_id: Option<&SandboxId>,
) -> Result<Response, String> {
    let stub = env
        .durable_object(binding)
        .map_err(|error| format!("the {binding} binding: {error}"))?
        .get_by_name(name)
        .map_err(|error| format!("the {name} Durable Object: {error}"))?;
    let headers = worker::Headers::new();
    headers
        .set("content-type", content_type)
        .map_err(|error| format!("the content type: {error}"))?;
    if let Some(id) = sandbox_id {
        headers
            .set(SANDBOX_ID_HEADER, id.as_str())
            .map_err(|error| format!("the sandbox identifier header: {error}"))?;
    }
    let mut init = worker::RequestInit::new();
    init.with_method(worker::Method::Post);
    init.with_headers(headers);
    // Every call through this helper is a Connect *server-streaming* call, so
    // the request body is enveloped like the response: flags(1) | len(4) |
    // payload. A plain JSON body makes the server read `{"spe` as a length.
    let mut framed = Vec::with_capacity(5 + body.len());
    framed.push(0);
    framed.extend_from_slice(&(u32::try_from(body.len()).unwrap_or(u32::MAX)).to_be_bytes());
    framed.extend_from_slice(body);
    let array = worker::js_sys::Uint8Array::from(framed.as_slice());
    init.with_body(Some(worker::wasm_bindgen::JsValue::from(array)));
    let url = format!("https://sandbox.rpc{path}");
    let request =
        Request::new_with_init(&url, &init).map_err(|error| format!("the request: {error}"))?;
    stub.fetch_with_request(request)
        .await
        .map_err(|error| format!("the {path} call: {error}"))
}

/// The state of one sandbox as text.
fn state_text(state: &foundation_sandbox::shared::control::SandboxState) -> String {
    serde_json::to_value(state)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| format!("{state:?}"))
}

/// The Connect frames of one streamed response.
///
/// The wire frame is `flags(1) | len(4, big endian) | payload(len)`. The codec
/// is JSON, so each data frame is one JSON message. The end-stream frame
/// (`flags & 0x02`) carries an optional error and is not a message.
pub struct ConnectFrames {
    inner: Pin<Box<worker::ByteStream>>,
    buffer: Vec<u8>,
    ended: bool,
}

impl ConnectFrames {
    /// The frames of one response.
    ///
    /// # Errors
    /// The error of the response body stream.
    pub fn new(mut response: Response) -> Result<Self, String> {
        let inner = response
            .stream()
            .map_err(|error| format!("the response stream: {error}"))?;
        Ok(Self {
            inner: Box::pin(inner),
            buffer: Vec::new(),
            ended: false,
        })
    }
}

impl Stream for ConnectFrames {
    type Item = Result<serde_json::Value, String>;

    fn poll_next(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        loop {
            if this.buffer.len() >= 5 {
                let length = u32::from_be_bytes([
                    this.buffer[1],
                    this.buffer[2],
                    this.buffer[3],
                    this.buffer[4],
                ]) as usize;
                if this.buffer.len() >= 5 + length {
                    let flags = this.buffer[0];
                    let payload = this.buffer[5..5 + length].to_vec();
                    this.buffer.drain(..5 + length);
                    if flags & 0x01 != 0 {
                        this.ended = true;
                        return Poll::Ready(Some(Err(
                            "the response used a compressed frame; this demo reads plain JSON"
                                .to_string(),
                        )));
                    }
                    if flags & 0x02 != 0 {
                        this.ended = true;
                        let error = serde_json::from_slice::<serde_json::Value>(&payload)
                            .ok()
                            .and_then(|value| {
                                value.get("error").map(|error| {
                                    error
                                        .get("message")
                                        .and_then(serde_json::Value::as_str)
                                        .unwrap_or("the call failed")
                                        .to_string()
                                })
                            });
                        return match error {
                            Some(message) => Poll::Ready(Some(Err(message))),
                            None => Poll::Ready(None),
                        };
                    }
                    return match serde_json::from_slice::<serde_json::Value>(&payload) {
                        Ok(value) => Poll::Ready(Some(Ok(value))),
                        Err(error) => {
                            Poll::Ready(Some(Err(format!("the frame is not JSON: {error}"))))
                        }
                    };
                }
            }
            if this.ended {
                return Poll::Ready(None);
            }
            match Pin::new(&mut this.inner).poll_next(context) {
                Poll::Ready(Some(Ok(chunk))) => this.buffer.extend_from_slice(&chunk),
                Poll::Ready(Some(Err(error))) => {
                    this.ended = true;
                    return Poll::Ready(Some(Err(format!("the response body: {error}"))));
                }
                Poll::Ready(None) => {
                    this.ended = true;
                    if this.buffer.is_empty() {
                        return Poll::Ready(None);
                    }
                    return Poll::Ready(Some(Err(
                        "the response ended in the middle of a frame".to_string()
                    )));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
