//! Ordered frame delivery and replay for one live stream.
use super::FrameTransport;
use foundation_ui_wire::{
    vocab::{vocab_version, VOCABULARY_VERSION},
    writer::{write_messages, ClientCaps, FrameHeader, StreamSequencer, WriteError, WriterConfig},
    MessageT, MountSurface, Platform,
};
use std::{
    collections::{BTreeMap, VecDeque},
    fmt,
    prelude::rust_2021::*,
    sync::{Arc, Mutex, MutexGuard},
};

/// Retained replay frames. The default keeps the whole history for a late joiner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BacklogConfig {
    limit: Option<usize>,
}

impl BacklogConfig {
    /// Set the maximum number of retained frames. A zero limit becomes one.
    #[must_use]
    pub fn max_frames(self, count: usize) -> Self {
        Self {
            limit: Some(count.max(1)),
        }
    }

    /// Retain all frames.
    #[must_use]
    pub fn unbounded(self) -> Self {
        Self::default()
    }

    /// Return the configured frame limit.
    #[must_use]
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }
}

/// The channel could not encode, replay, or deliver a frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LiveError {
    UpdateRequired,
    ResumeTooOld { oldest: u64 },
    ResumeAhead { last: u64 },
    ConnectionClosed,
    SequenceExhausted,
    Write(WriteError),
}

impl fmt::Display for LiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UpdateRequired => f.write_str("unsupported client vocabulary or mount surface"),
            Self::ResumeTooOld { oldest } => write!(f, "replay begins at frame {oldest}"),
            Self::ResumeAhead { last } => write!(f, "resume cursor exceeds last frame {last}"),
            Self::ConnectionClosed => f.write_str("connection closed during replay"),
            Self::SequenceExhausted => f.write_str("stream sequence exhausted"),
            Self::Write(err) => err.fmt(f),
        }
    }
}
impl std::error::Error for LiveError {}

struct StoredFrame {
    sequence: u64,
    encodings: BTreeMap<(u8, u32), Arc<[u8]>>,
}
impl StoredFrame {
    fn encode_for(
        &mut self,
        caps: &ClientCaps,
        config: &WriterConfig,
    ) -> Result<Arc<[u8]>, LiveError> {
        let key = (caps.mount.0, caps.vocabulary_version);
        if let Some(bytes) = self.encodings.get(&key) {
            return Ok(bytes.clone());
        }
        let current = &self.encodings[&(caps.mount.0, VOCABULARY_VERSION)];
        let frame = foundation_ui_wire::read_frame(current, config.wire_limits())
            .map_err(|error| LiveError::Write(error.into()))?;
        // The channel creates every retained frame with the message writer.
        let body = frame
            .kind_as_frame_message()
            .expect("retained message frame");
        let messages = body
            .messages()
            .into_iter()
            .flatten()
            .map(|message| message.unpack())
            .collect();
        let header = FrameHeader {
            stream_id: frame.stream_id(),
            sequence_id: self.sequence,
            ack: body.ack(),
        };
        let (bytes, _) =
            write_messages(messages, caps, header, config).map_err(LiveError::Write)?;
        Ok(self
            .encodings
            .entry(key)
            .or_insert_with(|| bytes.into())
            .clone())
    }

    fn prepared(&self, caps: &ClientCaps) -> &Arc<[u8]> {
        &self.encodings[&(caps.mount.0, caps.vocabulary_version)]
    }
}

type Client = (ClientCaps, Box<dyn FrameTransport + Send>);
struct Inner {
    clients: Vec<Client>,
    managed: Vec<(
        Arc<()>,
        ClientCaps,
        foundation_ui_runtime::dispatch::LiveSink,
    )>,
    history: VecDeque<StoredFrame>,
    sequence: StreamSequencer,
    backlog: BacklogConfig,
    writer: WriterConfig,
}

impl Inner {
    fn replay(&mut self, caps: &ClientCaps, after: u64) -> Result<Vec<Arc<[u8]>>, LiveError> {
        self.validate(caps, after)?;
        self.history
            .iter_mut()
            .filter(|frame| frame.sequence > after)
            .map(|frame| frame.encode_for(caps, &self.writer))
            .collect()
    }

    fn validate(&self, caps: &ClientCaps, resume_from: u64) -> Result<(), LiveError> {
        if vocab_version(caps.vocabulary_version).is_none()
            || !matches!(caps.mount, MountSurface::Web | MountSurface::Native)
        {
            return Err(LiveError::UpdateRequired);
        }
        let last = self.sequence.last();
        if resume_from > last {
            return Err(LiveError::ResumeAhead { last });
        }
        let floor = self.history.front().map_or(last, |frame| frame.sequence - 1);
        if resume_from < floor {
            return Err(LiveError::ResumeTooOld {
                oldest: floor.saturating_add(1),
            });
        }
        Ok(())
    }
}

