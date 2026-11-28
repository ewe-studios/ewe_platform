//! The client-side producer for idempotent appends.
//!
//! A producer names one writer with a producer identifier and an epoch. It
//! assigns a sequence number to each append and sends the three
//! `Producer-Id`/`Producer-Epoch`/`Producer-Seq` headers. A retry sends the same
//! sequence again, so the server recognizes a duplicate and writes the data
//! once.
//!
//! The producer pipelines requests. Several appends wait in one queue, the
//! producer groups the appends that arrive within [`ProducerConfig::linger`],
//! and it keeps at most [`ProducerConfig::max_in_flight`] requests in flight.
//! One caller drives the queue at a time, and every waiting caller wakes when
//! its result is ready. The wire order, the sequence order and the
//! acknowledgement order stay the same.
//!
//! When the server reports that a newer epoch fenced this producer, an
//! auto-claiming producer asks for the next epoch and starts again at sequence
//! zero. [`Producer::detach`] stops the producer without closing the stream.

use super::core::{AppendResult, ClientError, StreamHandle};
use crate::durable::ContentType;
use foundation_context::{Ctx, Duration};
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Context as TaskContext, Poll, Waker};

/// The default cap for one batch body, in bytes. One mebibyte matches the
/// reference client's default batch size.
pub const DEFAULT_MAX_BATCH_BYTES: usize = 1024 * 1024;
/// The default cap for the number of items in one batch.
pub const DEFAULT_MAX_BATCH_ITEMS: usize = 1000;
/// The default cap for all bytes in one [`Producer::append_batch`] call.
/// Eight mebibytes leaves room for a burst before the caller must wait.
pub const DEFAULT_MAX_QUEUE_BYTES: usize = 8 * 1024 * 1024;
/// The default cap for consecutive epoch claims after fencing.
pub const DEFAULT_MAX_EPOCH_CLAIMS: u32 = 3;
/// The default cap for producer requests in flight at once.
///
/// Four requests hide one round trip for a burst of appends without asking a
/// small server for more work than it can order.
pub const DEFAULT_MAX_IN_FLIGHT: usize = 4;
/// The default time the producer waits to group appends into one batch.
///
/// Five milliseconds groups an ordinary burst without a visible delay. Set the
/// value to zero to send every append at once.
pub const DEFAULT_LINGER: Duration = Duration::from_millis(5);

/// The pipelining configuration of a producer.
///
/// Every limit is a field, so a caller can tune it. The [`Default`]
/// implementation documents the defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProducerConfig {
    /// The most requests the producer keeps in flight. Default:
    /// [`DEFAULT_MAX_IN_FLIGHT`].
    pub max_in_flight: usize,
    /// The longest time the producer waits for more appends before it sends a
    /// batch. Default: [`DEFAULT_LINGER`]. Zero sends at once. A producer whose
    /// client has no [`Ctx`] does not linger.
    pub linger: Duration,
}

impl Default for ProducerConfig {
    fn default() -> Self {
        Self {
            max_in_flight: DEFAULT_MAX_IN_FLIGHT,
            linger: DEFAULT_LINGER,
        }
    }
}

impl ProducerConfig {
    /// Build a configuration with the default limits.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the most requests in flight at once.
    #[must_use]
    pub fn max_in_flight(mut self, requests: usize) -> Self {
        self.max_in_flight = requests;
        self
    }

    /// Set the longest time to wait for more appends before a batch is sent.
    #[must_use]
    pub fn linger(mut self, linger: Duration) -> Self {
        self.linger = linger;
        self
    }
}

/// Builds a [`Producer`] on a stream handle.
pub struct ProducerBuilder {
    handle: StreamHandle,
    producer_id: String,
    epoch: u64,
    auto_claim: bool,
    max_batch_bytes: usize,
    max_batch_items: usize,
    max_queue_bytes: usize,
    max_epoch_claims: u32,
    config: ProducerConfig,
}

impl ProducerBuilder {
    fn new(handle: StreamHandle, producer_id: String) -> Self {
        Self {
            handle,
            producer_id,
            epoch: 0,
            auto_claim: false,
            max_batch_bytes: DEFAULT_MAX_BATCH_BYTES,
            max_batch_items: DEFAULT_MAX_BATCH_ITEMS,
            max_queue_bytes: DEFAULT_MAX_QUEUE_BYTES,
            max_epoch_claims: DEFAULT_MAX_EPOCH_CLAIMS,
            config: ProducerConfig::default(),
        }
    }

