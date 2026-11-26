//! Submit one Todo through the public action endpoint used by release starter tests.
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use foundation_tasks::{ProcessCommand, TaskContext, TaskError, TaskResult};
use foundation_ui_wire::{
    finish_frame, read_frame, ActionInputT, ActionRequestT, CapsStatus, ClientHelloT, FrameKindT,
    FrameRequestT, FrameT, MountSurface, Platform, WireLimits, SCHEMA_VERSION,
};

fn encode(req: FrameRequestT, stream: u64, sequence: u64) -> Result<Vec<u8>, TaskError> {
    let mut frame = FrameT::default();
    frame.schema_version = SCHEMA_VERSION;
    frame.stream_id = stream;
    frame.sequence_id = sequence;
    frame.kind = FrameKindT::FrameRequest(Box::new(req));
    finish_frame(&frame).map_err(TaskError::operation)
}

pub async fn run(ctx: &TaskContext, url: &str) -> TaskResult {
    let mut hello = ClientHelloT::default();
    hello.vocabulary_version = foundation_ui_wire::vocab::VOCABULARY_VERSION;
    hello.platform = Platform::Web;
    hello.mount = MountSurface::Web;
    hello.subscription_resume = true;
    let mut req = FrameRequestT::default();
    req.hello = Some(Box::new(hello));
    let hello = encode(req, 0, 0)?;
    let send = ProcessCommand::new("curl")
        .args([
            "--silent",
            "--show-error",
            "--fail",
            "--header",
            "content-type: application/primal-frame",
            "--header",
            "accept: application/primal-frame",
            "--header",
            "x-ewe-action: 1",
            "--header",
        ])
        .arg(format!("x-ewe-hello: {}", URL_SAFE_NO_PAD.encode(&hello)))
        .args(["--data-binary", "@-"])
        .arg(format!("{}/action", url.trim_end_matches('/')));
    let response = super::output::device_query(ctx, send.clone().private_input(hello)).await?;
    let mut stream = None;
    inspect(&response, |frame| {
        let message = frame
            .kind_as_frame_message()
            .and_then(|body| body.messages())
            .ok_or_else(|| TaskError::operation("Missing handshake messages"))?;
        for entry in message {
            if let Some(caps) = entry.control().and_then(|control| control.caps()) {
                if caps.status() != CapsStatus::Ok
                    || !caps
                        .actions()
                        .is_some_and(|actions| actions.iter().any(|a| a.name() == Some("add_todo")))
                {
                    return Err(TaskError::operation(
                        "The server did not advertise add_todo",
                    ));
                }
                stream = Some(caps.stream_id());
            }
        }
        Ok(())
    })?;
    let stream = stream.ok_or_else(|| TaskError::operation("Missing handshake reply"))?;
    let mut input = ActionInputT::default();
    input.event_name = Some("submit".into());
    let id = ctx
        .context()
        .require_env()
        .map_err(TaskError::operation)?
        .entropy()
        .uuid_v4()
        .map_err(TaskError::operation)?;
    input.json = Some(
        serde_json::to_vec(&Add {
            id: id.to_string(),
            title: "Package starter check",
        })
        .map_err(TaskError::operation)?,
    );
    let mut action = ActionRequestT::default();
    action.action = Some("server:add_todo".into());
    action.input = Some(Box::new(input));
    let mut req = FrameRequestT::default();
    req.action = Some(Box::new(action));
    let response =
        super::output::device_query(ctx, send.private_input(encode(req, stream, 1)?)).await?;
    let mut received = false;
    inspect(&response, |frame| {
        let messages = frame
            .kind_as_frame_message()
            .and_then(|body| body.messages())
            .ok_or_else(|| TaskError::operation("Missing action reply"))?;
        for message in messages {
            if message.control().is_some() {
                return Err(TaskError::operation("The server rejected the Todo action"));
            }
            received |= message.data().is_some();
        }
        Ok(())
    })?;
    if received {
        Ok(())
    } else {
        Err(TaskError::operation("Missing Todo confirmation"))
    }
}
fn inspect(
    mut data: &[u8],
    mut visit: impl FnMut(foundation_ui_wire::Frame<'_>) -> TaskResult,
) -> TaskResult {
    while !data.is_empty() {
        let (header, rest) = data
            .split_at_checked(4)
            .ok_or_else(|| TaskError::operation("Truncated frame header"))?;
        let length = u32::from_le_bytes(header.try_into().map_err(TaskError::operation)?) as usize;
        let (frame, next) = rest
            .split_at_checked(length)
            .ok_or_else(|| TaskError::operation("Truncated frame body"))?;
        visit(read_frame(frame, &WireLimits::default()).map_err(TaskError::operation)?)?;
        data = next;
    }
    Ok(())
}

#[derive(serde::Serialize)]
struct Add {
    id: String,
    title: &'static str,
}
