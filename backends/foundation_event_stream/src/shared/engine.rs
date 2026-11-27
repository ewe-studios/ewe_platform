//! The transport-neutral subscription engine.
//!
//! The engine owns the subscription records, the generation fence, the lease, the wake snapshot,
//! the webhook delivery and the pull-wake flow. It reads the clock and the entropy source from
//! [`foundation_context::Ctx`], stores state in a [`foundation_core::durable::DurableStreamVfs`],
//! and calls the webhook transport that the host supplies.

use crate::shared::config::{normalize_relative_path, SubscriptionConfig};
use crate::shared::error::{SubscriptionError, SubscriptionErrorCode};
use crate::shared::glob::glob_match;
use crate::shared::hash::stable_config_hash;
use crate::shared::model::{
    AckEntry, AckRequest, AckResponse, ClaimResponse, CreateRequest, LinkType, StreamInfo,
    StreamLink, StreamView, SubscriptionCreate, SubscriptionRecord, SubscriptionStatus,
    SubscriptionType, SubscriptionView, WakeEvent, WakeNotification, WebhookCreate,
    WebhookSigningView, WebhookView,
};
use crate::shared::offsets::{compare_offsets, offset_from_len, zero_offset, BEFORE_FIRST};
use crate::shared::signing::WebhookSigningKey;
use crate::shared::store::{ObservingVfs, StreamWriteObserver, SubscriptionStore};
use crate::shared::url::validate_webhook_url;
use crate::shared::webhook::{WebhookRequest, WebhookResponse, WebhookSender};
use foundation_compact::rng::Rng;
use foundation_compact::time::unix_millis;
use foundation_context::Ctx;
use foundation_core::durable::{
    wire, AppendRequest, Chunk, DurableStreamStore, DurableStreamVfs, StoreError, StreamName,
    VfsFuture,
};
use serde::Deserialize;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};

/// The flag a webhook returns to finish synchronously.
#[derive(Debug, Deserialize)]
struct DoneFlag {
    #[serde(default)]
    done: bool,
}

/// A hook that a host runs after the engine queues a webhook delivery.
///
/// The engine never waits for it. A host uses it to start a worker without waiting for the next
/// [`SubscriptionEngine::tick`]. The hook must return quickly and must not block.
pub trait DeliveryNotifier: Send + Sync {
    /// Runs once for each delivery that enters the queue.
    fn delivery_queued(&self);
}

/// The result of a queue push.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueuePush {
    /// The delivery entered the queue.
    Queued,
    /// The subscription already has a queued delivery.
    Duplicate,
    /// The queue is at its bound.
    Full,
}

/// The bounded first-in-first-out queue of subscription identifiers.
struct DeliveryQueue {
    bound: usize,
    state: Mutex<QueueState>,
    drops: AtomicU64,
}

/// The queue order and the identifiers it holds, so a push is idempotent.
#[derive(Default)]
struct QueueState {
    order: VecDeque<String>,
    queued: BTreeSet<String>,
}

impl DeliveryQueue {
    fn new(bound: usize) -> Self {
        Self {
            bound: bound.max(1),
            state: Mutex::new(QueueState::default()),
            drops: AtomicU64::new(0),
        }
    }

    fn bound(&self) -> usize {
        self.bound
    }

    /// The number of pushes that a full queue refused.
    fn drops(&self) -> u64 {
        self.drops.load(AtomicOrdering::Relaxed)
    }

    fn push(&self, id: &str) -> QueuePush {
        let mut state = self.lock();
        if state.queued.contains(id) {
            return QueuePush::Duplicate;
        }
        if state.order.len() >= self.bound {
            self.drops.fetch_add(1, AtomicOrdering::Relaxed);
            return QueuePush::Full;
        }
        state.order.push_back(id.to_string());
        state.queued.insert(id.to_string());
        QueuePush::Queued
    }

    fn pop(&self) -> Option<String> {
        let mut state = self.lock();
        let id = state.order.pop_front()?;
        state.queued.remove(&id);
        Some(id)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, QueueState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The result of one bounded webhook attempt.
enum DeliveryOutcome {
    /// The sender answered or failed.
    Answered(Result<WebhookResponse, String>),
    /// The delivery timeout passed before the sender answered.
    TimedOut,
}

/// The append observer that the engine installs on the observed virtual file system (VFS).
struct EngineObserver {
    engine: SubscriptionEngine,
}

impl StreamWriteObserver for EngineObserver {
    fn stream_written(&self, name: StreamName) -> VfsFuture<'_, ()> {
        let engine = self.engine.clone();
        Box::pin(async move {
            engine.on_stream_written(&name).await;
            Ok(())
        })
    }
}

/// The subscription engine.
///
/// A create, a confirm and an append never wait for a webhook receiver. They record the wake and
/// put the subscription identifier in a queue. A worker drains the queue in
/// [`SubscriptionEngine::tick`], or when the host calls
/// [`SubscriptionEngine::drain_deliveries`] from its own executor.
#[derive(Clone)]
pub struct SubscriptionEngine {
    store: Arc<SubscriptionStore>,
    ctx: Ctx,
    signing: Arc<WebhookSigningKey>,
    sender: Arc<dyn WebhookSender>,
    queue: Arc<DeliveryQueue>,
    /// The current wake of each subscription that a delivery attempt already took.
    ///
    /// The sweep after a drain uses this to skip a wake that waits for a callback. Without it,
    /// the sweep would deliver the same open wake on every tick.
    attempted: Arc<Mutex<BTreeMap<String, String>>>,
    notifier: Option<Arc<dyn DeliveryNotifier>>,
}

impl SubscriptionEngine {
    /// Build the engine.
    ///
    /// The host supplies the raw virtual file system (VFS), the context, the signing key and the
    /// webhook transport. The engine reads no global.
    #[must_use]
    pub fn new(
        vfs: Arc<dyn DurableStreamVfs>,
        ctx: Ctx,
        signing: WebhookSigningKey,
        sender: Arc<dyn WebhookSender>,
        config: SubscriptionConfig,
    ) -> Self {
        let queue = Arc::new(DeliveryQueue::new(config.delivery_queue_bound));
        let store = Arc::new(SubscriptionStore::new(vfs, &ctx, config));
        Self {
            store,
            ctx,
            signing: Arc::new(signing),
            sender,
            queue,
            attempted: Arc::new(Mutex::new(BTreeMap::new())),
            notifier: None,
        }
    }