/// A channel shares ordered frames between a producer and its connected clients.
#[derive(Clone)]
pub struct LiveChannel(Arc<Mutex<Inner>>);

impl LiveChannel {
    #[must_use]
    pub fn new(stream_id: u64, backlog: BacklogConfig, writer: WriterConfig) -> Self {
        Self::resume(stream_id, 0, backlog, writer)
    }

    /// Open an empty replay history at the last durable sequence number.
    /// Older clients must request the current snapshot before they subscribe.
    #[must_use]
    pub fn resume(stream_id: u64, last: u64, backlog: BacklogConfig, writer: WriterConfig) -> Self {
        Self(Arc::new(Mutex::new(Inner {
            clients: Vec::new(),
            managed: Vec::new(),
            history: VecDeque::new(),
            sequence: StreamSequencer::resume(stream_id, last),
            backlog,
            writer,
        })))
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.0.lock().expect("live channel lock poisoned")
    }

    #[must_use]
    pub fn stream_id(&self) -> u64 {
        self.lock().sequence.stream_id()
    }
    #[must_use]
    pub fn sender(&self) -> LiveTx {
        LiveTx(self.clone())
    }
    #[must_use]
    pub fn last_sequence(&self) -> u64 {
        self.lock().sequence.last()
    }
    #[must_use]
    pub fn backlog_len(&self) -> usize {
        self.lock().history.len()
    }
    #[must_use]
    pub fn conn_count(&self) -> usize {
        let inner = self.lock();
        inner.clients.len() + inner.managed.len()
    }

    /// Disconnect current clients. Keep the history available for resume.
    pub fn close(&self) {
        let mut inner = self.lock();
        inner.clients.clear();
        for (_, _, mut send) in inner.managed.drain(..) {
            send(foundation_ui_runtime::dispatch::FeedEvent::Disconnected);
        }
    }

    /// Attach a web transport and return the retained frames that precede its live queue.
    /// A socket driver writes these shared frames before it reads from its bounded queue.
    ///
    /// # Errors
    /// Returns a cursor error before the channel attaches the transport.
    pub fn subscribe_web(
        &self,
        client: impl FrameTransport + Send + 'static,
        resume_from: u64,
    ) -> Result<Vec<Arc<[u8]>>, LiveError> {
        self.subscribe(client, &ClientCaps::web_current(), resume_from)
    }

    /// Subscribes a native renderer and returns its replay before its live frames.
    ///
    /// # Errors
    /// Returns an error if the client cannot resume from this cursor.
    pub fn subscribe_native(
        &self,
        client: impl FrameTransport + Send + 'static,
        resume_from: u64,
    ) -> Result<Vec<Arc<[u8]>>, LiveError> {
        self.subscribe(
            client,
            &ClientCaps::native_current(Platform::Unknown),
            resume_from,
        )
    }

    fn subscribe(
        &self,
        client: impl FrameTransport + Send + 'static,
        caps: &ClientCaps,
        resume_from: u64,
    ) -> Result<Vec<Arc<[u8]>>, LiveError> {
        let mut inner = self.lock();
        let replay = inner.replay(caps, resume_from)?;
        inner.clients.push((caps.clone(), Box::new(client)));
        Ok(replay)
    }

    /// Replay frames after the cursor and attach the client to the stream.
    ///
    /// # Errors
    /// Returns an error for unsupported clients, invalid cursors, or failed replay writes.
    pub fn register(
        &self,
        mut client: impl FrameTransport + Send + 'static,
        caps: &ClientCaps,
        resume_from: u64,
    ) -> Result<(), LiveError> {
        let mut inner = self.lock();
        for frame in inner.replay(caps, resume_from)? {
            if !client.send_binary(&frame) {
                return Err(LiveError::ConnectionClosed);
            }
        }
        inner.clients.push((caps.clone(), Box::new(client)));
        Ok(())
    }
}

/// A shared tree needs one encoding. Mixed or columnar batches use the surface writer.
fn portable_operation(op: &foundation_ui_wire::OperationT) -> bool {
    use foundation_ui_wire::{MobileOperationT, UiOperationT, WebOperationT};
    if !matches!(op.web, WebOperationT::NONE)
        || !matches!(op.mobile, MobileOperationT::NONE)
        || op.columnar.is_some()
    {
        return false;
    }
    let nested = match &op.ui {
        UiOperationT::Place(p) => p.operations.as_deref(),
        UiOperationT::ReplaceSubtree(p) => p.operations.as_deref(),
        _ => None,
    };
    nested.is_none_or(|ops| ops.iter().all(portable_operation))
}

/// A producer handle for one live channel.
#[derive(Clone)]
pub struct LiveTx(LiveChannel);

impl LiveTx {
    /// Encode the whole update for each surface, then store and deliver it.
    ///
    /// # Errors
    /// Returns a writer or sequence error before the channel publishes the update.
    pub fn push_messages(&self, messages: Vec<MessageT>) -> Result<u64, LiveError> {
        self.publish(messages).map(|(sequence, _)| sequence)
    }