    /// Set the starting epoch.
    #[must_use]
    pub fn epoch(mut self, epoch: u64) -> Self {
        self.epoch = epoch;
        self
    }

    /// Claim the next epoch when the server fences this producer.
    #[must_use]
    pub fn auto_claim(mut self, auto_claim: bool) -> Self {
        self.auto_claim = auto_claim;
        self
    }

    /// Set the cap for one batch body, in bytes.
    #[must_use]
    pub fn max_batch_bytes(mut self, bytes: usize) -> Self {
        self.max_batch_bytes = bytes;
        self
    }

    /// Set the cap for the number of items in one batch.
    #[must_use]
    pub fn max_batch_items(mut self, items: usize) -> Self {
        self.max_batch_items = items;
        self
    }

    /// Set the cap for all bytes in one [`Producer::append_batch`] call.
    #[must_use]
    pub fn max_queue_bytes(mut self, bytes: usize) -> Self {
        self.max_queue_bytes = bytes;
        self
    }

    /// Set the cap for consecutive epoch claims after fencing.
    #[must_use]
    pub fn max_epoch_claims(mut self, claims: u32) -> Self {
        self.max_epoch_claims = claims;
        self
    }

    /// Set the most requests in flight at once.
    #[must_use]
    pub fn max_in_flight(mut self, requests: usize) -> Self {
        self.config.max_in_flight = requests;
        self
    }

    /// Set the longest time to wait for more appends before a batch is sent.
    ///
    /// The timer reads the clock of the client's [`Ctx`]. A client without a
    /// `Ctx` sends at once.
    #[must_use]
    pub fn linger(mut self, linger: Duration) -> Self {
        self.config.linger = linger;
        self
    }

    /// Set the whole pipelining configuration.
    #[must_use]
    pub fn config(mut self, config: ProducerConfig) -> Self {
        self.config = config;
        self
    }

    /// Build the producer.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidArgument`] when the producer identifier is
    /// empty or a limit is zero. The message names the field.
    pub fn build(self) -> Result<Producer, ClientError> {
        if self.producer_id.is_empty() {
            return Err(invalid("producerId", "must not be empty"));
        }
        if self.max_batch_bytes == 0 {
            return Err(invalid("maxBatchBytes", "must be more than zero"));
        }
        if self.max_batch_items == 0 {
            return Err(invalid("maxBatchItems", "must be more than zero"));
        }
        if self.max_queue_bytes == 0 {
            return Err(invalid("maxQueueBytes", "must be more than zero"));
        }
        if self.config.max_in_flight == 0 {
            return Err(invalid("maxInFlight", "must be more than zero"));
        }
        let ctx = self.handle.ctx().cloned();
        Ok(Producer {
            shared: Arc::new(Shared {
                id: self.producer_id,
                state: Mutex::new(ProducerState {
                    epoch: self.epoch,
                    sequence: 0,
                    detached: false,
                    epoch_claims: 0,
                    pending: VecDeque::new(),
                    pending_bytes: 0,
                    next_ticket: 0,
                    outcomes: HashMap::new(),
                    wakers: HashMap::new(),
                    flushing: false,
                }),
            }),
            handle: self.handle,
            ctx,
            auto_claim: self.auto_claim,
            max_batch_bytes: self.max_batch_bytes,
            max_batch_items: self.max_batch_items,
            max_queue_bytes: self.max_queue_bytes,
            max_epoch_claims: self.max_epoch_claims,
            config: self.config,
        })
    }
}

/// The items of one batch.
type BatchGroup = Vec<Vec<u8>>;

/// A boxed producer request.
type BatchFuture = Pin<Box<dyn Future<Output = Result<AppendResult, ClientError>> + Send>>;

/// One append waiting for a slot or a send.
enum Queued {
    /// One append that may group with the appends next to it.
    Item {
        data: Vec<u8>,
        content_type: Option<String>,
        ticket: u64,
    },
    /// One pre-formed batch from [`Producer::append_batch`] or
    /// [`Producer::close`].
    Batch {
        body: Vec<u8>,
        content_type: Option<String>,
        close: bool,
        ticket: u64,
    },
    /// A batch that was already assigned an epoch and a sequence, and that a
    /// retry or an epoch claim put back at the front of the queue.
    Prepared(WireBatch),
}