    /// Set the hook that the engine calls after it queues a delivery.
    ///
    /// The hook lets the host start a worker at once. Without it, the host drains the queue on
    /// its [`SubscriptionEngine::tick`] schedule.
    #[must_use]
    pub fn with_delivery_notifier(mut self, notifier: Arc<dyn DeliveryNotifier>) -> Self {
        self.notifier = Some(notifier);
        self
    }

    /// The configuration.
    #[must_use]
    pub fn config(&self) -> &SubscriptionConfig {
        self.store.config()
    }

    /// The subscription store.
    #[must_use]
    pub fn store(&self) -> &SubscriptionStore {
        &self.store
    }

    /// The webhook signing key.
    #[must_use]
    pub fn signing(&self) -> &WebhookSigningKey {
        &self.signing
    }

    /// The context.
    #[must_use]
    pub fn context(&self) -> &Ctx {
        &self.ctx
    }

    /// Wrap the raw virtual file system (VFS) so appends reach the engine.
    ///
    /// The host builds its `DurableStream` on this wrapper. The engine itself keeps using the raw
    /// virtual file system (VFS), so its own state writes do not wake subscriptions.
    #[must_use]
    pub fn observed_vfs(&self) -> Arc<dyn DurableStreamVfs> {
        ObservingVfs::new(
            self.store.raw_vfs().clone(),
            Arc::new(EngineObserver {
                engine: self.clone(),
            }),
        )
    }

    /// Read the persisted subscriptions. The call is idempotent.
    ///
    /// # Errors
    ///
    /// Returns the storage error of the state read.
    pub async fn load(&self) -> Result<(), String> {
        self.store.load().await
    }

    /// The published JSON Web Key Set (JWKS) as text.
    ///
    /// # Errors
    ///
    /// Returns an error when the key set cannot be serialized.
    pub fn jwks_json(&self) -> Result<String, SubscriptionError> {
        self.signing.jwks_json()
    }

    /// Create a subscription or re-confirm an identical one.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionErrorCode::InvalidRequest`] for a bad body,
    /// [`SubscriptionErrorCode::WebhookUrlRejected`] for an unsafe webhook uniform resource
    /// locator (URL), and [`SubscriptionErrorCode::SubscriptionAlreadyExists`] when the same
    /// identifier has a different configuration.
    pub async fn create_or_confirm(
        &self,
        id: &str,
        request: CreateRequest,
        request_base_url: &str,
    ) -> Result<(SubscriptionView, bool), SubscriptionError> {
        self.ensure_loaded().await;
        if id.is_empty() {
            return Err(SubscriptionError::invalid("subscription id is empty"));
        }
        let config = self.config().clone();
        let input = normalize_create(&config, request)?;
        let config_hash = stable_config_hash(&input);

        if let Some(existing) = self.store.get(id) {
            if existing.config_hash != config_hash {
                return Err(SubscriptionError::new(
                    SubscriptionErrorCode::SubscriptionAlreadyExists,
                    "Subscription already exists with a different configuration",
                ));
            }
            return Ok((self.view_of(&existing).await, false));
        }

        let base_url = config
            .public_base_url
            .clone()
            .unwrap_or_else(|| request_base_url.trim_end_matches('/').to_string());
        let mut record = SubscriptionRecord {
            id: id.to_string(),
            kind: input.kind,
            pattern: input.pattern.clone(),
            webhook_url: input.webhook.as_ref().map(|webhook| webhook.url.clone()),
            wake_stream: input.wake_stream.clone(),
            lease_ttl_ms: input.lease_ttl_ms,
            description: input.description.clone(),
            created_at: self.created_at(),
            status: SubscriptionStatus::Active,
            config_hash,
            callback_base_url: base_url,
            streams: BTreeMap::new(),
            generation: 0,
            wake_id: None,
            wake_snapshot: BTreeMap::new(),
            token: None,
            token_generation: 0,
            holder: None,
            lease_expires_at_ms: None,
            retry_count: 0,
            next_attempt_at_ms: None,
        };

        for path in &input.streams {
            let tail = self.tail_of(path).await;
            record.streams.insert(
                path.clone(),
                StreamLink {
                    path: path.clone(),
                    link_types: BTreeSet::from([LinkType::Explicit]),
                    acked_offset: tail,
                },
            );
        }
        if let Some(pattern) = &record.pattern {
            for (path, tail) in self.list_streams().await {
                if !glob_match(pattern, &path) {
                    continue;
                }
                record
                    .streams
                    .entry(path.clone())
                    .or_insert_with(|| StreamLink {
                        path,
                        link_types: BTreeSet::from([LinkType::Glob]),
                        acked_offset: tail,
                    })
                    .link_types
                    .insert(LinkType::Glob);
            }
        }

        self.save(&record).await;
        Ok((self.view_of(&record).await, true))
    }

    /// Read a subscription.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionErrorCode::SubscriptionNotFound`] when the identifier is unknown.
    pub async fn read(&self, id: &str) -> Result<SubscriptionView, SubscriptionError> {
        self.ensure_loaded().await;
        let record = self
            .store
            .get(id)
            .ok_or_else(SubscriptionError::not_found)?;
        Ok(self.view_of(&record).await)
    }

    /// Delete a subscription. A missing subscription is not an error.
    pub async fn delete(&self, id: &str) {
        self.ensure_loaded().await;
        if self.store.remove(id).is_some() {
            self.attempted
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .remove(id);
            if let Err(error) = self.store.persist_tombstone(id).await {
                tracing::warn!(
                    subscription = id,
                    "subscription tombstone write failed: {error}"
                );
            }
        }
    }

