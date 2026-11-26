//! Exchange action frames over an upgraded connection.
use crate::{
    http,
    subscription::{one, FeedLeases, Replies, Subscriptions},
    ViewError,
};
use foundation_core::io::{DecodeStep, IncrementalDecoder};
use foundation_netio::websocket::{
    MessageAssembler, Opcode, WebSocketFrame, WebSocketFrameDecoder, WebSocketMessage,
};
use foundation_ui_runtime::dispatch::{Connection, Dispatcher, HelloOutcome};
use foundation_ui_wire::OwnedFrame;
use http::native::writer::{ReadWait, ResponseWriter, WriterConfig};
use std::{
    collections::HashMap,
    io::{self, Read},
    sync::Arc,
};

/// Limits the number of views on one physical connection.
#[derive(Clone, Copy)]
pub struct SocketConfig {
    /// The default follows the HTTP/2 concurrent stream limit used by nginx.
    pub max_views: usize,
    /// Maximum waiting pushes per socket and subscriptions per view.
    pub max_subscriptions: usize,
    /// Event-stream heartbeat. Defaults to 15 seconds.
    pub heartbeat: std::time::Duration,
}

impl Default for SocketConfig {
    fn default() -> Self {
        Self {
            max_views: 128,
            max_subscriptions: 128,
            heartbeat: std::time::Duration::from_secs(15),
        }
    }
}

/// Serves one or more screens through the same authenticated socket.
#[derive(Clone)]
pub struct ActionSocketRoute(Arc<Dispatcher>);

impl ActionSocketRoute {
    #[must_use]
    pub fn new(dispatcher: Arc<Dispatcher>) -> Self {
        Self(dispatcher)
    }
}

impl http::Route for ActionSocketRoute {
    const PATH: &'static str = "/socket";
    const METHOD: http::SimpleMethod = http::SimpleMethod::GET;
}

impl http::Serve for ActionSocketRoute {
    fn serve(
        &self,
        ctx: http::Ctx,
        req: http::SimpleIncomingRequest,
        conn: http::SharedByteBufferStream<http::RawStream>,
    ) -> http::ConnectionResult {
        let dispatcher = self.0.clone();
        http::ConnectionResult::Async(Box::pin(async move {
            if !http::native::upgrade::accept_websocket(&conn, &req, &ctx).await {
                return crate::delivery::error_response(
                    &ctx,
                    conn,
                    ViewError::BadRequest("Expected a WebSocket upgrade".into()),
                )
                .await;
            }
            match drive(&ctx, conn, &dispatcher).await {
                Ok(()) => http::ServeOutcome::Close(None),
                Err(err) => http::ServeOutcome::Close(Some(
                    http::shared::serve::ServeError::InternalError {
                        status: 400,
                        reason: err.to_string(),
                    }
                    .into(),
                )),
            }
        }))
    }
}

// Preserve the distinction between an idle socket and a peer that closed it.
struct SocketReader {
    conn: http::SharedByteBufferStream<http::RawStream>,
    read_bytes: usize,
    eof: bool,
}
impl Read for SocketReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        self.read_bytes = 0;
        let count = self.conn.read(output)?;
        self.read_bytes = count;
        self.eof = count == 0 && !output.is_empty();
        Ok(count)
    }
}

