//! Keep a replay control request separate from its open event response.
use crate::{
    action_route::hello_header, delivery::error_response, http, subscription::Subscriptions,
    SocketConfig, ViewError,
};
use foundation_ui_runtime::dispatch::{Dispatcher, HelloOutcome};
use foundation_ui_wire::{AsyncFrameReader, OwnedFrame, StreamFrameWriter, WireLimits};
use http::native::writer::{ReadWait, ResponseWriter, WriterConfig};
use http::native::{upgrade::SseStream, writer::send_bytes};
use std::io::{self, Read};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

/// Install the authenticated principal in each request context when the feed is private.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveryPrincipal(pub String);

struct Session {
    principal: Option<DeliveryPrincipal>,
    subscriptions: Mutex<Subscriptions>,
    output: async_channel::Sender<Vec<u8>>,
}

#[derive(Default)]
struct Sessions(Mutex<HashMap<u64, Arc<Session>>>);

struct Lease(Arc<Sessions>, u64);
impl Drop for Lease {
    fn drop(&mut self) {
        self.0
             .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.1);
    }
}

/// Serve GET for delivery and POST for resume on the same endpoint.
#[derive(Clone)]
pub struct EventDelivery {
    dispatcher: Arc<Dispatcher>,
    sessions: Arc<Sessions>,
    legacy: Arc<dyn http::Serve>,
}

impl EventDelivery {
    pub fn new(dispatcher: Arc<Dispatcher>, legacy: impl http::Serve + 'static) -> Self {
        Self {
            dispatcher,
            sessions: Arc::default(),
            legacy: Arc::new(legacy),
        }
    }

    async fn control(
        &self,
        ctx: &http::Ctx,
        mut req: http::SimpleIncomingRequest,
    ) -> Result<(), ViewError> {
        let limits = self.dispatcher.wire_limits();
        let mut reader = AsyncFrameReader::raw(
            req.take_async_body()
                .ok_or_else(|| ViewError::BadRequest("Missing resume request".into()))?,
            (*limits).max_frame_bytes(limits.action_request_bytes().min(limits.frame_bytes())),
        );
        let frame = reader
            .next_frame()
            .await
            .map_err(|e| ViewError::BadRequest(e.to_string()))?
            .ok_or_else(|| ViewError::BadRequest("Missing resume frame".into()))?;
        let view = frame.view();
        let resume = view
            .kind_as_frame_request()
            .filter(|r| r.hello().is_none() && r.action().is_none())
            .and_then(|r| r.resume())
            .filter(|r| r.delivery_session() == view.stream_id())
            .ok_or_else(|| ViewError::BadRequest("Invalid resume request".into()))?;
        let active = self
            .sessions
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&resume.delivery_session())
            .cloned()
            .filter(|s| s.principal == ctx.get_cloned::<DeliveryPrincipal>())
            .ok_or_else(|| ViewError::BadRequest("Unknown delivery session".into()))?;
        let config = ctx.get_cloned::<SocketConfig>().unwrap_or_default();
        let replies = active
            .subscriptions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replay(resume, limits, config.max_subscriptions)
            .map_err(|e| ViewError::BadRequest(e.to_string()))?;
        for reply in replies {
            active
                .output
                .send(reply.map_err(|e| ViewError::Render(e.to_string()))?)
                .await
                .map_err(|_| ViewError::BadRequest("Delivery closed".into()))?;
        }
        Ok(())
    }
}

/// Write length-prefixed frames until the listener releases the HTTP stream.
async fn binary_delivery(
    mut conn: http::SharedByteBufferStream<http::RawStream>,
    ctx: &http::Ctx,
    caps: Vec<u8>,
    frames: async_channel::Receiver<Vec<u8>>,
    limits: &WireLimits,
) -> Result<(), ViewError> {
    let config = ctx.get_cloned::<WriterConfig>().unwrap_or_default();
    let interval = config.poll_fallback_interval;
    let mut ready = ReadWait::new(&conn);
    let mut writer = ResponseWriter::new(conn.clone(), config, ctx.clone());
    writer
        .write_all(&crate::delivery::response_head("application/primal-frame")?)
        .await?;
    let closed = async {
        loop {
            ready.readable(interval, ctx).await?;
            match conn.read(&mut [0]) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(ViewError::Write(error)),
                Ok(_) => return Ok(()),
            }
        }
    };
    let send = async {
        let mut next = Some(caps);
        while let Some(bytes) = next.take() {
            let frame =
                OwnedFrame::new(bytes, limits).map_err(|e| ViewError::Render(e.to_string()))?;
            let mut chunk = Vec::with_capacity(frame.as_bytes().len() + 4);
            StreamFrameWriter::length_prefixed(&mut chunk).write_frame(&frame)?;
            writer
                .write_all(&http::Http11Chunk::Chunked(chunk).render())
                .await?;
            writer.flush().await?;
            next = frames.recv().await.ok();
        }
        writer
            .write_all(&http::Http11Chunk::Chunked(Vec::new()).render())
            .await?;
        writer.flush().await?;
        Ok(())
    };
    futures_lite::future::race(closed, send).await
}