    /// Add explicit streams to a subscription.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionErrorCode::SubscriptionNotFound`] when the identifier is unknown.
    pub async fn add_explicit_streams(
        &self,
        id: &str,
        streams: Vec<String>,
    ) -> Result<(), SubscriptionError> {
        self.ensure_loaded().await;
        let mut normalized = Vec::with_capacity(streams.len());
        for stream in streams {
            let path = normalize_relative_path(&stream);
            if path.is_empty() {
                return Err(SubscriptionError::invalid(
                    "streams must contain non-empty paths",
                ));
            }
            normalized.push(path);
        }
        if self.store.get(id).is_none() {
            return Err(SubscriptionError::not_found());
        }
        for path in normalized {
            let tail = self.tail_of(&path).await;
            self.store.with_record_mut(id, |slot| {
                let Some(record) = slot else {
                    return;
                };
                match record.streams.get_mut(&path) {
                    Some(link) => {
                        link.link_types.insert(LinkType::Explicit);
                    }
                    None => {
                        record.streams.insert(
                            path.clone(),
                            StreamLink {
                                path: path.clone(),
                                link_types: BTreeSet::from([LinkType::Explicit]),
                                acked_offset: tail,
                            },
                        );
                    }
                }
            });
        }
        self.persist_record(id).await;
        Ok(())
    }

    /// Remove the explicit link of a stream. A matching glob link stays.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionErrorCode::SubscriptionNotFound`] when the identifier is unknown.
    pub async fn remove_explicit_stream(
        &self,
        id: &str,
        stream: &str,
    ) -> Result<(), SubscriptionError> {
        self.ensure_loaded().await;
        let path = normalize_relative_path(stream);
        if self.store.get(id).is_none() {
            return Err(SubscriptionError::not_found());
        }
        self.store.with_record_mut(id, |slot| {
            let Some(record) = slot else {
                return;
            };
            let Some(link) = record.streams.get_mut(&path) else {
                return;
            };
            link.link_types.remove(&LinkType::Explicit);
            if link.link_types.is_empty() {
                record.streams.remove(&path);
            }
        });
        self.persist_record(id).await;
        Ok(())
    }

    /// Claim a pull-wake subscription.
    ///
    /// # Errors
    ///
    /// Returns [`SubscriptionErrorCode::AlreadyClaimed`] when another worker holds the lease,
    /// [`SubscriptionErrorCode::NoPendingWork`] when no data waits, and
    /// [`SubscriptionErrorCode::InvalidRequest`] for a webhook subscription.
    pub async fn claim(&self, id: &str, worker: &str) -> Result<ClaimResponse, SubscriptionError> {
        self.ensure_loaded().await;
        let now = self.now_ms();
        self.expire_due_leases(now).await;
        let record = self
            .store
            .get(id)
            .ok_or_else(SubscriptionError::not_found)?;
        if record.kind != SubscriptionType::PullWake {
            return Err(SubscriptionError::invalid("Subscription is not pull-wake"));
        }
        if let Some(holder) = &record.holder {
            return Err(SubscriptionError::new(
                SubscriptionErrorCode::AlreadyClaimed,
                "Another worker holds the lease",
            )
            .with_holder(holder.clone(), record.generation));
        }
        if !self.has_pending(&record).await {
            return Err(SubscriptionError::new(
                SubscriptionErrorCode::NoPendingWork,
                "Subscription has no pending work",
            ));
        }
        if record.wake_id.is_none() {
            let triggered_by = self.first_pending(&record).await;
            self.create_wake(id, &triggered_by).await;
        }
        let token = self.new_token();
        let updated = self.store.with_record_mut(id, |slot| {
            let record = slot?;
            record.holder = Some(worker.to_string());
            record.token = Some(token.clone());
            record.token_generation = record.generation;
            record.lease_expires_at_ms = Some(now.saturating_add(Self::lease_ms(record)));
            record.status = SubscriptionStatus::Active;
            Some(record.clone())
        });
        let record = updated.ok_or_else(SubscriptionError::not_found)?;
        self.save(&record).await;
        Ok(ClaimResponse {
            wake_id: record.wake_id.clone().unwrap_or_default(),
            generation: record.generation,
            token,
            streams: self.stream_infos(&record).await,
            lease_ttl_ms: record.lease_ttl_ms,
        })
    }

    /// Acknowledge a pull-wake claim. Without `done` the call extends the lease.
    ///
    /// # Errors
    ///
    /// Returns the fence or token error, or an invalid offset.
    pub async fn ack(
        &self,
        id: &str,
        token: &str,
        request: AckRequest,
    ) -> Result<AckResponse, SubscriptionError> {
        self.finish_request(id, token, request, true).await
    }

    /// Acknowledge a webhook wake.
    ///
    /// # Errors
    ///
    /// Returns the fence or token error, or an invalid offset.
    pub async fn callback(
        &self,
        id: &str,
        token: &str,
        request: AckRequest,
    ) -> Result<AckResponse, SubscriptionError> {
        self.finish_request(id, token, request, false).await
    }

    /// Release a pull-wake lease without acknowledging.
    ///
    /// # Errors
    ///
    /// Returns the fence or token error.
    pub async fn release(
        &self,
        id: &str,
        token: &str,
        request: AckRequest,
    ) -> Result<(), SubscriptionError> {
        self.ensure_loaded().await;
        let now = self.now_ms();
        self.expire_due_leases(now).await;
        let record = self
            .store
            .get(id)
            .ok_or_else(SubscriptionError::not_found)?;
        if record.kind != SubscriptionType::PullWake {
            return Err(SubscriptionError::invalid("Subscription is not pull-wake"));
        }
        validate_wake(&record, token, &request)?;
        let updated = self.store.with_record_mut(id, |slot| {
            let record = slot?;
            record.clear_wake();
            Some(record.clone())
        });
        let Some(record) = updated else {
            return Err(SubscriptionError::not_found());
        };
        Self::log_persist(id, self.store.persist(&record).await);
        if self.has_pending(&record).await {
            let triggered_by = self.first_pending(&record).await;
            self.create_wake(id, &triggered_by).await;
        }
        Ok(())
    }

