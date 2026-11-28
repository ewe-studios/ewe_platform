//! The control API of the page and the live test.
//!
//! Every route is under `/api/`. The API runs in the Worker, and it calls the
//! sandbox Durable Objects (DOs) and the model paths. It streams a command, a
//! file read and an agent turn, so no body is read whole.

use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;
use worker::{Env, Request, Response};

use crate::agent::{self, ModelPath};
use crate::config::DemoConfig;
use crate::sandbox;

/// One control API request.
pub async fn handle(
    mut request: Request,
    env: Env,
    config: &DemoConfig,
) -> worker::Result<Response> {
    let method = request.method();
    let path = request.path();
    let Some(route) = path.strip_prefix("/api/") else {
        return not_found();
    };
    let segments: Vec<&str> = route.split('/').filter(|part| !part.is_empty()).collect();
    match (method, segments.as_slice()) {
        (worker::Method::Get, ["probe"]) => probe(),
        (worker::Method::Post, ["chat"]) => central_chat(&mut request, &env, config).await,
        (worker::Method::Get, ["sandboxes"]) => list(&env, config).await,
        (worker::Method::Post, ["sandboxes"]) => create(&mut request, &env, config).await,
        (worker::Method::Delete, ["sandboxes"]) => stop_all(&env, config).await,
        (worker::Method::Delete, ["sandboxes", id]) => stop(&env, config, id).await,
        (worker::Method::Post, ["sandboxes", id, "exec"]) => {
            exec(&mut request, &env, config, id).await
        }
        (worker::Method::Post, ["sandboxes", id, "file"]) => {
            write_file(&mut request, &env, config, id).await
        }
        (worker::Method::Get, ["sandboxes", id, "file"]) => {
            read_file(&request, &env, config, id).await
        }
        (worker::Method::Post, ["sandboxes", id, "chat"]) => {
            chat(&mut request, &env, config, id).await
        }
        _ => not_found(),
    }
}

/// `GET /api/probe`: the control probe of the local `wrangler dev` check.
fn probe() -> worker::Result<Response> {
    Response::from_json(&json!({
        "ok": true,
        "demo": "cf-sandbox-demo",
        "version": foundation_sandbox::VERSION,
        "driver": "cloudflare-container",
        "model_paths": ["workers-ai", "openrouter", "container-model"],
    }))
}

/// `GET /api/sandboxes`: the count and one tab per sandbox.
async fn list(env: &Env, config: &DemoConfig) -> worker::Result<Response> {
    let sandboxes = match sandbox::list(env, config).await {
        Ok(sandboxes) => sandboxes,
        Err(error) => return error_response(502, &error),
    };
    let tabs: Vec<serde_json::Value> = sandboxes
        .iter()
        .enumerate()
        .map(|(index, sandbox)| {
            let path = ModelPath::for_tab(index);
            json!({
                "index": index + 1,
                "name": tab_name(index),
                "id": sandbox.id.as_str(),
                // The sandbox Durable Object is named by its identifier.
                "do_name": sandbox.id.as_str(),
                "do_binding": sandbox::SANDBOX_BINDING,
                "state": sandbox.state,
                "created_unix_ms": sandbox.created_unix_ms,
                "model_path": path.as_str(),
                "model_label": path.label(),
                "model": path.model_name(config),
                "model_service": path.service(),
                "model_address": path.address(config),
                "model_protocol": path.protocol(config),
            })
        })
        .collect();
    Response::from_json(&json!({
        "count": tabs.len(),
        "max": config.max_sandboxes,
        "sandboxes": tabs,
    }))
}

/// The body of `POST /api/sandboxes`.
#[derive(Deserialize)]
struct CreateBody {
    /// How many sandboxes to add. Default 1.
    #[serde(default)]
    count: Option<usize>,
}