impl http::Serve for EventDelivery {
    fn serve(
        &self,
        ctx: http::Ctx,
        req: http::SimpleIncomingRequest,
        conn: http::SharedByteBufferStream<http::RawStream>,
    ) -> http::ConnectionResult {
        let this = self.clone();
        if req.method == http::SimpleMethod::POST {
            return http::ConnectionResult::Async(Box::pin(async move {
                match this.control(&ctx, req).await {
                    Ok(()) => match send_bytes(
                        conn,
                        204,
                        "application/octet-stream",
                        Vec::<u8>::new(),
                        &ctx,
                    )
                    .split()
                    {
                        Ok(write) => write.await,
                        Err(outcome) => outcome,
                    },
                    Err(error) => error_response(&ctx, conn, error).await,
                }
            }));
        }
        let binary = req
            .headers
            .iter()
            .filter(|(name, _)| name.to_string().eq_ignore_ascii_case("accept"))
            .flat_map(|(_, values)| values)
            .any(|value| {
                value.split(',').any(|media| {
                    media
                        .trim()
                        .eq_ignore_ascii_case("application/primal-frame")
                })
            });
        let hello = match hello_header(&req, self.dispatcher.wire_limits()) {
            Ok(Some(hello)) if hello.view().stream_id() == 0 => hello,
            Ok(None) => return self.legacy.serve(ctx, req, conn),
            _ => {
                return http::ConnectionResult::Async(Box::pin(async move {
                    error_response(
                        &ctx,
                        conn,
                        ViewError::BadRequest("Invalid delivery hello".into()),
                    )
                    .await
                }))
            }
        };
        http::ConnectionResult::Async(Box::pin(async move {
            this.stream(ctx, req, conn, hello, binary).await
        }))
    }
}

impl EventDelivery {
    async fn stream(
        &self,
        ctx: http::Ctx,
        req: http::SimpleIncomingRequest,
        conn: http::SharedByteBufferStream<http::RawStream>,
        hello: OwnedFrame,
        binary: bool,
    ) -> http::ServeOutcome {
        let Ok(HelloOutcome::Accepted {
            connection,
            caps_frame,
        }) = self.dispatcher.hello_verified(&ctx, hello.view())
        else {
            return error_response(&ctx, conn, ViewError::UnsupportedVocabulary).await;
        };
        if !connection.subscription_resume() {
            return match self.legacy.serve(ctx, req, conn).split() {
                Ok(pending) => pending.await,
                Err(outcome) => outcome,
            };
        }
        let config = ctx.get_cloned::<SocketConfig>().unwrap_or_default();
        let (output, frames) = async_channel::bounded(config.max_subscriptions.max(1));
        let subscriptions = Subscriptions::new(&ctx, connection.caps());
        let mut leases = super::subscription::FeedLeases::new();
        if let Err(error) = subscriptions.attach(&mut leases, &output) {
            return error_response(&ctx, conn, ViewError::BadRequest(error.to_string())).await;
        }
        let session = Arc::new(Session {
            principal: ctx.get_cloned::<DeliveryPrincipal>(),
            output,
            subscriptions: Mutex::new(subscriptions),
        });
        let id = connection.stream_id();
        let available = {
            use std::collections::hash_map::Entry;
            let mut active = self
                .sessions
                .0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            match active.entry(id) {
                Entry::Vacant(slot) => {
                    slot.insert(session);
                    true
                }
                Entry::Occupied(_) => false,
            }
        };
        if !available {
            return error_response(
                &ctx,
                conn,
                ViewError::Render("Delivery identifier collision".into()),
            )
            .await;
        }
        let _lease = Lease(self.sessions.clone(), id);
        if binary {
            if let Err(error) = binary_delivery(
                conn,
                &ctx,
                caps_frame,
                frames,
                self.dispatcher.wire_limits(),
            )
            .await
            {
                tracing::debug!(%error, "Binary delivery closed");
            }
            return http::ServeOutcome::Close(None);
        }
        let result = async {
            let mut stream = SseStream::new(&conn, &ctx).await?;
            stream.binary(&caps_frame).await?;
            loop {
                let frame = futures_lite::future::race(
                    async { frames.recv().await.ok().map(Some) },
                    async {
                        foundation_core::valtron::timer::sleep_for(config.heartbeat, &ctx).await;
                        Some(None)
                    },
                )
                .await;
                match frame {
                    Some(Some(bytes)) => stream.binary(&bytes).await?,
                    Some(None) => stream.comment("keep-alive").await?,
                    None => break,
                }
            }
            Ok::<(), http::native::upgrade::UpgradeError>(())
        }
        .await;
        if let Err(error) = result {
            tracing::debug!(%error, "Event delivery closed");
        }
        http::ServeOutcome::Close(None)
    }
}