    /// Expire due leases, queue due webhook retries and drain the delivery queue.
    ///
    /// A host calls this on a timer. The call uses the injected clock, so a test drives it
    /// without a real wait.
    pub async fn tick(&self) {
        let now = self.now_ms();
        self.expire_due_leases(now).await;
        self.enqueue_due_retries(now);
        self.drain_deliveries().await;
    }

    /// Run the queued webhook deliveries. The call returns the number of attempts.
    ///
    /// The call makes at most `delivery_queue_bound` attempts. A delivery that finishes a wake
    /// can queue the next wake; the same call runs it when the bound allows. The call never
    /// waits for a receiver longer than `delivery_timeout_ms` on the injected clock.
    ///
    /// After the drain the call queues every open webhook wake that a full queue refused. That
    /// sweep makes the next call, or the next [`SubscriptionEngine::tick`], deliver the wake
    /// instead of waiting for the lease to expire and a later append.
    pub async fn drain_deliveries(&self) -> usize {
        let cap = self.queue.bound();
        let mut attempts = 0;
        while attempts < cap {
            let Some(id) = self.queue.pop() else {
                break;
            };
            self.deliver(&id).await;
            attempts += 1;
        }
        self.requeue_dropped_deliveries(self.now_ms());
        attempts
    }

    /// The number of webhook deliveries that a full queue dropped.
    ///
    /// The count never resets, so a host reads it as a running total. A non-zero value with a
    /// growing trend means that `delivery_queue_bound` is too small for the burst.
    #[must_use]
    pub fn delivery_drops(&self) -> u64 {
        self.queue.drops()
    }

    /// Queue the open webhook wakes that a full queue refused.
    ///
    /// The sweep visits at most `delivery_queue_bound` records, like the drain. It skips a
    /// pull-wake subscription, a record with no open wake, a record whose next retry is still
    /// in the future, and a record whose current wake a delivery attempt already took.
    fn requeue_dropped_deliveries(&self, now: i64) {
        let cap = self.queue.bound();
        let due = self.store.with_records(|records| {
            let mut due = Vec::new();
            for record in records.values() {
                if due.len() >= cap {
                    break;
                }
                let Some(wake_id) = record.wake_id.as_deref() else {
                    continue;
                };
                if record.kind != SubscriptionType::Webhook {
                    continue;
                }
                if record.next_attempt_at_ms.is_some_and(|at| at > now) {
                    continue;
                }
                if record.status == SubscriptionStatus::Active
                    && self.wake_attempted(&record.id, wake_id)
                {
                    continue;
                }
                due.push(record.id.clone());
            }
            due
        });
        for id in due {
            self.enqueue_delivery(&id);
        }
    }

    /// True when a delivery attempt already took this wake.
    fn wake_attempted(&self, id: &str, wake_id: &str) -> bool {
        self.attempted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(id)
            .is_some_and(|attempted| attempted == wake_id)
    }

    /// Record that a delivery attempt takes this wake.
    fn mark_attempted(&self, id: &str, wake_id: &str) {
        self.attempted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(id.to_string(), wake_id.to_string());
    }

    /// Queue the webhook retries that are due at `now`.
    fn enqueue_due_retries(&self, now: i64) {
        let due = self.store.with_records(|records| {
            records
                .values()
                .filter(|record| {
                    record.status == SubscriptionStatus::Failed
                        && record.next_attempt_at_ms.is_some_and(|at| at <= now)
                })
                .map(|record| record.id.clone())
                .collect::<Vec<_>>()
        });
        for id in due {
            self.enqueue_delivery(&id);
        }
    }

    /// Put a subscription in the delivery queue.
    fn enqueue_delivery(&self, id: &str) {
        match self.queue.push(id) {
            QueuePush::Queued => {
                if let Some(notifier) = &self.notifier {
                    notifier.delivery_queued();
                }
            }
            QueuePush::Duplicate => {}
            QueuePush::Full => tracing::warn!(
                subscription = id,
                bound = self.queue.bound(),
                "webhook delivery queue is full; the subscription lease queues the delivery again"
            ),
        }
    }

    /// Handle an append to a stream. The observed virtual file system (VFS) calls this.
    pub async fn on_stream_written(&self, name: &StreamName) {
        let config = self.config().clone();
        let Some(relative) = config.relative_path(name.as_str()).map(str::to_string) else {
            return;
        };
        let now = self.now_ms();
        self.expire_due_leases(now).await;

        let candidates = self.store.with_records(|records| {
            records
                .values()
                .filter(|record| {
                    record.streams.contains_key(&relative)
                        || record
                            .pattern
                            .as_deref()
                            .is_some_and(|pattern| glob_match(pattern, &relative))
                })
                .map(|record| record.id.clone())
                .collect::<Vec<_>>()
        });
        if candidates.is_empty() {
            return;
        }

        let tail = self.tail_of(&relative).await;
        for id in candidates {
            let outcome = self.store.with_record_mut(&id, |slot| {
                let record = slot?;
                let mut changed = false;
                if record
                    .pattern
                    .as_deref()
                    .is_some_and(|pattern| glob_match(pattern, &relative))
                    && !record.streams.contains_key(&relative)
                {
                    record.streams.insert(
                        relative.clone(),
                        StreamLink {
                            path: relative.clone(),
                            link_types: BTreeSet::from([LinkType::Glob]),
                            acked_offset: BEFORE_FIRST.to_string(),
                        },
                    );
                    changed = true;
                }
                let should_wake = record.streams.get(&relative).is_some_and(|link| {
                    record.is_idle()
                        && compare_offsets(&tail, &link.acked_offset) == Ordering::Greater
                });
                Some((record.clone(), changed, should_wake))
            });
            let Some((record, changed, should_wake)) = outcome else {
                continue;
            };
            if changed || should_wake {
                Self::log_persist(&id, self.store.persist(&record).await);
            }
            if should_wake {
                self.create_wake(&id, &relative).await;
            }
        }
    }