impl Queued {
    /// The bytes this entry adds to the queue.
    fn bytes(&self) -> usize {
        match self {
            Self::Item { data, .. } => data.len(),
            Self::Batch { body, .. } => body.len(),
            Self::Prepared(batch) => batch.bytes,
        }
    }

    /// Record the ticket of this entry.
    fn set_ticket(&mut self, value: u64) {
        match self {
            Self::Item { ticket, .. } | Self::Batch { ticket, .. } => *ticket = value,
            Self::Prepared(_) => {}
        }
    }

    /// The tickets this entry completes.
    fn tickets(&self) -> Vec<u64> {
        match self {
            Self::Item { ticket, .. } | Self::Batch { ticket, .. } => vec![*ticket],
            Self::Prepared(batch) => batch.tickets.clone(),
        }
    }
}

/// One request on the wire.
struct WireBatch {
    epoch: u64,
    sequence: u64,
    body: Vec<u8>,
    content_type: Option<String>,
    close: bool,
    tickets: Vec<u64>,
    attempts: u32,
    bytes: usize,
}

/// The shared, mutable state of one producer.
struct Shared {
    id: String,
    state: Mutex<ProducerState>,
}

/// The state that several append futures share.
struct ProducerState {
    epoch: u64,
    /// The sequence number that the next batch uses.
    sequence: u64,
    detached: bool,
    /// The consecutive epoch claims made after fencing.
    epoch_claims: u32,
    pending: VecDeque<Queued>,
    pending_bytes: usize,
    next_ticket: u64,
    outcomes: HashMap<u64, Result<AppendResult, ClientError>>,
    wakers: HashMap<u64, Waker>,
    /// True while one caller drives the queue.
    flushing: bool,
}

/// One writer with a producer identifier, an epoch and a sequence.
///
/// A clone shares the same epoch, sequence and queue, so several tasks can
/// append through their own clone of one producer. The queue keeps the order
/// of the appends, so the wire and the acknowledgements keep that order too.
#[derive(Clone)]
pub struct Producer {
    shared: Arc<Shared>,
    handle: StreamHandle,
    ctx: Option<Ctx>,
    auto_claim: bool,
    max_batch_bytes: usize,
    max_batch_items: usize,
    max_queue_bytes: usize,
    max_epoch_claims: u32,
    config: ProducerConfig,
}

impl Producer {
    /// The producer identifier.
    #[must_use]
    pub fn producer_id(&self) -> &str {
        &self.shared.id
    }

