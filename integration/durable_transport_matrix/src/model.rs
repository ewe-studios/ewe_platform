//! The model oracle.
//!
//! The model is an independent, in-memory implementation of the same driver
//! operations. It decides each outcome from the protocol rules alone. The suite
//! runs one scenario on the model and on every transport, then compares the
//! results. Agreement alone is not proof: the model is the expected answer.
//!
//! The model takes its [`Ctx`] in the constructor. It reads the wall clock from
//! that bundle and never from a global. The host servers read the same clock.

use std::collections::BTreeMap;
use std::time::Duration;

use foundation_context::{Clock, Ctx, SystemTime};
use foundation_core::durable::{wire, StreamOffset, VfsOffset};
#[cfg(feature = "native-drivers")]
use foundation_core::valtron::timer;

use crate::driver::{Driver, ErrorKind, Observation, Producer, StepResult};
use crate::scenario::{run_scenario, Scenario};

/// The current wall time in UNIX milliseconds.
fn wall_ms(clock: &dyn Clock) -> i64 {
    clock
        .wall_now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

/// One stream in the model.
#[derive(Debug, Default)]
struct ModelStream {
    /// The appended bytes.
    bytes: Vec<u8>,
    /// True after a close.
    closed: bool,
    /// The last accepted sequence for each producer identifier, with its epoch.
    producers: BTreeMap<String, (u64, u64)>,
    /// The last accepted `Stream-Seq`, as its bytes.
    last_seq: Option<Vec<u8>>,
    /// The sliding time to live, in milliseconds.
    ttl_ms: Option<i64>,
    /// The last write time, in UNIX milliseconds.
    last_access_ms: i64,
}

impl ModelStream {
    /// The wire text of the tail offset.
    fn next_offset(&self) -> String {
        let position = u64::try_from(self.bytes.len()).unwrap_or(u64::MAX);
        wire::offset_text(&StreamOffset::from(VfsOffset(position)))
    }

    /// True when the sliding TTL ran out at `now`.
    fn is_expired(&self, now: i64) -> bool {
        self.ttl_ms
            .is_some_and(|ttl| now >= self.last_access_ms.saturating_add(ttl))
    }
}

/// The in-memory protocol model.
struct Model {
    /// The streams by name.
    streams: BTreeMap<String, ModelStream>,
    /// The injected context. The model reads its clock and timer.
    ctx: Ctx,
}

impl Model {
    /// Build an empty model on the injected context.
    fn new(ctx: Ctx) -> Self {
        Self {
            streams: BTreeMap::new(),
            ctx,
        }
    }

    /// The current wall time in UNIX milliseconds.
    fn now_ms(&self) -> i64 {
        wall_ms(self.ctx.clock().as_ref())
    }

    /// Remove the stream when its TTL ran out. Return true in that case.
    fn expire(&mut self, stream: &str, now: i64) -> bool {
        let expired = self
            .streams
            .get(stream)
            .is_some_and(|state| state.is_expired(now));
        if expired {
            self.streams.remove(stream);
        }
        expired
    }

    /// Move a producer forward. Return false when the append is a duplicate.
    fn accept_producer(state: &mut ModelStream, producer: &Producer) -> bool {
        if let Some((epoch, last)) = state.producers.get(&producer.id).copied() {
            if producer.epoch == epoch && producer.seq <= last {
                return false;
            }
            state
                .producers
                .insert(producer.id.clone(), (producer.epoch, producer.seq));
            return true;
        }
        if producer.seq == 0 {
            state
                .producers
                .insert(producer.id.clone(), (producer.epoch, producer.seq));
        }
        true
    }

    /// Check a `Stream-Seq`. Return false when it repeats or regresses.
    ///
    /// The protocol makes the sequence strictly increasing: `seq <= last` is a
    /// `409 StreamSeqConflict`. The reference server applies the same rule.
    fn accept_stream_seq(state: &mut ModelStream, seq: &str) -> bool {
        let value = seq.as_bytes();
        if let Some(last) = &state.last_seq {
            if value <= last.as_slice() {
                return false;
            }
        }
        state.last_seq = Some(value.to_vec());
        true
    }
}

/// Decode a wire offset to a byte position.
fn position(text: &str) -> usize {
    wire::parse_offset(text).map_or(0, |offset| {
        usize::try_from(VfsOffset::from(&offset).0).unwrap_or(usize::MAX)
    })
}

// The trait is async so the suite can call it like a transport; the model has
// no I/O and no await point.
#[allow(clippy::unused_async, clippy::unused_async_trait_impl)]
impl Driver for Model {
    async fn create(
        &mut self,
        stream: &str,
        _content_type: &str,
        ttl_ms: Option<u64>,
    ) -> StepResult {
        let now = self.now_ms();
        let status = if self.streams.contains_key(stream) {
            200
        } else {
            self.streams.insert(
                stream.to_string(),
                ModelStream {
                    ttl_ms: ttl_ms.map(|value| i64::try_from(value).unwrap_or(i64::MAX)),
                    last_access_ms: now,
                    ..ModelStream::default()
                },
            );
            201
        };
        let next = self.streams.get(stream).map(ModelStream::next_offset);
        StepResult::new(Observation::status(status).with_next_offset(true), next)
    }

    async fn append(
        &mut self,
        stream: &str,
        data: &[u8],
        _content_type: &str,
        producer: Option<&Producer>,
        stream_seq: Option<&str>,
        close: bool,
    ) -> StepResult {
        let now = self.now_ms();
        if self.expire(stream, now) {
            return StepResult::new(Observation::error(ErrorKind::NotFound), None);
        }
        let Some(state) = self.streams.get_mut(stream) else {
            return StepResult::new(Observation::error(ErrorKind::NotFound), None);
        };
        if state.closed {
            return StepResult::new(Observation::error(ErrorKind::Closed), None);
        }
        if producer.is_some_and(|value| !Self::accept_producer(state, value)) {
            return StepResult::new(
                Observation::status(204)
                    .with_next_offset(true)
                    .with_closed(state.closed)
                    .with_duplicate(true),
                Some(state.next_offset()),
            );
        }
        if stream_seq.is_some_and(|seq| !Self::accept_stream_seq(state, seq)) {
            return StepResult::new(Observation::error(ErrorKind::Conflict), None);
        }
        state.bytes.extend_from_slice(data);
        state.last_access_ms = now;
        if close {
            state.closed = true;
        }
        let observation = Observation::status(204)
            .with_next_offset(true)
            .with_closed(state.closed);
        StepResult::new(observation, Some(state.next_offset()))
    }

    async fn close(&mut self, stream: &str, data: &[u8], _content_type: &str) -> StepResult {
        let now = self.now_ms();
        if self.expire(stream, now) {
            return StepResult::new(Observation::error(ErrorKind::NotFound), None);
        }
        let Some(state) = self.streams.get_mut(stream) else {
            return StepResult::new(Observation::error(ErrorKind::NotFound), None);
        };
        if state.closed {
            let observation = Observation::status(204)
                .with_next_offset(true)
                .with_closed(true)
                .with_duplicate(true);
            return StepResult::new(observation, Some(state.next_offset()));
        }
        state.bytes.extend_from_slice(data);
        state.last_access_ms = now;
        state.closed = true;
        let observation = Observation::status(204)
            .with_next_offset(true)
            .with_closed(true);
        StepResult::new(observation, Some(state.next_offset()))
    }

    async fn read(&mut self, stream: &str, offset: Option<&str>) -> StepResult {
        let now = self.now_ms();
        if self.expire(stream, now) {
            return StepResult::new(Observation::error(ErrorKind::NotFound), None);
        }
        let Some(state) = self.streams.get_mut(stream) else {
            return StepResult::new(Observation::error(ErrorKind::NotFound), None);
        };
        state.last_access_ms = now;
        let from = offset.map_or(0, position);
        let body = state
            .bytes
            .get(from..)
            .map_or_else(Vec::new, <[u8]>::to_vec);
        let observation = Observation::status(200)
            .with_body(body)
            .with_next_offset(true)
            .with_up_to_date(true)
            .with_closed(state.closed);
        StepResult::new(observation, Some(state.next_offset()))
    }

    async fn fork(&mut self, child: &str, source: &str, at: Option<&str>) -> StepResult {
        let now = self.now_ms();
        if self.expire(source, now) {
            return StepResult::new(Observation::error(ErrorKind::NotFound), None);
        }
        let Some(parent) = self.streams.get(source) else {
            return StepResult::new(Observation::error(ErrorKind::NotFound), None);
        };
        let length = parent.bytes.len();
        let from = at.map_or(length, position).min(length);
        let bytes = parent.bytes[..from].to_vec();
        let exists = self.streams.contains_key(child);
        if !exists {
            self.streams.insert(
                child.to_string(),
                ModelStream {
                    bytes,
                    last_access_ms: now,
                    ..ModelStream::default()
                },
            );
        }
        let status = if exists { 200 } else { 201 };
        let next = self.streams.get(child).map(ModelStream::next_offset);
        StepResult::new(Observation::status(status).with_next_offset(true), next)
    }

    async fn wait(&mut self, duration: Duration) {
        // The model runs natively, where the injected timer advances its clock.
        // A wasm build compiles the same type but never runs the model: the DO
        // driver owns the wait with a JavaScript `setTimeout`.
        #[cfg(feature = "native-drivers")]
        timer::sleep_for(duration, &self.ctx).await;
        #[cfg(not(feature = "native-drivers"))]
        {
            let _ = (duration, &self.ctx);
        }
    }
}

/// Run one scenario on the model and return the expected observations.
pub async fn model_scenario(scenario: Scenario) -> Vec<Observation> {
    run_scenario(&mut Model::new(Ctx::host()), scenario).await
}