    async fn finish_request(
        &self,
        id: &str,
        token: &str,
        request: AckRequest,
        require_pull_wake: bool,
    ) -> Result<AckResponse, SubscriptionError> {
        self.ensure_loaded().await;
        let now = self.now_ms();
        self.expire_due_leases(now).await;
        let record = self
            .store
            .get(id)
            .ok_or_else(SubscriptionError::not_found)?;
        if require_pull_wake && record.kind != SubscriptionType::PullWake {
            return Err(SubscriptionError::invalid("Subscription is not pull-wake"));
        }
        validate_wake(&record, token, &request)?;
        self.apply_acks(id, request.acks.as_deref().unwrap_or(&[]))
            .await?;
        if request.done {
            let next_wake = self.finish_wake(id).await;
            Ok(AckResponse {
                ok: true,
                next_wake,
            })
        } else {
            self.extend_lease(id, now).await;
            Ok(AckResponse {
                ok: true,
                next_wake: false,
            })
        }
    }

    async fn apply_acks(&self, id: &str, entries: &[AckEntry]) -> Result<(), SubscriptionError> {
        if entries.is_empty() {
            return Ok(());
        }
        let record = self
            .store
            .get(id)
            .ok_or_else(SubscriptionError::not_found)?;
        let mut planned = Vec::with_capacity(entries.len());
        for entry in entries {
            let Some(raw) = entry.stream_path() else {
                return Err(invalid_offset("Ack entry has no stream"));
            };
            let path = normalize_relative_path(raw);
            let Some(link) = record.streams.get(&path) else {
                return Err(invalid_offset(
                    "Ack references an unknown subscription stream",
                ));
            };
            if entry.offset == BEFORE_FIRST {
                return Err(invalid_offset("Ack offset must not be -1"));
            }
            if compare_offsets(&entry.offset, &link.acked_offset) == Ordering::Less {
                return Err(invalid_offset("Ack offset regresses the committed cursor"));
            }
            planned.push((path, entry.offset.clone()));
        }
        for (path, offset) in &planned {
            let tail = self.tail_of(path).await;
            if compare_offsets(offset, &tail) == Ordering::Greater {
                return Err(invalid_offset("Ack offset is beyond stream tail"));
            }
        }
        self.store.with_record_mut(id, |slot| {
            let Some(record) = slot else {
                return;
            };
            for (path, offset) in planned {
                if let Some(link) = record.streams.get_mut(&path) {
                    link.acked_offset = offset;
                }
            }
        });
        Ok(())
    }

    async fn finish_wake(&self, id: &str) -> bool {
        let updated = self.store.with_record_mut(id, |slot| {
            let record = slot?;
            let snapshot = record.wake_snapshot.clone();
            for (path, offset) in snapshot {
                if let Some(link) = record.streams.get_mut(&path) {
                    if compare_offsets(&offset, &link.acked_offset) == Ordering::Greater {
                        link.acked_offset = offset;
                    }
                }
            }
            record.clear_wake();
            record.status = SubscriptionStatus::Active;
            record.retry_count = 0;
            record.next_attempt_at_ms = None;
            Some(record.clone())
        });
        let Some(record) = updated else {
            return false;
        };
        Self::log_persist(id, self.store.persist(&record).await);
        if self.has_pending(&record).await {
            let triggered_by = self.first_pending(&record).await;
            // The wake path calls the delivery path, and a finished delivery can create the next
            // wake. Box this edge so the async call graph stays finite.
            Box::pin(self.create_wake(id, &triggered_by)).await;
            true
        } else {
            false
        }
    }

    async fn extend_lease(&self, id: &str, now: i64) {
        let updated = self.store.with_record_mut(id, |slot| {
            let record = slot?;
            record.lease_expires_at_ms = Some(now.saturating_add(Self::lease_ms(record)));
            Some(record.clone())
        });
        if let Some(record) = updated {
            Self::log_persist(id, self.store.persist(&record).await);
        }
    }

    async fn create_wake(&self, id: &str, triggered_by: &str) {
        let Some(record) = self.store.get(id) else {
            return;
        };
        if !record.is_idle() {
            return;
        }
        let mut tails = BTreeMap::new();
        for path in record.streams.keys() {
            tails.insert(path.clone(), self.tail_of(path).await);
        }
        let now = self.now_ms();
        let created = self.store.with_record_mut(id, |slot| {
            let record = slot?;
            if !record.is_idle() {
                return None;
            }
            record.generation = record.generation.saturating_add(1);
            record.wake_id = Some(self.new_wake_id());
            record.wake_snapshot = tails.clone();
            record.retry_count = 0;
            record.next_attempt_at_ms = None;
            match record.kind {
                SubscriptionType::Webhook => {
                    record.token = Some(self.new_token());
                    record.token_generation = record.generation;
                    record.lease_expires_at_ms = Some(now.saturating_add(Self::lease_ms(record)));
                }
                SubscriptionType::PullWake => {
                    record.token = None;
                    record.token_generation = record.generation;
                    record.lease_expires_at_ms = None;
                }
            }
            Some(record.clone())
        });
        let Some(record) = created else {
            return;
        };
        Self::log_persist(id, self.store.persist(&record).await);
        match record.kind {
            SubscriptionType::Webhook => self.enqueue_delivery(id),
            SubscriptionType::PullWake => {
                if let Some(wake_stream) = record.wake_stream.clone() {
                    self.write_wake_event(id, &wake_stream, triggered_by, record.generation)
                        .await;
                }
            }
        }
    }