/// `POST /api/sandboxes`: creates sandboxes and streams one JSON line per
/// step. It streams from the first byte, because a container start can take
/// longer than a Worker may wait before its response head.
async fn create(request: &mut Request, env: &Env, config: &DemoConfig) -> worker::Result<Response> {
    let body: CreateBody = match json_body(request, config.control_body_max_bytes).await {
        Ok(body) => body,
        Err(message) => return error_response(400, &message),
    };
    let count = body.count.unwrap_or(1);
    if count == 0 || count > config.max_sandboxes {
        return error_response(
            400,
            &format!(
                "count must be between 1 and the configured maximum of {}",
                config.max_sandboxes
            ),
        );
    }
    let current = match sandbox::list(env, config).await {
        Ok(current) => current.len(),
        Err(error) => return error_response(502, &error),
    };
    if current + count > config.max_sandboxes {
        return error_response(
            409,
            &format!(
                "{current} sandboxes run; the maximum is {}",
                config.max_sandboxes
            ),
        );
    }
    let state = CreateState {
        env: env.clone(),
        config: config.clone(),
        remaining: count,
        ids: Vec::new(),
        first: true,
        done: false,
    };
    let stream = futures::stream::unfold(state, |mut state| async move {
        if state.done {
            return None;
        }
        if state.first {
            state.first = false;
            return Some((
                line(json!({ "event": "starting", "count": state.remaining })),
                state,
            ));
        }
        if state.remaining > 0 {
            return match sandbox::create(&state.env, &state.config).await {
                Ok(id) => {
                    state.remaining -= 1;
                    state.ids.push(id.to_string());
                    let body = json!({
                        "event": "created",
                        "id": id.as_str(),
                        "count": state.ids.len(),
                    });
                    Some((line(body), state))
                }
                Err(message) => {
                    state.done = true;
                    Some((line(json!({ "event": "error", "message": message })), state))
                }
            };
        }
        state.done = true;
        Some((
            line(json!({
                "event": "done",
                "count": state.ids.len(),
                "ids": state.ids.clone(),
            })),
            state,
        ))
    });
    streamed(stream, "application/x-ndjson")
}

/// The state of one create stream.
struct CreateState {
    env: Env,
    config: DemoConfig,
    remaining: usize,
    ids: Vec<String>,
    first: bool,
    done: bool,
}

/// One JSON line as a stream item.
fn line(value: serde_json::Value) -> Result<Vec<u8>, worker::Error> {
    let mut text =
        serde_json::to_vec(&value).map_err(|error| worker::Error::RustError(error.to_string()))?;
    text.push(b'\n');
    Ok(text)
}

/// `DELETE /api/sandboxes`: stops every sandbox.
async fn stop_all(env: &Env, config: &DemoConfig) -> worker::Result<Response> {
    let sandboxes = match sandbox::list(env, config).await {
        Ok(sandboxes) => sandboxes,
        Err(error) => return error_response(502, &error),
    };
    let mut stopped = 0_usize;
    let mut failures = Vec::new();
    for sandbox in &sandboxes {
        match sandbox::stop(env, &sandbox.id, config).await {
            Ok(()) => stopped += 1,
            Err(error) => failures.push(json!({ "id": sandbox.id.as_str(), "error": error })),
        }
        agent::close_openrouter(sandbox.id.as_str()).await;
    }
    let status = if failures.is_empty() { 200 } else { 500 };
    json_status(status, json!({ "stopped": stopped, "failed": failures }))
}

/// `DELETE /api/sandboxes/<id>`: stops one sandbox.
async fn stop(env: &Env, config: &DemoConfig, id: &str) -> worker::Result<Response> {
    let Ok(id) = id.parse::<foundation_sandbox::shared::ids::SandboxId>() else {
        return error_response(400, "the sandbox identifier is not valid");
    };
    match sandbox::stop(env, &id, config).await {
        Ok(()) => {
            agent::close_openrouter(id.as_str()).await;
            Response::from_json(&json!({ "stopped": id.as_str() }))
        }
        Err(error) => error_response(502, &error),
    }
}

/// The body of `POST /api/sandboxes/<id>/exec`.
#[derive(Deserialize)]
struct ExecBody {
    /// A command line. It is split on space, and no shell runs it.
    #[serde(default)]
    command: Option<String>,
    /// The argument vector, when the caller sends one.
    #[serde(default)]
    argv: Option<Vec<String>>,
}