    /// The current epoch. It changes after an auto-claim.
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.lock().epoch
    }

    /// The sequence number that the next append uses.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.lock().sequence
    }

    /// The pipelining configuration of this producer.
    #[must_use]
    pub const fn config(&self) -> ProducerConfig {
        self.config
    }

    /// True after [`Self::detach`].
    #[must_use]
    pub fn is_detached(&self) -> bool {
        self.lock().detached
    }

    /// Append one message.
    ///
    /// The call waits at most `linger` for more appends and then sends the
    /// group as one batch. The client retries a transient failure with the same
    /// sequence, so the server deduplicates the write. A non-retryable failure
    /// does not advance the sequence.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] for a refused append, an exhausted retry, a full
    /// queue or a producer that is detached.
    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(
            stream = %self.handle.path(),
            producer = %self.shared.id,
            epoch = self.epoch(),
            seq = self.sequence(),
            bytes = data.len(),
        ),
        err
    )]
    pub async fn append(
        &self,
        data: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<AppendResult, ClientError> {
        let ticket = self.enqueue_one(Queued::Item {
            data,
            content_type: content_type.map(str::to_string),
            ticket: 0,
        })?;
        self.await_one(ticket).await
    }

    /// Append several items in batches.
    ///
    /// The call splits `items` into groups, each within `max_batch_items` and
    /// `max_batch_bytes`. A JSON stream receives each group as one array; every
    /// other stream receives the group's bytes joined in order. Each group is
    /// one sequence number. The groups keep their order on the wire and in the
    /// returned results, and the producer keeps several groups in flight at
    /// once.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidArgument`] when the items exceed
    /// `max_queue_bytes`, or any error of [`Self::append`].
    #[tracing::instrument(
        level = "debug",
        skip_all,
        fields(
            stream = %self.handle.path(),
            producer = %self.shared.id,
            epoch = self.epoch(),
            items = items.len(),
            bytes = items.iter().map(Vec::len).sum::<usize>(),
        ),
        err
    )]
    pub async fn append_batch(
        &self,
        items: Vec<Vec<u8>>,
        content_type: Option<&str>,
    ) -> Result<Vec<AppendResult>, ClientError> {
        if items.is_empty() {
            return Ok(Vec::new());
        }
        let total: usize = items.iter().map(Vec::len).sum();
        if total > self.max_queue_bytes {
            return Err(invalid(
                "maxQueueBytes",
                &format!("the batch holds {total} bytes, more than the limit"),
            ));
        }
        let is_json = content_type.is_some_and(is_json_content_type);
        let mut queued = Vec::new();
        for group in self.split_batches(&items) {
            let body = if is_json {
                json_array(&group)
            } else {
                group.concat()
            };
            queued.push(Queued::Batch {
                body,
                content_type: content_type.map(str::to_string),
                close: false,
                ticket: 0,
            });
        }
        let tickets = self.enqueue(queued)?;
        let mut results = Vec::with_capacity(tickets.len());
        for outcome in self.await_tickets(tickets).await {
            results.push(outcome?);
        }
        Ok(results)
    }

    /// Append the final message, if any, and close the stream.
    ///
    /// A repeated close keeps the same sequence, so the server treats it as the
    /// same write. The sequence does not move.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] for a refused close, an exhausted retry or a
    /// producer that is detached.
    #[tracing::instrument(
        level = "info",
        skip_all,
        fields(
            stream = %self.handle.path(),
            producer = %self.shared.id,
            epoch = self.epoch(),
            seq = self.sequence(),
            bytes = data.len(),
        ),
        err
    )]
    pub async fn close(
        &self,
        data: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<AppendResult, ClientError> {
        let ticket = self.enqueue_one(Queued::Batch {
            body: data,
            content_type: content_type.map(str::to_string),
            close: true,
            ticket: 0,
        })?;
        self.await_one(ticket).await
    }

    /// Stop the producer without closing the stream.
    ///
    /// The call sends no request. Appends that still wait fail with
    /// [`ClientError::Detached`], and a later [`Self::append`] does the same.
    pub fn detach(&self) {
        let mut wakers = Vec::new();
        {
            let mut state = self.lock();
            if state.detached {
                return;
            }
            state.detached = true;
            let tickets = drain_pending(&mut state);
            for ticket in tickets {
                state.outcomes.insert(ticket, Err(ClientError::Detached));
                if let Some(waker) = state.wakers.remove(&ticket) {
                    wakers.push(waker);
                }
            }
        }
        wake_all(wakers);
    }

    /// Take the state lock.
    fn lock(&self) -> MutexGuard<'_, ProducerState> {
        self.shared
            .state
            .lock()
            .expect("the producer state lock is not poisoned")
    }

    /// Add entries to the queue and return their tickets.
    fn enqueue(&self, mut queued: Vec<Queued>) -> Result<Vec<u64>, ClientError> {
        let mut state = self.lock();
        if state.detached {
            return Err(ClientError::Detached);
        }
        let added: usize = queued.iter().map(Queued::bytes).sum();
        let queued_bytes = state.pending_bytes.saturating_add(added);
        if queued_bytes > self.max_queue_bytes {
            return Err(invalid(
                "maxQueueBytes",
                &format!("the queue would hold {queued_bytes} bytes, more than the limit"),
            ));
        }
        let mut tickets = Vec::with_capacity(queued.len());
        for entry in &mut queued {
            let ticket = state.next_ticket;
            state.next_ticket = state.next_ticket.saturating_add(1);
            entry.set_ticket(ticket);
            tickets.push(ticket);
        }
        state.pending_bytes = queued_bytes;
        state.pending.extend(queued);
        Ok(tickets)
    }

    /// Add one entry to the queue and return its ticket.
    fn enqueue_one(&self, entry: Queued) -> Result<u64, ClientError> {
        let mut tickets = self.enqueue(vec![entry])?;
        tickets
            .pop()
            .ok_or_else(|| ClientError::Conflict("the producer queue lost the append".to_string()))
    }

    /// Wait for the outcome of one ticket.
    async fn await_one(&self, ticket: u64) -> Result<AppendResult, ClientError> {
        let mut outcomes = self.await_tickets(vec![ticket]).await;
        outcomes.pop().ok_or_else(|| {
            ClientError::Conflict("the producer queue lost the outcome".to_string())
        })?
    }

    /// Wait until every ticket has an outcome.
    async fn await_tickets(&self, tickets: Vec<u64>) -> Vec<Result<AppendResult, ClientError>> {
        let mut results: Vec<Option<Result<AppendResult, ClientError>>> =
            tickets.iter().map(|_| None).collect();
        loop {
            let missing = {
                let mut state = self.lock();
                let mut missing = None;
                for (index, ticket) in tickets.iter().enumerate() {
                    if results[index].is_some() {
                        continue;
                    }
                    if let Some(outcome) = state.outcomes.remove(ticket) {
                        results[index] = Some(outcome);
                    } else if missing.is_none() {
                        missing = Some(*ticket);
                    }
                }
                missing
            };
            let Some(ticket) = missing else {
                break;
            };
            if self.try_become_leader() {
                self.run_flush().await;
            } else {
                self.wait_for(ticket).await;
            }
        }
        results
            .into_iter()
            .map(|result| result.expect("every ticket has an outcome"))
            .collect()
    }

    /// Take the flusher role when no other caller drives the queue.
    fn try_become_leader(&self) -> bool {
        let mut state = self.lock();
        if state.flushing {
            return false;
        }
        state.flushing = true;
        true
    }

    /// Wait until the ticket has an outcome.
    async fn wait_for(&self, ticket: u64) {
        futures_lite::future::poll_fn(|cx| {
            let mut state = self.lock();
            if state.outcomes.contains_key(&ticket) {
                return Poll::Ready(());
            }
            state.wakers.insert(ticket, cx.waker().clone());
            Poll::Pending
        })
        .await;
    }

    /// Drive the queue until it is empty.
    async fn run_flush(&self) {
        let mut lingered = false;
        loop {
            {
                let mut state = self.lock();
                if state.detached {
                    let tickets = drain_pending(&mut state);
                    state.flushing = false;
                    drop(state);
                    self.complete_error(tickets, ClientError::Detached);
                    return;
                }
                if state.pending.is_empty() {
                    state.flushing = false;
                    return;
                }
            }
            if !lingered {
                if let Err(error) = self.wait_linger().await {
                    let tickets = {
                        let mut state = self.lock();
                        let tickets = drain_pending(&mut state);
                        state.flushing = false;
                        tickets
                    };
                    self.complete_error(tickets, error);
                    return;
                }
                lingered = true;
            }
            let window = {
                let mut state = self.lock();
                self.take_window(&mut state)
            };
            if window.is_empty() {
                let mut state = self.lock();
                state.flushing = false;
                return;
            }
            let results = send_window(&self.handle, &self.shared.id, &window).await;
            if !self.apply_window(window, results) {
                let mut state = self.lock();
                state.flushing = false;
                return;
            }
        }
    }

    /// Wait for the linger time, and always yield once so other callers can
    /// enqueue into the same batch.
    async fn wait_linger(&self) -> Result<(), ClientError> {
        futures_lite::future::yield_now().await;
        if self.config.linger.is_zero() {
            return Ok(());
        }
        // A producer without a context does not linger, as a client without a
        // context does not retry. The producer never reads the real clock.
        let Some(ctx) = &self.ctx else {
            return Ok(());
        };
        let timer = ctx
            .try_timer()
            .map_err(|error| ClientError::MissingClock(error.to_string()))?;
        let deadline = ctx.clock().now() + self.config.linger;
        timer.sleep_until(ctx.clone(), deadline).await;
        Ok(())
    }

    /// Pop up to `max_in_flight` batches from the queue, in order.
    fn take_window(&self, state: &mut ProducerState) -> Vec<WireBatch> {
        let mut window = Vec::new();
        while window.len() < self.config.max_in_flight && !state.pending.is_empty() {
            if matches!(state.pending.front(), Some(Queued::Prepared(_))) {
                if let Some(batch) = Self::take_prepared(state) {
                    window.push(batch);
                }
                continue;
            }
            if matches!(state.pending.front(), Some(Queued::Batch { .. })) {
                if let Some(batch) = self.take_batch(state) {
                    window.push(batch);
                }
                continue;
            }
            match self.take_items(state) {
                Some(batch) => window.push(batch),
                None => break,
            }
        }
        window
    }

    /// Pop one prepared batch from the queue.
    fn take_prepared(state: &mut ProducerState) -> Option<WireBatch> {
        let Queued::Prepared(batch) = state.pending.pop_front()? else {
            return None;
        };
        state.pending_bytes = state.pending_bytes.saturating_sub(batch.bytes);
        Some(batch)
    }

    /// Pop one pre-formed batch from the queue and assign its sequence.
    fn take_batch(&self, state: &mut ProducerState) -> Option<WireBatch> {
        let Queued::Batch {
            body,
            content_type,
            close,
            ticket,
        } = state.pending.pop_front()?
        else {
            return None;
        };
        let bytes = body.len();
        state.pending_bytes = state.pending_bytes.saturating_sub(bytes);
        let sequence = state.sequence;
        if !close {
            state.sequence = state.sequence.saturating_add(1);
        }
        Some(WireBatch {
            epoch: state.epoch,
            sequence,
            body,
            content_type,
            close,
            tickets: vec![ticket],
            attempts: 0,
            bytes,
        })
    }

    /// Pop the consecutive single appends that form one batch.
    fn take_items(&self, state: &mut ProducerState) -> Option<WireBatch> {
        let mut parts: Vec<Vec<u8>> = Vec::new();
        let mut tickets = Vec::new();
        let mut content_type: Option<String> = None;
        let mut bytes = 0_usize;
        loop {
            let next = match state.pending.front() {
                Some(Queued::Item {
                    data, content_type, ..
                }) => Some((data.len(), content_type.clone())),
                _ => None,
            };
            let Some((length, next_type)) = next else {
                break;
            };
            if !tickets.is_empty()
                && (next_type != content_type
                    || tickets.len() >= self.max_batch_items
                    || bytes.saturating_add(length) > self.max_batch_bytes)
            {
                break;
            }
            let Some(Queued::Item {
                data,
                content_type: next_type,
                ticket,
            }) = state.pending.pop_front()
            else {
                break;
            };
            content_type = next_type;
            bytes = bytes.saturating_add(data.len());
            parts.push(data);
            tickets.push(ticket);
        }
        if tickets.is_empty() {
            return None;
        }
        state.pending_bytes = state.pending_bytes.saturating_sub(bytes);
        let body = if content_type.as_deref().is_some_and(is_json_content_type) {
            json_array(&parts)
        } else {
            parts.concat()
        };
        let sequence = state.sequence;
        state.sequence = state.sequence.saturating_add(1);
        Some(WireBatch {
            epoch: state.epoch,
            sequence,
            body,
            content_type,
            close: false,
            tickets,
            attempts: 0,
            bytes,
        })
    }

    /// Record the results of one window, and decide whether to continue.
    ///
    /// The results arrive in wire order. A later success proves that the server
    /// accepted every earlier sequence, so the producer sends a failed earlier
    /// batch again with the same sequence and the server answers a duplicate.
    /// A fencing `403` claims the next epoch and numbers the failed batches
    /// again from zero.
    fn apply_window(
        &self,
        window: Vec<WireBatch>,
        results: Vec<Result<AppendResult, ClientError>>,
    ) -> bool {
        if results.len() != window.len() {
            return false;
        }
        let mut completed: Vec<(u64, Result<AppendResult, ClientError>)> = Vec::new();
        let mut failed: Vec<(usize, ClientError)> = Vec::new();
        let mut success: Vec<bool> = Vec::with_capacity(results.len());
        for (index, result) in results.into_iter().enumerate() {
            success.push(result.is_ok());
            match result {
                Ok(outcome) => {
                    for ticket in &window[index].tickets {
                        completed.push((*ticket, Ok(outcome.clone())));
                    }
                }
                Err(error) => failed.push((index, error)),
            }
        }
        self.complete_many(completed);
        if failed.is_empty() {
            let mut state = self.lock();
            state.epoch_claims = 0;
            return true;
        }
        let first_index = failed[0].0;
        let first_error = failed[0].1.clone();
        let (window, claimed) = self.claim_epoch(window, &failed, &first_error);
        if claimed {
            return true;
        }
        let (window, retried) = self.retry_failed(window, &failed);
        if retried {
            return true;
        }
        self.fail_window(window, failed, &success, first_index, &first_error);
        false
    }

    /// Claim the next epoch after a fencing `403` and renumber the failed
    /// batches from zero.
    fn claim_epoch(
        &self,
        window: Vec<WireBatch>,
        failed: &[(usize, ClientError)],
        first_error: &ClientError,
    ) -> (Vec<WireBatch>, bool) {
        let Some(first_index) = failed.first().map(|(index, _)| *index) else {
            return (window, false);
        };
        let Some(next_epoch) = fenced_next_epoch(first_error, window[first_index].epoch) else {
            return (window, false);
        };
        if !self.auto_claim {
            return (window, false);
        }
        let mut state = self.lock();
        if state.epoch_claims >= self.max_epoch_claims {
            return (window, false);
        }
        state.epoch_claims = state.epoch_claims.saturating_add(1);
        state.epoch = next_epoch;
        state.sequence = 0;
        let failed_indices: Vec<usize> = failed.iter().map(|(index, _)| *index).collect();
        let mut batches: Vec<WireBatch> = window
            .into_iter()
            .enumerate()
            .filter(|(index, _)| failed_indices.contains(index))
            .map(|(_, batch)| batch)
            .collect();
        let mut sequence = 0_u64;
        for batch in &mut batches {
            batch.epoch = next_epoch;
            batch.sequence = sequence;
            if !batch.close {
                sequence = sequence.saturating_add(1);
            }
        }
        state.sequence = sequence;
        state.pending_bytes = state
            .pending_bytes
            .saturating_add(batches.iter().map(|batch| batch.bytes).sum::<usize>());
        for batch in batches.into_iter().rev() {
            state.pending.push_front(Queued::Prepared(batch));
        }
        (Vec::new(), true)
    }

    /// Return the failed batches to the front of the queue for one more round.
    fn retry_failed(
        &self,
        window: Vec<WireBatch>,
        failed: &[(usize, ClientError)],
    ) -> (Vec<WireBatch>, bool) {
        let retryable = failed
            .iter()
            .all(|(_, error)| matches!(error, ClientError::Conflict(_)));
        let limit = self.handle.retry_policy().attempt_limit();
        let attempts_remain = failed
            .iter()
            .all(|(index, _)| window[*index].attempts < limit);
        if !retryable || !attempts_remain {
            return (window, false);
        }
        let failed_indices: Vec<usize> = failed.iter().map(|(index, _)| *index).collect();
        let mut batches: Vec<WireBatch> = window
            .into_iter()
            .enumerate()
            .filter(|(index, _)| failed_indices.contains(index))
            .map(|(_, mut batch)| {
                batch.attempts = batch.attempts.saturating_add(1);
                batch
            })
            .collect();
        let mut state = self.lock();
        state.pending_bytes = state
            .pending_bytes
            .saturating_add(batches.iter().map(|batch| batch.bytes).sum::<usize>());
        for batch in batches.drain(..).rev() {
            state.pending.push_front(Queued::Prepared(batch));
        }
        (Vec::new(), true)
    }

    /// Fail a window that cannot recover, and everything that still waits.
    ///
    /// The sequence returns to the first failure unless a later batch proved
    /// that the server accepted it. A later success proves that the server wrote
    /// the failed batch, because the protocol accepts only the next sequence.
    fn fail_window(
        &self,
        window: Vec<WireBatch>,
        failed: Vec<(usize, ClientError)>,
        success: &[bool],
        first_index: usize,
        first_error: &ClientError,
    ) {
        let mut outcomes = Vec::new();
        for (index, error) in failed {
            for ticket in &window[index].tickets {
                outcomes.push((*ticket, Err(error.clone())));
            }
        }
        let later_success = success
            .iter()
            .enumerate()
            .any(|(index, ok)| *ok && index > first_index);
        let mut state = self.lock();
        if !later_success {
            state.sequence = window[first_index].sequence;
        }
        let waiting = drain_pending(&mut state);
        let message = format!("an earlier producer batch failed: {first_error}");
        for ticket in waiting {
            outcomes.push((ticket, Err(ClientError::Conflict(message.clone()))));
        }
        state.flushing = false;
        drop(state);
        self.complete_many(outcomes);
    }

    /// Complete the tickets of a set of batches with one failure.
    fn complete_error(&self, tickets: Vec<u64>, error: ClientError) {
        let outcomes = tickets
            .into_iter()
            .map(|ticket| (ticket, Err(error.clone())))
            .collect();
        self.complete_many(outcomes);
    }

    /// Complete tickets and wake their waiters.
    fn complete_many(&self, outcomes: Vec<(u64, Result<AppendResult, ClientError>)>) {
        if outcomes.is_empty() {
            return;
        }
        let mut wakers = Vec::new();
        {
            let mut state = self.lock();
            for (ticket, outcome) in outcomes {
                state.outcomes.insert(ticket, outcome);
                if let Some(waker) = state.wakers.remove(&ticket) {
                    wakers.push(waker);
                }
            }
        }
        wake_all(wakers);
    }

    /// Split items into groups that obey both batch limits.
    fn split_batches(&self, items: &[Vec<u8>]) -> Vec<BatchGroup> {
        let mut groups = Vec::new();
        let mut current: Vec<Vec<u8>> = Vec::new();
        let mut bytes = 0_usize;
        for item in items {
            let length = item.len();
            let full = !current.is_empty()
                && (current.len() >= self.max_batch_items
                    || bytes.saturating_add(length) > self.max_batch_bytes);
            if full {
                groups.push(std::mem::take(&mut current));
                bytes = 0;
            }
            bytes = bytes.saturating_add(length);
            current.push(item.clone());
        }
        if !current.is_empty() {
            groups.push(current);
        }
        groups
    }
}