    pub(super) fn publish(&self, messages: Vec<MessageT>) -> Result<(u64, usize), LiveError> {
        self.publish_with(|_| Ok(messages))
    }

    /// Build an update with its assigned stream position before publishing it.
    /// The builder must not call this channel while it holds the position.
    ///
    /// # Errors
    /// A builder or writer error leaves the cursor and history unchanged.
    pub fn push_with<E: From<LiveError>>(
        &self,
        build: impl FnOnce(u64) -> Result<Vec<MessageT>, E>,
    ) -> Result<u64, E> {
        self.publish_with(build).map(|(position, _)| position)
    }

    fn publish_with<E: From<LiveError>>(
        &self,
        build: impl FnOnce(u64) -> Result<Vec<MessageT>, E>,
    ) -> Result<(u64, usize), E> {
        let mut locked = self.0.lock();
        let inner = &mut *locked;
        let sequence = inner
            .sequence
            .last()
            .checked_add(1)
            .ok_or(LiveError::SequenceExhausted)?;
        let messages = build(sequence)?;
        let header = FrameHeader {
            stream_id: inner.sequence.stream_id(),
            sequence_id: sequence,
            ack: 0,
        };
        let shared = messages
            .iter()
            .all(|m| m.operation.as_deref().is_none_or(portable_operation));
        let (web, native): (Arc<[u8]>, Arc<[u8]>) = if shared {
            let (bytes, _) =
                write_messages(messages, &ClientCaps::web_current(), header, &inner.writer)
                    .map_err(LiveError::Write)?;
            let bytes: Arc<[u8]> = bytes.into();
            (bytes.clone(), bytes)
        } else {
            let (web, _) = write_messages(
                messages.clone(),
                &ClientCaps::web_current(),
                header,
                &inner.writer,
            )
            .map_err(LiveError::Write)?;
            let (native, _) = write_messages(
                messages,
                &ClientCaps::native_current(Platform::Unknown),
                header,
                &inner.writer,
            )
            .map_err(LiveError::Write)?;
            (web.into(), native.into())
        };
        let size = web.len() + native.len();
        let mut frame = StoredFrame {
            sequence,
            encodings: BTreeMap::from([
                ((MountSurface::Web.0, VOCABULARY_VERSION), web),
                ((MountSurface::Native.0, VOCABULARY_VERSION), native),
            ]),
        };
        // Prepare every active version before advancing the stream position.
        for caps in inner
            .clients
            .iter()
            .map(|(caps, _)| caps)
            .chain(inner.managed.iter().map(|(_, caps, _)| caps))
        {
            frame.encode_for(caps, &inner.writer)?;
        }
        inner.sequence.next();
        inner.history.push_back(frame);
        if inner
            .backlog
            .limit()
            .is_some_and(|limit| inner.history.len() > limit)
        {
            inner.history.pop_front();
        }
        let frame = inner.history.back().expect("published frame is retained");
        inner
            .clients
            .retain_mut(|(surface, client)| client.send_binary(frame.prepared(surface)));
        inner.managed.retain_mut(|(_, surface, deliver)| {
            deliver(foundation_ui_runtime::dispatch::FeedEvent::Frame(
                frame.prepared(surface),
            ))
        });
        Ok((sequence, size))
    }
}

impl foundation_ui_runtime::dispatch::ReplayFeed for LiveChannel {
    fn stream_id(&self) -> u64 {
        LiveChannel::stream_id(self)
    }

    fn listen(
        &self,
        caps: &ClientCaps,
        send: foundation_ui_runtime::dispatch::LiveSink,
    ) -> Result<
        foundation_ui_runtime::dispatch::FeedLease,
        foundation_ui_runtime::dispatch::ReplayError,
    > {
        let token = Arc::new(());
        let mut inner = self.lock();
        inner
            .validate(caps, inner.sequence.last())
            .map_err(replay_error)?;
        inner.managed.push((token.clone(), caps.clone(), send));
        let channel = Arc::downgrade(&self.0);
        Ok(foundation_ui_runtime::dispatch::FeedLease::new(move || {
            if let Some(inner) = channel.upgrade() {
                inner
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .managed
                    .retain(|(id, _, _)| !Arc::ptr_eq(id, &token));
            }
        }))
    }

    fn replay(
        &self,
        caps: &ClientCaps,
        after: u64,
    ) -> Result<Vec<Arc<[u8]>>, foundation_ui_runtime::dispatch::ReplayError> {
        self.lock().replay(caps, after).map_err(replay_error)
    }
}
fn replay_error(error: LiveError) -> foundation_ui_runtime::dispatch::ReplayError {
    match error {
        LiveError::ResumeTooOld { .. } | LiveError::ResumeAhead { .. } => {
            foundation_ui_runtime::dispatch::ReplayError::Expired
        }
        other => foundation_ui_runtime::dispatch::ReplayError::Unavailable(other.to_string()),
    }
}
