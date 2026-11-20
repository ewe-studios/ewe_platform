//! Ordered frame delivery and replay for one live stream.
use super::FrameTransport;
use foundation_ui_wire::{
    vocab::VOCABULARY_VERSION,
    writer::{write_messages, ClientCaps, FrameHeader, StreamSequencer, WriteError, WriterConfig},
    MessageT, MountSurface, Platform,
};
use std::{
    collections::VecDeque,
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
    web: Arc<[u8]>,
    native: Arc<[u8]>,
}
impl StoredFrame {
    fn bytes(&self, surface: MountSurface) -> &Arc<[u8]> {
        if surface == MountSurface::Web {
            &self.web
        } else {
            &self.native
        }
    }
}

type Client = (MountSurface, Box<dyn FrameTransport + Send>);
struct Inner {
    clients: Vec<Client>,
    history: VecDeque<StoredFrame>,
    sequence: StreamSequencer,
    backlog: BacklogConfig,
    writer: WriterConfig,
}

impl Inner {
    fn validate(&self, caps: &ClientCaps, resume_from: u64) -> Result<(), LiveError> {
        if caps.vocabulary_version != VOCABULARY_VERSION
            || !matches!(caps.mount, MountSurface::Web | MountSurface::Native)
        {
            return Err(LiveError::UpdateRequired);
        }
        let last = self.sequence.last();
        if resume_from > last {
            return Err(LiveError::ResumeAhead { last });
        }
        if let Some(oldest) = self.history.front().map(|frame| frame.sequence) {
            if resume_from < oldest - 1 {
                return Err(LiveError::ResumeTooOld { oldest });
            }
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
        Self(Arc::new(Mutex::new(Inner {
            clients: Vec::new(),
            history: VecDeque::new(),
            sequence: StreamSequencer::new(stream_id),
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
        self.lock().clients.len()
    }

    /// Disconnect current clients. Keep the history available for resume.
    pub fn close(&self) {
        self.lock().clients.clear();
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
        inner.validate(caps, resume_from)?;
        let replay = inner
            .history
            .iter()
            .filter(|frame| frame.sequence > resume_from)
            .map(|frame| Arc::clone(frame.bytes(caps.mount)))
            .collect();
        inner.clients.push((caps.mount, Box::new(client)));
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
        inner.validate(caps, resume_from)?;
        for frame in inner
            .history
            .iter()
            .filter(|frame| frame.sequence > resume_from)
        {
            if !client.send_binary(frame.bytes(caps.mount)) {
                return Err(LiveError::ConnectionClosed);
            }
        }
        inner.clients.push((caps.mount, Box::new(client)));
        Ok(())
    }
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
        let mut locked = self.0.lock();
        let inner = &mut *locked;
        let sequence = inner
            .sequence
            .last()
            .checked_add(1)
            .ok_or(LiveError::SequenceExhausted)?;
        let header = FrameHeader {
            stream_id: inner.sequence.stream_id(),
            sequence_id: sequence,
            ack: 0,
        };
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
        let size = web.len() + native.len();
        inner.sequence.next();
        inner.history.push_back(StoredFrame {
            sequence,
            web: web.into(),
            native: native.into(),
        });
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
            .retain_mut(|(surface, client)| client.send_binary(frame.bytes(*surface)));
        Ok((sequence, size))
    }
}