/// Drain the queue and return the tickets that still wait.
fn drain_pending(state: &mut ProducerState) -> Vec<u64> {
    let mut tickets = Vec::new();
    while let Some(entry) = state.pending.pop_front() {
        tickets.extend(entry.tickets());
    }
    state.pending_bytes = 0;
    tickets
}

/// Wake every stored waker.
fn wake_all(wakers: Vec<Waker>) {
    for waker in wakers {
        waker.wake();
    }
}

/// Send one window of batches, polling every request at once.
async fn send_window(
    handle: &StreamHandle,
    producer_id: &str,
    window: &[WireBatch],
) -> Vec<Result<AppendResult, ClientError>> {
    let mut futures: Vec<Option<BatchFuture>> = Vec::with_capacity(window.len());
    for batch in window {
        let handle = handle.clone();
        let producer_id = producer_id.to_string();
        let body = batch.body.clone();
        let content_type = batch.content_type.clone();
        let epoch = batch.epoch;
        let sequence = batch.sequence;
        let close = batch.close;
        futures.push(Some(Box::pin(async move {
            handle
                .append(
                    body,
                    content_type.as_deref(),
                    None,
                    Some((&producer_id, epoch, sequence)),
                    close,
                )
                .await
        })));
    }
    JoinAll {
        futures,
        results: window.iter().map(|_| None).collect(),
    }
    .await
}