/// `POST /api/sandboxes/<id>/exec`: runs one command and streams its output.
async fn exec(
    request: &mut Request,
    env: &Env,
    config: &DemoConfig,
    id: &str,
) -> worker::Result<Response> {
    let id = match parse_id(id) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let body: ExecBody = match json_body(request, config.command_max_bytes * 4).await {
        Ok(body) => body,
        Err(message) => return error_response(400, &message),
    };
    let argv = match body.argv {
        Some(argv) => argv,
        None => match body.command {
            Some(command) => split_command(&command),
            None => return error_response(400, "send `command` or `argv`"),
        },
    };
    if argv.is_empty() {
        return error_response(400, "the command is empty");
    }
    if argv.len() > config.command_max_argv {
        return error_response(
            400,
            &format!(
                "the command has {} arguments; the maximum is {}",
                argv.len(),
                config.command_max_argv
            ),
        );
    }
    let bytes: usize = argv.iter().map(String::len).sum();
    if bytes > config.command_max_bytes {
        return error_response(
            400,
            &format!(
                "the command has {bytes} bytes; the maximum is {}",
                config.command_max_bytes
            ),
        );
    }
    match sandbox::exec(env, &id, argv, config).await {
        Ok(response) => with_id(response, &id),
        Err(error) => error_response(502, &error),
    }
}

/// The body of `POST /api/sandboxes/<id>/file`.
#[derive(Deserialize)]
struct FileBody {
    /// The path in the guest.
    path: String,
    /// The file content, as text.
    content: String,
}

/// `POST /api/sandboxes/<id>/file`: writes one small file.
async fn write_file(
    request: &mut Request,
    env: &Env,
    config: &DemoConfig,
    id: &str,
) -> worker::Result<Response> {
    let id = match parse_id(id) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let body: FileBody = match json_body(request, config.file_max_bytes * 2).await {
        Ok(body) => body,
        Err(message) => return error_response(400, &message),
    };
    if let Err(message) = check_path(&body.path) {
        return error_response(400, &message);
    }
    let data = body.content.into_bytes();
    if data.len() > config.file_max_bytes {
        return error_response(
            400,
            &format!(
                "the file is {} bytes; the maximum is {}",
                data.len(),
                config.file_max_bytes
            ),
        );
    }
    match sandbox::write_file(env, &id, &body.path, data, config).await {
        Ok(size) => Response::from_json(&json!({ "written": size, "path": body.path })),
        Err(error) => error_response(502, &error),
    }
}

/// `GET /api/sandboxes/<id>/file?path=...`: reads one file and streams it.
async fn read_file(
    request: &Request,
    env: &Env,
    config: &DemoConfig,
    id: &str,
) -> worker::Result<Response> {
    let id = match parse_id(id) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let path = match request.url() {
        Ok(url) => url
            .query_pairs()
            .find(|(name, _)| name.as_ref() == "path")
            .map(|(_, value)| value.into_owned()),
        Err(_) => None,
    };
    let Some(path) = path else {
        return error_response(400, "send the `path` query parameter");
    };
    if let Err(message) = check_path(&path) {
        return error_response(400, &message);
    }
    match sandbox::read_file(env, &id, &path, config).await {
        Ok(response) => with_id(response, &id),
        Err(error) => error_response(502, &error),
    }
}

/// The body of `POST /api/sandboxes/<id>/chat` and `POST /api/chat`.
#[derive(Deserialize)]
struct ChatBody {
    /// The prompt of one turn.
    prompt: String,
}

/// `POST /api/sandboxes/<id>/chat`: one agent turn on the model path of the
/// sandbox's tab. The answer streams.
async fn chat(
    request: &mut Request,
    env: &Env,
    config: &DemoConfig,
    id: &str,
) -> worker::Result<Response> {
    let id = match parse_id(id) {
        Ok(id) => id,
        Err(response) => return response,
    };
    let body: ChatBody = match json_body(request, config.prompt_max_bytes * 2).await {
        Ok(body) => body,
        Err(message) => return error_response(400, &message),
    };
    let sandboxes = match sandbox::list(env, config).await {
        Ok(sandboxes) => sandboxes,
        Err(error) => return error_response(502, &error),
    };
    let Some(index) = sandboxes.iter().position(|sandbox| sandbox.id == id) else {
        return error_response(404, "the sandbox is not in the index");
    };
    let prompt = match checked_prompt(config, &body.prompt) {
        Ok(prompt) => prompt,
        Err(message) => return error_response(400, &message),
    };
    run_chat(env, config, index, &id, &tab_name(index), prompt).await
}