    /// Deliver the current wake to the webhook receiver.
    ///
    /// The worker calls this. The call holds the wake lease over the attempt, and it bounds the
    /// attempt with the delivery timeout on the injected clock.
    async fn deliver(&self, id: &str) {
        let Some(record) = self.store.get(id) else {
            return;
        };
        let (Some(url), Some(wake_id), Some(token)) = (
            record.webhook_url.clone(),
            record.wake_id.clone(),
            record.token.clone(),
        ) else {
            return;
        };
        self.mark_attempted(id, &wake_id);
        self.hold_lease_for_attempt(id).await;
        let notification = WakeNotification {
            subscription_id: id.to_string(),
            wake_id,
            generation: record.generation,
            streams: self.stream_infos(&record).await,
            callback_url: format!(
                "{}{}/callback",
                record.callback_base_url,
                self.config().subscription_path(id)
            ),
            callback_token: token,
        };
        let body = match serde_json::to_vec(&notification) {
            Ok(body) => body,
            Err(error) => {
                tracing::error!(subscription = id, "webhook body encode failed: {error}");
                return;
            }
        };
        let signature = self.signing.sign_now(&self.ctx, &body);
        let request = WebhookRequest {
            url,
            headers: vec![
                ("content-type".to_string(), "application/json".to_string()),
                ("webhook-signature".to_string(), signature.header_value()),
            ],
            body,
        };
        match self.send_bounded(request).await {
            DeliveryOutcome::Answered(Ok(response)) if (200..300).contains(&response.status) => {
                let done =
                    serde_json::from_slice::<DoneFlag>(&response.body).is_ok_and(|flag| flag.done);
                let updated = self.store.with_record_mut(id, |slot| {
                    let record = slot?;
                    record.status = SubscriptionStatus::Active;
                    record.retry_count = 0;
                    record.next_attempt_at_ms = None;
                    Some(record.clone())
                });
                if let Some(record) = updated {
                    Self::log_persist(id, self.store.persist(&record).await);
                }
                if done {
                    self.finish_wake(id).await;
                }
            }
            DeliveryOutcome::Answered(Ok(response)) => {
                tracing::warn!(
                    subscription = id,
                    status = response.status,
                    "webhook delivery returned a failure status"
                );
                self.schedule_retry(id).await;
            }
            DeliveryOutcome::Answered(Err(error)) => {
                tracing::warn!(subscription = id, "webhook delivery failed: {error}");
                self.schedule_retry(id).await;
            }
            DeliveryOutcome::TimedOut => {
                tracing::warn!(
                    subscription = id,
                    timeout_ms = self.config().delivery_timeout_ms,
                    "webhook delivery exceeded the delivery timeout"
                );
                self.schedule_retry(id).await;
            }
        }
    }

    /// Send one webhook within the delivery timeout.
    ///
    /// The timer comes from the context ([`Ctx::timer`], spec 92). A bounded
    /// delivery no longer needs a host-installed `Env` timer and never falls
    /// back to an unbounded send (GitHub issue #1090). A `delivery_timeout_ms`
    /// of 0 still disables the engine bound on purpose.
    async fn send_bounded(&self, request: WebhookRequest) -> DeliveryOutcome {
        let timeout_ms = self.config().delivery_timeout_ms;
        if timeout_ms == 0 {
            return DeliveryOutcome::Answered(self.sender.send(request).await);
        }
        let deadline = self.ctx.clock().now() + std::time::Duration::from_millis(timeout_ms);
        let timer = self.ctx.timer().sleep_until(self.ctx.clone(), deadline);
        let send = async move { DeliveryOutcome::Answered(self.sender.send(request).await) };
        let timeout = async move {
            timer.await;
            DeliveryOutcome::TimedOut
        };
        futures_lite::future::or(send, timeout).await
    }

    /// Extend the wake lease so it cannot expire during one delivery attempt.
    async fn hold_lease_for_attempt(&self, id: &str) {
        let now = self.now_ms();
        let hold = i64::try_from(self.config().delivery_timeout_ms).unwrap_or(i64::MAX);
        let updated = self.store.with_record_mut(id, |slot| {
            let record = slot.filter(|record| record.wake_id.is_some())?;
            let until = now
                .saturating_add(Self::lease_ms(record))
                .saturating_add(hold);
            record.lease_expires_at_ms =
                Some(record.lease_expires_at_ms.unwrap_or(until).max(until));
            Some(record.clone())
        });
        if let Some(record) = updated {
            Self::log_persist(id, self.store.persist(&record).await);
        }
    }

    async fn schedule_retry(&self, id: &str) {
        let now = self.now_ms();
        let bound = self.config().delivery_retry_bound.max(1);
        let mut exhausted = false;
        let updated = self.store.with_record_mut(id, |slot| {
            let record = slot?;
            record.retry_count = record.retry_count.saturating_add(1);
            record.status = SubscriptionStatus::Failed;
            if record.retry_count >= bound {
                // The bound is reached. Stop retrying this wake and release it, so the next
                // append starts a new wake with a new budget.
                exhausted = true;
                record.clear_wake();
                record.next_attempt_at_ms = None;
            } else {
                let delay = self.retry_delay(record.retry_count);
                let next = now.saturating_add(i64::try_from(delay).unwrap_or(i64::MAX));
                record.next_attempt_at_ms = Some(next);
                // The lease must not expire between retries. Keep it over the next attempt
                // and its callback window.
                let until = next.saturating_add(Self::lease_ms(record)).saturating_add(
                    i64::try_from(self.config().delivery_timeout_ms).unwrap_or(i64::MAX),
                );
                record.lease_expires_at_ms =
                    Some(record.lease_expires_at_ms.unwrap_or(until).max(until));
            }
            Some(record.clone())
        });
        if exhausted {
            tracing::warn!(
                subscription = id,
                bound,
                "webhook delivery retry bound reached; the subscription stays failed"
            );
        }
        if let Some(record) = updated {
            Self::log_persist(id, self.store.persist(&record).await);
        }
    }