/// A join that polls every future in one pass, so they run at the same time.
struct JoinAll {
    futures: Vec<Option<BatchFuture>>,
    results: Vec<Option<Result<AppendResult, ClientError>>>,
}

impl Future for JoinAll {
    type Output = Vec<Result<AppendResult, ClientError>>;

    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let mut pending = false;
        for index in 0..this.futures.len() {
            if let Some(future) = this.futures[index].as_mut() {
                match future.as_mut().poll(cx) {
                    Poll::Ready(result) => {
                        this.results[index] = Some(result);
                        this.futures[index] = None;
                    }
                    Poll::Pending => pending = true,
                }
            }
        }
        if pending {
            return Poll::Pending;
        }
        Poll::Ready(
            this.results
                .iter_mut()
                .map(|result| {
                    result
                        .take()
                        .expect("every joined future returned a result")
                })
                .collect(),
        )
    }
}

impl StreamHandle {
    /// Build a producer for this stream.
    #[must_use]
    pub fn producer(&self, producer_id: impl Into<String>) -> ProducerBuilder {
        ProducerBuilder::new(self.clone(), producer_id.into())
    }
}

/// The next epoch to claim after a fencing `403`, when the failure is one.
///
/// The server reports its current epoch in `Producer-Epoch`. The claim is one
/// above the server value, and never below one above the current epoch.
fn fenced_next_epoch(error: &ClientError, current: u64) -> Option<u64> {
    let ClientError::UnexpectedStatus {
        status: 403,
        headers,
        ..
    } = error
    else {
        return None;
    };
    let server_epoch = headers
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("producer-epoch"))
        .and_then(|(_, value)| value.parse::<u64>().ok());
    let next = server_epoch.map_or(0, |value| value.saturating_add(1));
    Some(next.max(current.saturating_add(1)))
}

/// True when the content type is JSON, ignoring parameters.
fn is_json_content_type(value: &str) -> bool {
    ContentType::parse(value).is_ok_and(|content_type| content_type.is_json())
}

/// Build one JSON array body from the items of a batch.
fn json_array(items: &[Vec<u8>]) -> Vec<u8> {
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

/// Build an invalid-argument failure for one field.
fn invalid(field: &str, message: &str) -> ClientError {
    ClientError::InvalidArgument {
        field: field.to_string(),
        message: message.to_string(),
    }
}