/// `POST /api/chat`: one agent turn routed by `@name`.
///
/// The body holds the name and the message, for example
/// `{"prompt": "@sandbox-2 hello"}`. The Worker resolves the name in the
/// sandbox index and calls that sandbox. An unknown name answers the names
/// that exist.
async fn central_chat(
    request: &mut Request,
    env: &Env,
    config: &DemoConfig,
) -> worker::Result<Response> {
    let body: ChatBody = match json_body(request, config.prompt_max_bytes * 2).await {
        Ok(body) => body,
        Err(message) => return error_response(400, &message),
    };
    let sandboxes = match sandbox::list(env, config).await {
        Ok(sandboxes) => sandboxes,
        Err(error) => return error_response(502, &error),
    };
    let (index, message) = match resolve_target(&sandboxes, &body.prompt) {
        Ok(target) => target,
        Err(TargetError::EmptyIndex) => {
            return error_response(409, "there are no sandboxes; create one first")
        }
        Err(TargetError::MissingHandle) => {
            return error_response(
                400,
                "start the message with @name, for example `@sandbox-1 hello`",
            )
        }
        Err(TargetError::UnknownName { handle, known }) => {
            return error_response(
                404,
                &format!(
                    "the name @{handle} is not known; known names: {}",
                    known.join(", ")
                ),
            )
        }
        Err(TargetError::EmptyMessage(name)) => {
            return error_response(400, &format!("write a message after @{name}"))
        }
    };
    let prompt = match checked_prompt(config, &message) {
        Ok(prompt) => prompt,
        Err(message) => return error_response(400, &message),
    };
    let id = sandboxes[index].id.clone();
    run_chat(env, config, index, &id, &tab_name(index), prompt).await
}

/// The name of the tab and the sandbox at `index` (zero-based).
fn tab_name(index: usize) -> String {
    format!("sandbox-{}", index + 1)
}

/// The reason that a central chat prompt has no target.
enum TargetError {
    /// The index holds no sandbox.
    EmptyIndex,
    /// The message does not start with `@name`.
    MissingHandle,
    /// The name is not in the index.
    UnknownName {
        /// The name that the user wrote.
        handle: String,
        /// The names that exist.
        known: Vec<String>,
    },
    /// The name is known, but no message follows it.
    EmptyMessage(String),
}

/// Resolves `@name message` against the sandbox index. On success it returns
/// the zero-based index and the message. The page calls the sandboxes
/// `sandbox-1`, `sandbox-2`, and so on. The identifier is also accepted.
fn resolve_target(
    sandboxes: &[sandbox::Sandbox],
    prompt: &str,
) -> Result<(usize, String), TargetError> {
    if sandboxes.is_empty() {
        return Err(TargetError::EmptyIndex);
    }
    let prompt = prompt.trim();
    let Some(rest) = prompt.strip_prefix('@') else {
        return Err(TargetError::MissingHandle);
    };
    let (handle, message) = match rest.split_once(char::is_whitespace) {
        Some((handle, message)) => (handle.trim(), message.trim()),
        None => (rest.trim(), ""),
    };
    if handle.is_empty() {
        return Err(TargetError::MissingHandle);
    }
    let target = sandboxes.iter().enumerate().find(|(index, sandbox)| {
        tab_name(*index).eq_ignore_ascii_case(handle) || sandbox.id.as_str() == handle
    });
    let Some((index, _)) = target else {
        let known = (0..sandboxes.len()).map(tab_name).collect();
        return Err(TargetError::UnknownName {
            handle: handle.to_string(),
            known,
        });
    };
    if message.is_empty() {
        return Err(TargetError::EmptyMessage(tab_name(index)));
    }
    Ok((index, message.to_string()))
}

/// Checks and trims one prompt.
fn checked_prompt(config: &DemoConfig, prompt: &str) -> Result<String, String> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err("the prompt is empty".to_string());
    }
    if prompt.len() > config.prompt_max_bytes {
        return Err(format!(
            "the prompt is {} bytes; the maximum is {}",
            prompt.len(),
            config.prompt_max_bytes
        ));
    }
    Ok(prompt.to_string())
}