    async fn expire_due_leases(&self, now: i64) {
        let expired = self.store.with_records(|records| {
            records
                .values()
                .filter(|record| record.lease_expires_at_ms.is_some_and(|at| at <= now))
                .map(|record| record.id.clone())
                .collect::<Vec<_>>()
        });
        for id in expired {
            let updated = self.store.with_record_mut(&id, |slot| {
                let record = slot?;
                tracing::debug!(
                    subscription = id.as_str(),
                    "expiring the subscription lease"
                );
                record.clear_wake();
                Some(record.clone())
            });
            let Some(record) = updated else {
                continue;
            };
            Self::log_persist(&id, self.store.persist(&record).await);
            if self.has_pending(&record).await {
                let triggered_by = self.first_pending(&record).await;
                self.create_wake(&id, &triggered_by).await;
            }
        }
    }

    async fn write_wake_event(&self, id: &str, wake_stream: &str, stream: &str, generation: u64) {
        let absolute = self.config().absolute_path(wake_stream);
        let Ok(name) = StreamName::parse(&absolute) else {
            tracing::warn!(
                subscription = id,
                stream = absolute,
                "wake stream name is invalid"
            );
            return;
        };
        let event = WakeEvent {
            kind: "wake".to_string(),
            subscription_id: id.to_string(),
            stream: stream.to_string(),
            generation,
            ts: self.now_ms(),
        };
        let bytes = match serde_json::to_vec(&event) {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::error!(subscription = id, "wake event encode failed: {error}");
                return;
            }
        };
        if let Err(error) = self
            .store
            .stream()
            .append(&name, AppendRequest::new(Chunk::new(bytes)))
            .await
        {
            tracing::warn!(
                subscription = id,
                stream = absolute,
                "wake event write failed: {error}"
            );
        }
    }

    async fn list_streams(&self) -> Vec<(String, String)> {
        let Ok(prefix) = StreamName::parse(&self.config().stream_root) else {
            return Vec::new();
        };
        match self.store.list(&prefix).await {
            Ok(entries) => entries
                .into_iter()
                .filter_map(|entry| {
                    let relative = self
                        .config()
                        .relative_path(entry.name.as_str())
                        .map(str::to_string)?;
                    Some((relative, offset_from_len(entry.meta.len)))
                })
                .collect(),
            Err(error) => {
                tracing::warn!("stream list failed: {error}");
                Vec::new()
            }
        }
    }

    async fn stream_infos(&self, record: &SubscriptionRecord) -> Vec<StreamInfo> {
        let mut infos = Vec::with_capacity(record.streams.len());
        for link in record.streams.values() {
            let tail = self.tail_of(&link.path).await;
            infos.push(StreamInfo {
                path: link.path.clone(),
                link_type: link.link_type(),
                acked_offset: link.acked_offset.clone(),
                has_pending: compare_offsets(&tail, &link.acked_offset) == Ordering::Greater,
                tail_offset: tail,
            });
        }
        infos
    }

    async fn has_pending(&self, record: &SubscriptionRecord) -> bool {
        for link in record.streams.values() {
            let tail = self.tail_of(&link.path).await;
            if compare_offsets(&tail, &link.acked_offset) == Ordering::Greater {
                return true;
            }
        }
        false
    }

    async fn first_pending(&self, record: &SubscriptionRecord) -> String {
        for link in record.streams.values() {
            let tail = self.tail_of(&link.path).await;
            if compare_offsets(&tail, &link.acked_offset) == Ordering::Greater {
                return link.path.clone();
            }
        }
        String::new()
    }

    async fn tail_of(&self, relative: &str) -> String {
        let absolute = self.config().absolute_path(relative);
        let Ok(name) = StreamName::parse(&absolute) else {
            return zero_offset();
        };
        match self.store.stream().head(&name).await {
            Ok(head) => wire::offset_text(&head.tail),
            Err(StoreError::NotFound | StoreError::Gone) => zero_offset(),
            Err(error) => {
                tracing::warn!(stream = absolute, "stream head failed: {error}");
                zero_offset()
            }
        }
    }

    async fn view_of(&self, record: &SubscriptionRecord) -> SubscriptionView {
        let streams = self
            .stream_infos(record)
            .await
            .into_iter()
            .map(|info| StreamView {
                path: info.path,
                link_type: info.link_type,
                acked_offset: info.acked_offset,
            })
            .collect();
        let webhook = record.webhook_url.as_ref().map(|url| WebhookView {
            url: url.clone(),
            signing: WebhookSigningView {
                alg: "ed25519".to_string(),
                kid: self.signing.kid().to_string(),
                jwks_url: format!("{}{}", record.callback_base_url, self.config().jwks_path()),
            },
        });
        SubscriptionView {
            id: record.id.clone(),
            subscription_id: record.id.clone(),
            kind: record.kind,
            pattern: record.pattern.clone(),
            streams,
            webhook,
            wake_stream: record.wake_stream.clone(),
            lease_ttl_ms: record.lease_ttl_ms,
            created_at: record.created_at.clone(),
            status: record.status,
            description: record.description.clone(),
        }
    }

    async fn save(&self, record: &SubscriptionRecord) {
        if let Err(error) = self.store.save(record).await {
            tracing::warn!(
                subscription = record.id.as_str(),
                "subscription state write failed: {error}"
            );
        }
    }

    async fn persist_record(&self, id: &str) {
        if let Some(record) = self.store.get(id) {
            Self::log_persist(id, self.store.persist(&record).await);
        }
    }

    fn log_persist(id: &str, result: Result<(), String>) {
        if let Err(error) = result {
            tracing::warn!(
                subscription = id,
                "subscription state write failed: {error}"
            );
        }
    }

    async fn ensure_loaded(&self) {
        if let Err(error) = self.store.load().await {
            tracing::warn!("subscription state load failed: {error}");
        }
    }

    fn now_ms(&self) -> i64 {
        unix_millis(self.ctx.clock().wall_now())
    }