async fn drive(
    ctx: &http::Ctx,
    conn: http::SharedByteBufferStream<http::RawStream>,
    dispatcher: &Dispatcher,
) -> io::Result<()> {
    let limits = dispatcher.wire_limits();
    let max = limits.action_request_bytes().min(limits.frame_bytes());
    let mut decoder = WebSocketFrameDecoder::with_limit(max);
    let mut assembler = MessageAssembler::new(max);
    let mut views = HashMap::new();
    let mut attached = FeedLeases::new();
    let config = ctx.get_cloned::<SocketConfig>().unwrap_or_default();
    let (output, pushes) = async_channel::bounded(config.max_subscriptions.max(1));
    let writer_config = ctx.get_cloned::<WriterConfig>().unwrap_or_default();
    let interval = writer_config.poll_fallback_interval;
    let mut writer = ResponseWriter::new(conn.clone(), writer_config, ctx.clone());
    let mut ready = ReadWait::new(&conn);
    let mut reader = SocketReader {
        conn,
        read_bytes: 0,
        eof: false,
    };
    loop {
        let DecodeStep::Frame(frame) = decoder.step(&mut reader).map_err(io::Error::other)? else {
            if reader.eof {
                return Ok(());
            }
            if reader.read_bytes > 0 {
                continue;
            }
            let push = futures_lite::future::race(
                async { ready.readable(interval, ctx).await.map(|()| None) },
                async { pushes.recv().await.map(Some).map_err(io::Error::other) },
            )
            .await?;
            if let Some(bytes) = push {
                write_packet(&mut writer, Opcode::Binary, bytes).await?;
            }
            continue;
        };
        if frame.mask.is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Unmasked client frame",
            ));
        }
        let (opcode, payload) = match frame.opcode {
            Opcode::Close => (Opcode::Close, frame.payload),
            Opcode::Ping => (Opcode::Pong, frame.payload),
            Opcode::Pong => continue,
            _ => match assembler.process_frame(frame).map_err(io::Error::other)? {
                None => continue,
                Some(WebSocketMessage::Binary(bytes)) => {
                    let frame = OwnedFrame::new(bytes, limits).map_err(io::Error::other)?;
                    let replies = exchange(
                        ctx,
                        dispatcher,
                        frame,
                        &mut views,
                        &mut attached,
                        &output,
                        config,
                    )
                    .await?;
                    for reply in replies {
                        write_packet(&mut writer, Opcode::Binary, reply?).await?;
                    }
                    continue;
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Expected a binary Foundation frame",
                    ))
                }
            },
        };
        write_packet(&mut writer, opcode, payload).await?;
        if opcode == Opcode::Close {
            return Ok(());
        }
    }
}

async fn write_packet(
    writer: &mut ResponseWriter,
    opcode: Opcode,
    payload: Vec<u8>,
) -> io::Result<()> {
    let packet = WebSocketFrame {
        fin: true,
        opcode,
        mask: None,
        payload,
    };
    writer.write_all(&packet.encode()).await?;
    writer.flush().await
}

async fn exchange(
    ctx: &http::Ctx,
    dispatcher: &Dispatcher,
    frame: OwnedFrame,
    views: &mut HashMap<u64, (Connection, Option<Subscriptions>)>,
    attached: &mut FeedLeases,
    output: &async_channel::Sender<Vec<u8>>,
    config: SocketConfig,
) -> io::Result<Replies> {
    let verified = frame.view();
    let request = verified
        .kind_as_frame_request()
        .ok_or_else(|| io::Error::other("Expected a request"))?;
    if request.hello().is_some() {
        if verified.stream_id() != 0 || views.len() >= config.max_views {
            return Err(io::Error::other("Cannot start another view"));
        }
        return match dispatcher
            .hello_verified(ctx, verified)
            .map_err(io::Error::other)?
        {
            HelloOutcome::Accepted {
                connection,
                caps_frame,
            } => {
                if views.contains_key(&connection.stream_id()) {
                    return Err(io::Error::other("View identifier collision"));
                }
                let subscriptions = if connection.subscription_resume() {
                    let subscriptions = Subscriptions::new(ctx, connection.caps());
                    subscriptions.attach(attached, output)?;
                    Some(subscriptions)
                } else {
                    None
                };
                views.insert(connection.stream_id(), (connection, subscriptions));
                Ok(one(caps_frame))
            }
            HelloOutcome::UpdateRequired { caps_frame } => Ok(one(caps_frame)),
        };
    }
    let (view, subscriptions) = views
        .get_mut(&verified.stream_id())
        .ok_or_else(|| io::Error::other("Unknown delivery session"))?;
    if let Some(resume) = request.resume() {
        if request.action().is_some() || resume.delivery_session() != view.stream_id() {
            return Err(io::Error::other("Invalid subscription request"));
        }
        return subscriptions
            .as_mut()
            .ok_or_else(|| io::Error::other("Replay was not negotiated"))?
            .replay(resume, dispatcher.wire_limits(), config.max_subscriptions);
    }
    Ok(one(dispatcher.dispatch_verified(ctx, view, verified).await))
}
