//! The HTTP driver.
//!
//! The driver starts no server. It sends the same business operations over the
//! client core and the native `NetioTransport` to the in-process server.

use foundation_core::durable::client::{ClientError, DurableClient, StreamHandle};
use foundation_core::durable::StreamName;

use crate::driver::{Driver, ErrorKind, Observation, Producer, StepResult};

/// The Durable Streams client over HTTP.
pub struct HttpDriver {
    client: DurableClient,
}

impl HttpDriver {
    /// Build the driver on a client.
    #[must_use]
    pub fn new(client: DurableClient) -> Self {
        Self { client }
    }

    /// The handle for one scenario stream.
    fn handle(&self, stream: &str) -> StreamHandle {
        let name = StreamName::parse(stream).expect("a valid scenario stream name");
        self.client.stream(&name)
    }
}

/// Read the closure header of a write result.
///
/// The transport keeps the header name as the server sends it, so the lookup
/// ignores case.
fn closed_header(headers: &std::collections::BTreeMap<String, String>) -> bool {
    headers.iter().any(|(name, value)| {
        name.eq_ignore_ascii_case("stream-closed") && value.eq_ignore_ascii_case("true")
    })
}

/// Build the observation of a create or fork.
fn create_observation(status: u16, offset: Option<&String>, closed: bool) -> Observation {
    Observation::status(if status == 201 { 201 } else { 200 })
        .with_next_offset(offset.is_some())
        .with_closed(closed)
}

/// Build the observation of a refused step.
fn error_step(error: ClientError) -> StepResult {
    let kind = match error {
        ClientError::NotFound { .. } => ErrorKind::NotFound,
        ClientError::Gone { .. } => ErrorKind::Gone,
        ClientError::Closed => ErrorKind::Closed,
        ClientError::Conflict(_) => ErrorKind::Conflict,
        ClientError::InvalidOffset(_) => ErrorKind::BadRequest,
        other => ErrorKind::Other(other.to_string()),
    };
    StepResult::new(Observation::error(kind), None)
}

impl Driver for HttpDriver {
    async fn create(&mut self, stream: &str, content_type: &str) -> StepResult {
        match self
            .handle(stream)
            .create(Some(content_type), None, None, false, Vec::new())
            .await
        {
            Ok(result) => {
                let observation = create_observation(
                    result.status,
                    result.offset.as_ref(),
                    closed_header(&result.headers),
                );
                StepResult::new(observation, result.offset)
            }
            Err(error) => error_step(error),
        }
    }

    async fn append(
        &mut self,
        stream: &str,
        data: &[u8],
        content_type: &str,
        producer: Option<&Producer>,
        stream_seq: Option<&str>,
        close: bool,
    ) -> StepResult {
        let producer = producer.map(|value| (value.id.as_str(), value.epoch, value.seq));
        match self
            .handle(stream)
            .append(
                data.to_vec(),
                Some(content_type),
                stream_seq,
                producer,
                close,
            )
            .await
        {
            Ok(result) => {
                let observation = Observation::status(204)
                    .with_next_offset(result.offset.is_some())
                    .with_closed(closed_header(&result.headers))
                    .with_duplicate(result.duplicate);
                StepResult::new(observation, result.offset)
            }
            Err(error) => error_step(error),
        }
    }

    async fn close(&mut self, stream: &str, data: &[u8], content_type: &str) -> StepResult {
        match self
            .handle(stream)
            .close(data.to_vec(), Some(content_type))
            .await
        {
            Ok(result) => {
                let observation = Observation::status(204)
                    .with_next_offset(result.offset.is_some())
                    .with_closed(closed_header(&result.headers))
                    .with_duplicate(result.duplicate);
                StepResult::new(observation, result.offset)
            }
            Err(error) => error_step(error),
        }
    }

    async fn read(&mut self, stream: &str, offset: Option<&str>) -> StepResult {
        let mut builder = self.handle(stream).read();
        if let Some(offset) = offset {
            builder = builder.offset(offset);
        }
        match builder.send().await {
            Ok(result) => {
                let body: Vec<u8> = result
                    .chunks
                    .iter()
                    .flat_map(|chunk| chunk.data.iter().copied())
                    .collect();
                let observation = Observation::status(200)
                    .with_body(body)
                    .with_next_offset(result.next_offset.is_some())
                    .with_up_to_date(result.up_to_date)
                    .with_closed(result.closed);
                StepResult::new(observation, result.next_offset)
            }
            Err(error) => error_step(error),
        }
    }

    async fn fork(&mut self, child: &str, source: &str, at: Option<&str>) -> StepResult {
        let mut headers = vec![("stream-forked-from".to_string(), source.to_string())];
        if let Some(at) = at {
            headers.push(("stream-fork-offset".to_string(), at.to_string()));
        }
        match self
            .handle(child)
            .with_headers(headers)
            .create(None, None, None, false, Vec::new())
            .await
        {
            Ok(result) => {
                let observation = create_observation(
                    result.status,
                    result.offset.as_ref(),
                    closed_header(&result.headers),
                );
                StepResult::new(observation, result.offset)
            }
            Err(error) => error_step(error),
        }
    }
}