    fn created_at(&self) -> String {
        let millis = self.now_ms();
        time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(millis) * 1_000_000)
            .ok()
            .and_then(|time| {
                time.format(&time::format_description::well_known::Rfc3339)
                    .ok()
            })
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string())
    }

    fn lease_ms(record: &SubscriptionRecord) -> i64 {
        i64::try_from(record.lease_ttl_ms).unwrap_or(i64::MAX)
    }

    fn retry_delay(&self, retry_count: u32) -> u64 {
        let config = self.config();
        let exponent = retry_count.saturating_sub(1).min(16);
        let base = config
            .min_retry_delay_ms
            .saturating_mul(1_u64 << exponent)
            .min(config.max_retry_delay_ms);
        let span = base.saturating_mul(u64::from(config.retry_jitter_percent)) / 100;
        if span == 0 {
            return base;
        }
        let random = self.random_u64() % span.saturating_mul(2).saturating_add(1);
        let jitter = i64::try_from(random)
            .unwrap_or(i64::MAX)
            .saturating_sub(i64::try_from(span).unwrap_or(i64::MAX));
        u64::try_from(
            i64::try_from(base)
                .unwrap_or(i64::MAX)
                .saturating_add(jitter),
        )
        .unwrap_or(base)
        .max(config.min_retry_delay_ms)
    }

    fn new_wake_id(&self) -> String {
        let bytes = self.random_bytes(12);
        let mut out = String::with_capacity(2 + bytes.len() * 2);
        out.push_str("w_");
        for byte in bytes {
            let _ = write!(out, "{byte:02x}");
        }
        out
    }

    fn new_token(&self) -> String {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine as _;
        URL_SAFE_NO_PAD.encode(self.random_bytes(32))
    }

    fn random_u64(&self) -> u64 {
        let rng = self.ctx.rng();
        // The mutex poison type comes from the platform context mutex. The closure is portable
        // across the standard and wasm backends; the function pointer is not.
        #[expect(
            clippy::redundant_closure_for_method_calls,
            reason = "the poison type is platform specific, so the closure keeps the wasm build"
        )]
        let mut rng = rng.lock().unwrap_or_else(|error| error.into_inner());
        rng.next_u64()
    }

    fn random_bytes(&self, len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            let value = self.random_u64().to_le_bytes();
            let take = (len - out.len()).min(value.len());
            out.extend_from_slice(&value[..take]);
        }
        out
    }
}

fn validate_wake(
    record: &SubscriptionRecord,
    token: &str,
    request: &AckRequest,
) -> Result<(), SubscriptionError> {
    tracing::debug!(
        subscription = record.id.as_str(),
        token_present = record.token.is_some(),
        token_matches = record.token.as_deref() == Some(token),
        token_generation = record.token_generation,
        generation = record.generation,
        wake_id = record.wake_id.as_deref(),
        request_generation = request.generation,
        request_wake_id = request.wake_id.as_deref(),
        "validating subscription wake"
    );
    let Some(stored) = record.token.as_deref() else {
        return Err(SubscriptionError::new(
            SubscriptionErrorCode::TokenInvalid,
            "Token invalid",
        ));
    };
    if request
        .generation
        .is_some_and(|generation| generation != record.generation)
    {
        return Err(SubscriptionError::fenced());
    }
    if stored != token {
        return Err(SubscriptionError::new(
            SubscriptionErrorCode::TokenInvalid,
            "Token invalid",
        ));
    }
    if record.token_generation != record.generation
        || request.generation != Some(record.generation)
        || request.wake_id.as_deref() != record.wake_id.as_deref()
    {
        return Err(SubscriptionError::fenced());
    }
    Ok(())
}

fn invalid_offset(message: &str) -> SubscriptionError {
    SubscriptionError::new(SubscriptionErrorCode::InvalidOffset, message)
}

fn normalize_create(
    config: &SubscriptionConfig,
    request: CreateRequest,
) -> Result<SubscriptionCreate, SubscriptionError> {
    let kind = match request.kind.as_str() {
        "webhook" => SubscriptionType::Webhook,
        "pull-wake" => SubscriptionType::PullWake,
        _ => {
            return Err(SubscriptionError::invalid(
                "type must be \"webhook\" or \"pull-wake\"",
            ))
        }
    };
    let pattern = request
        .pattern
        .map(|pattern| normalize_relative_path(&pattern))
        .filter(|pattern| !pattern.is_empty());
    let mut streams = BTreeSet::new();
    for stream in request.streams {
        let path = normalize_relative_path(&stream);
        if path.is_empty() {
            return Err(SubscriptionError::invalid(
                "streams must contain non-empty paths",
            ));
        }
        streams.insert(path);
    }
    if pattern.is_none() && streams.is_empty() {
        return Err(SubscriptionError::invalid(
            "At least one of pattern or streams is required",
        ));
    }
    let lease_ttl_ms = request.lease_ttl_ms.unwrap_or(config.default_lease_ttl_ms);
    if lease_ttl_ms < config.min_lease_ttl_ms || lease_ttl_ms > config.max_lease_ttl_ms {
        return Err(SubscriptionError::invalid(format!(
            "lease_ttl_ms must be an integer from {} to {}",
            config.min_lease_ttl_ms, config.max_lease_ttl_ms
        )));
    }
    let webhook = match kind {
        SubscriptionType::Webhook => {
            let url = request
                .webhook
                .map(|webhook| webhook.url)
                .filter(|url| !url.is_empty())
                .ok_or_else(|| {
                    SubscriptionError::invalid("webhook subscriptions require webhook.url")
                })?;
            validate_webhook_url(&url).map_err(|message| {
                SubscriptionError::new(SubscriptionErrorCode::WebhookUrlRejected, message)
            })?;
            Some(WebhookCreate { url })
        }
        SubscriptionType::PullWake => None,
    };
    let wake_stream = request
        .wake_stream
        .map(|stream| normalize_relative_path(&stream))
        .filter(|stream| !stream.is_empty());
    if kind == SubscriptionType::PullWake && wake_stream.is_none() {
        return Err(SubscriptionError::invalid(
            "pull-wake subscriptions require wake_stream",
        ));
    }
    Ok(SubscriptionCreate {
        kind,
        pattern,
        streams: streams.into_iter().collect(),
        webhook,
        wake_stream,
        lease_ttl_ms,
        description: request.description,
    })
}
