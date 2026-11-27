//! The client-side producer for idempotent appends.
//!
//! A producer names one writer with a producer identifier and an epoch. It
//! assigns a sequence number to each append and sends the three
//! `Producer-Id`/`Producer-Epoch`/`Producer-Seq` headers. A retry sends the same
//! sequence again, so the server recognizes a duplicate and writes the data
//! once.
//!
//! When the server reports that a newer epoch fenced this producer, an
//! auto-claiming producer asks for the next epoch and starts again at sequence
//! zero. [`Producer::detach`] stops the producer without closing the stream.

use super::core::{AppendResult, ClientError, StreamHandle};
use crate::durable::ContentType;

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

    /// Build the producer.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidArgument`] when the producer identifier is
    /// empty or a size limit is zero. The message names the field.
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
        Ok(Producer {
            handle: self.handle,
            id: self.producer_id,
            epoch: self.epoch,
            sequence: 0,
            auto_claim: self.auto_claim,
            max_batch_bytes: self.max_batch_bytes,
            max_batch_items: self.max_batch_items,
            max_queue_bytes: self.max_queue_bytes,
            max_epoch_claims: self.max_epoch_claims,
            detached: false,
        })
    }
}

/// The items of one batch.
type BatchGroup = Vec<Vec<u8>>;

/// One writer with a producer identifier, an epoch and a sequence.
pub struct Producer {
    handle: StreamHandle,
    id: String,
    epoch: u64,
    sequence: u64,
    auto_claim: bool,
    max_batch_bytes: usize,
    max_batch_items: usize,
    max_queue_bytes: usize,
    max_epoch_claims: u32,
    detached: bool,
}

impl Producer {
    /// The producer identifier.
    #[must_use]
    pub fn producer_id(&self) -> &str {
        &self.id
    }

    /// The current epoch. It changes after an auto-claim.
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// The sequence number that the next append uses.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.sequence
    }

    /// True after [`Self::detach`].
    #[must_use]
    pub fn is_detached(&self) -> bool {
        self.detached
    }

    /// Append one message.
    ///
    /// The client retries a transient failure with the same sequence, so the
    /// server deduplicates the write. A non-retryable failure does not advance
    /// the sequence.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError`] for a refused append, an exhausted retry or a
    /// producer that is detached.
    pub async fn append(
        &mut self,
        data: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<AppendResult, ClientError> {
        self.write(data, content_type, false).await
    }

    /// Append several items in batches.
    ///
    /// The call splits `items` into groups, each within `max_batch_items` and
    /// `max_batch_bytes`. A JSON stream receives each group as one array; every
    /// other stream receives the group's bytes joined in order. Each group is
    /// one sequence number.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidArgument`] when the items exceed
    /// `max_queue_bytes`, or any error of [`Self::append`].
    pub async fn append_batch(
        &mut self,
        items: Vec<Vec<u8>>,
        content_type: Option<&str>,
    ) -> Result<Vec<AppendResult>, ClientError> {
        if self.detached {
            return Err(ClientError::Detached);
        }
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
        let groups = self.split_batches(&items);
        let mut results = Vec::with_capacity(groups.len());
        for group in groups {
            let body = if is_json {
                json_array(&group)
            } else {
                group.concat()
            };
            results.push(self.write(body, content_type, false).await?);
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
    pub async fn close(
        &mut self,
        data: Vec<u8>,
        content_type: Option<&str>,
    ) -> Result<AppendResult, ClientError> {
        self.write(data, content_type, true).await
    }

    /// Stop the producer without closing the stream.
    ///
    /// The call sends no request. A later [`Self::append`] returns
    /// [`ClientError::Detached`].
    pub fn detach(&mut self) {
        self.detached = true;
    }

    /// Send one append, claiming a new epoch when the server fences this one.
    async fn write(
        &mut self,
        data: Vec<u8>,
        content_type: Option<&str>,
        close: bool,
    ) -> Result<AppendResult, ClientError> {
        if self.detached {
            return Err(ClientError::Detached);
        }
        let mut claims = 0_u32;
        loop {
            let producer_id = self.id.clone();
            let epoch = self.epoch;
            let sequence = self.sequence;
            let outcome = self
                .handle
                .append(
                    data.clone(),
                    content_type,
                    None,
                    Some((&producer_id, epoch, sequence)),
                    close,
                )
                .await;
            match outcome {
                Ok(result) => {
                    if !close {
                        self.sequence = self.sequence.saturating_add(1);
                    }
                    return Ok(result);
                }
                Err(error) => {
                    let Some(next_epoch) = fenced_next_epoch(&error, self.epoch) else {
                        return Err(error);
                    };
                    if !self.auto_claim || claims >= self.max_epoch_claims {
                        return Err(error);
                    }
                    claims += 1;
                    self.epoch = next_epoch;
                    self.sequence = 0;
                }
            }
        }
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