/// One agent turn on the model path of the sandbox at `index`. The answer
/// streams. The metadata headers name the path, its service, its address and
/// its protocol, so the page never guesses them.
async fn run_chat(
    env: &Env,
    config: &DemoConfig,
    index: usize,
    id: &foundation_sandbox::shared::ids::SandboxId,
    name: &str,
    prompt: String,
) -> worker::Result<Response> {
    let path = ModelPath::for_tab(index);
    let model = path.model_name(config);
    let result = match path {
        ModelPath::WorkersAi => agent::workers_ai_turn(env, config, &prompt).await,
        ModelPath::OpenRouter => {
            let ctx = crate::ctx();
            agent::openrouter_turn(env, config, &ctx, id.as_str(), &prompt).await
        }
        ModelPath::Container => agent::container_turn(env, config, id.as_str(), &prompt).await,
    };
    match result {
        Ok(response) => with_chat_headers(response, path, config, &model, id, name),
        Err(message) => {
            let response = error_response(503, &message)?;
            with_chat_headers(response, path, config, &model, id, name)
        }
    }
}

/// Adds the model and sandbox metadata headers to one chat response.
fn with_chat_headers(
    response: Response,
    path: ModelPath,
    config: &DemoConfig,
    model: &str,
    id: &foundation_sandbox::shared::ids::SandboxId,
    name: &str,
) -> worker::Result<Response> {
    let headers = response.headers();
    headers.set("x-ewe-model-path", path.as_str())?;
    headers.set("x-ewe-model", model)?;
    headers.set("x-ewe-model-service", path.service())?;
    headers.set("x-ewe-model-address", &path.address(config))?;
    headers.set("x-ewe-model-protocol", path.protocol(config))?;
    headers.set("x-ewe-sandbox-id", id.as_str())?;
    headers.set("x-ewe-sandbox-name", name)?;
    Ok(response)
}

/// Reads one bounded JSON body.
async fn json_body<T: for<'de> Deserialize<'de>>(
    request: &mut Request,
    max: usize,
) -> Result<T, String> {
    let bytes = read_body(request, max).await?;
    serde_json::from_slice(&bytes).map_err(|error| format!("the JSON body: {error}"))
}

/// Reads one bounded body.
async fn read_body(request: &mut Request, max: usize) -> Result<Vec<u8>, String> {
    let mut stream = request
        .stream()
        .map_err(|error| format!("the request body: {error}"))?;
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("the request body: {error}"))?;
        if body.len().saturating_add(chunk.len()) > max {
            return Err(format!("the body is larger than {max} bytes"));
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

/// A command line as an argument vector. It runs no shell, so a quoting
/// character is a literal part of an argument.
fn split_command(command: &str) -> Vec<String> {
    command
        .split_whitespace()
        .filter(|word| !word.is_empty())
        .map(str::to_string)
        .collect()
}

/// Checks a guest path of a file write or read.
fn check_path(path: &str) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("the path is empty".to_string());
    }
    if path.len() > 1_024 {
        return Err("the path is longer than 1024 bytes".to_string());
    }
    if path.contains('\0') {
        return Err("the path holds a NUL byte".to_string());
    }
    Ok(())
}

/// One sandbox identifier, or the error response.
fn parse_id(
    id: &str,
) -> Result<foundation_sandbox::shared::ids::SandboxId, worker::Result<Response>> {
    id.parse()
        .map_err(|_| error_response(400, "the sandbox identifier is not valid"))
}

/// Adds the sandbox identifier header.
fn with_id(
    response: Response,
    id: &foundation_sandbox::shared::ids::SandboxId,
) -> worker::Result<Response> {
    response.headers().set("x-ewe-sandbox-id", id.as_str())?;
    Ok(response)
}

/// One streamed response with a status of 200.
fn streamed<S>(stream: S, content_type: &str) -> worker::Result<Response>
where
    S: futures::Stream<Item = Result<Vec<u8>, worker::Error>> + 'static,
{
    let response = Response::from_stream(stream)?.with_status(200);
    let headers = worker::Headers::new();
    headers.set("content-type", content_type)?;
    Ok(response.with_headers(headers))
}

/// One JSON error response.
fn error_response(status: u16, message: &str) -> worker::Result<Response> {
    json_status(status, json!({ "error": message }))
}

/// One JSON response with a status.
fn json_status(status: u16, body: serde_json::Value) -> worker::Result<Response> {
    Response::from_json(&body).map(|response| response.with_status(status))
}

/// The 404 answer of an unknown route.
fn not_found() -> worker::Result<Response> {
    error_response(404, "not found")
}
