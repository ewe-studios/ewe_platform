//! The model oracle.
//!
//! The model is an independent, in-memory implementation of the same driver
//! operations. It decides each outcome from the protocol rules alone. The suite
//! runs one scenario on the model and on every transport, then compares the
//! results. Agreement alone is not proof: the model is the expected answer.

use std::collections::BTreeMap;

use foundation_core::durable::{wire, StreamOffset, VfsOffset};

use crate::driver::{Driver, ErrorKind, Observation, Producer, StepResult};
use crate::scenario::{run_scenario, Scenario};

/// One stream in the model.
#[derive(Debug, Default)]
struct ModelStream {
    /// The appended bytes.
    bytes: Vec<u8>,
    /// True after a close.
    closed: bool,
    /// The last accepted sequence for each producer identifier, with its epoch.
    producers: BTreeMap<String, (u64, u64)>,
}

impl ModelStream {
    /// The wire text of the tail offset.
    fn next_offset(&self) -> String {
        let position = u64::try_from(self.bytes.len()).unwrap_or(u64::MAX);
        wire::offset_text(&StreamOffset::from(VfsOffset(position)))
    }
}

/// The in-memory protocol model.
#[derive(Debug, Default)]
struct Model {
    /// The streams by name.
    streams: BTreeMap<String, ModelStream>,
}

impl Model {
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
    async fn create(&mut self, stream: &str, _content_type: &str) -> StepResult {
        let status = if self.streams.contains_key(stream) {
            200
        } else {
            self.streams
                .insert(stream.to_string(), ModelStream::default());
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
        _stream_seq: Option<&str>,
        close: bool,
    ) -> StepResult {
        let Some(state) = self.streams.get_mut(stream) else {
            return StepResult::new(Observation::error(ErrorKind::NotFound), None);
        };
        if state.closed {
            return StepResult::new(Observation::error(ErrorKind::Closed), None);
        }
        let duplicate = producer.is_some_and(|value| !Self::accept_producer(state, value));
        if !duplicate {
            state.bytes.extend_from_slice(data);
        }
        if close {
            state.closed = true;
        }
        let observation = Observation::status(204)
            .with_next_offset(true)
            .with_closed(state.closed)
            .with_duplicate(duplicate);
        StepResult::new(observation, Some(state.next_offset()))
    }

    async fn close(&mut self, stream: &str, data: &[u8], _content_type: &str) -> StepResult {
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
        state.closed = true;
        let observation = Observation::status(204)
            .with_next_offset(true)
            .with_closed(true);
        StepResult::new(observation, Some(state.next_offset()))
    }

    async fn read(&mut self, stream: &str, offset: Option<&str>) -> StepResult {
        let Some(state) = self.streams.get(stream) else {
            return StepResult::new(Observation::error(ErrorKind::NotFound), None);
        };
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
                    ..ModelStream::default()
                },
            );
        }
        let status = if exists { 200 } else { 201 };
        let next = self.streams.get(child).map(ModelStream::next_offset);
        StepResult::new(Observation::status(status).with_next_offset(true), next)
    }
}

/// Run one scenario on the model and return the expected observations.
pub async fn model_scenario(scenario: Scenario) -> Vec<Observation> {
    run_scenario(&mut Model::default(), scenario).await
}
