//! The `ConnectRPC` driver.
//!
//! The driver uses the generated `DurableStreamService` client against the
//! adapter service. The generated request and reply types carry the protocol
//! outcome, so the driver decodes them to the same [`Observation`] as the other
//! transports.

use std::sync::Arc;
use std::time::Duration;

use foundation_connectrpc::shared::durable::proto::v1 as pb;
use foundation_connectrpc::shared::durable::proto::v1::durable_stream_service::DurableStreamServiceClient;
use foundation_connectrpc::{
    ClientOptions, Code, ConnectError, H2Transport, Request, RpcCtx, Transport,
};
use foundation_context::Ctx;
use foundation_errstacks::ErrorTrace;

use crate::driver::{Driver, ErrorKind, Observation, Producer, StepResult};

/// The Durable Streams `ConnectRPC` client.
pub struct RpcDriver {
    client: DurableStreamServiceClient,
}

impl RpcDriver {
    /// Build the driver on a base URL.
    ///
    /// # Panics
    /// Panics when the generated client cannot use the URL.
    #[must_use]
    pub fn new(base_url: &str) -> Self {
        let transport: Arc<dyn Transport> = Arc::new(H2Transport::new());
        let client = DurableStreamServiceClient::new(
            transport,
            base_url,
            ClientOptions::new().with_connect(),
        )
        .expect("rpc client");
        Self { client }
    }

    /// A call context with a deadline that outlasts a slow worker.
    fn call_context() -> RpcCtx {
        RpcCtx::client(Ctx::host()).with_deadline(Duration::from_secs(120))
    }
}

/// Map one RPC refusal to a transport-neutral kind.
fn rpc_error(error: &ErrorTrace<ConnectError>) -> StepResult {
    let kind = match error.current_context().code() {
        Code::NotFound => ErrorKind::NotFound,
        Code::FailedPrecondition => ErrorKind::Closed,
        Code::AlreadyExists => ErrorKind::Conflict,
        Code::InvalidArgument => ErrorKind::BadRequest,
        Code::Unimplemented => ErrorKind::Unsupported,
        other => ErrorKind::Other(other.as_str().to_string()),
    };
    StepResult::new(Observation::error(kind), None)
}

/// Build the observation of a create or fork reply.
fn create_step(created: bool, head: Option<&pb::StreamHead>) -> StepResult {
    let offset = head.map(|value| value.tail.clone());
    let closed = head.is_some_and(|value| value.closed);
    let status = if created { 201 } else { 200 };
    let observation = Observation::status(status)
        .with_next_offset(offset.is_some())
        .with_closed(closed);
    StepResult::new(observation, offset)
}

/// Build the observation of an append or close reply.
fn append_step(duplicate: bool, cursor: Option<&pb::Cursor>) -> StepResult {
    let offset = cursor.map(|value| value.next.clone());
    let closed = cursor.is_some_and(|value| value.closed);
    let observation = Observation::status(204)
        .with_next_offset(offset.is_some())
        .with_closed(closed)
        .with_duplicate(duplicate);
    StepResult::new(observation, offset)
}

impl Driver for RpcDriver {
    async fn create(&mut self, stream: &str, content_type: &str) -> StepResult {
        let request = pb::CreateRequest {
            name: stream.to_string(),
            content_type: Some(content_type.to_string()),
            ..Default::default()
        };
        match self
            .client
            .create(Self::call_context(), Request::new(request))
            .await
        {
            Ok(response) => create_step(response.msg.created, response.msg.head.as_option()),
            Err(error) => rpc_error(&error),
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
        let request = pb::AppendRequest {
            name: stream.to_string(),
            chunk: data.to_vec(),
            content_type: Some(content_type.to_string()),
            stream_seq: stream_seq.map(str::to_string),
            producer: producer
                .map(|value| pb::Producer {
                    id: value.id.clone(),
                    epoch: value.epoch,
                    seq: value.seq,
                    ..Default::default()
                })
                .into(),
            close,
            ..Default::default()
        };
        match self
            .client
            .append(Self::call_context(), Request::new(request))
            .await
        {
            Ok(response) => append_step(response.msg.duplicate, response.msg.cursor.as_option()),
            Err(error) => rpc_error(&error),
        }
    }

    async fn close(&mut self, stream: &str, data: &[u8], _content_type: &str) -> StepResult {
        let request = pb::CloseRequest {
            name: stream.to_string(),
            final_chunk: (!data.is_empty()).then(|| data.to_vec()),
            ..Default::default()
        };
        match self
            .client
            .close(Self::call_context(), Request::new(request))
            .await
        {
            Ok(response) => append_step(response.msg.duplicate, response.msg.cursor.as_option()),
            Err(error) => rpc_error(&error),
        }
    }

    async fn read(&mut self, stream: &str, offset: Option<&str>) -> StepResult {
        let request = pb::ReadRequest {
            name: stream.to_string(),
            offset: offset.map(str::to_string),
            ..Default::default()
        };
        match self
            .client
            .read(Self::call_context(), Request::new(request))
            .await
        {
            Ok(response) => {
                let message = response.msg;
                let cursor = message.cursor.as_option();
                let offset = cursor.map(|value| value.next.clone());
                let body: Vec<u8> = message
                    .chunks
                    .iter()
                    .flat_map(|chunk| chunk.data.iter().copied())
                    .collect();
                let observation = Observation::status(200)
                    .with_body(body)
                    .with_next_offset(offset.is_some())
                    .with_up_to_date(cursor.is_some_and(|value| value.up_to_date))
                    .with_closed(cursor.is_some_and(|value| value.closed));
                StepResult::new(observation, offset)
            }
            Err(error) => rpc_error(&error),
        }
    }

    async fn fork(&mut self, child: &str, source: &str, at: Option<&str>) -> StepResult {
        let request = pb::ForkRequest {
            name: child.to_string(),
            source: source.to_string(),
            at: at.map(str::to_string),
            ..Default::default()
        };
        match self
            .client
            .fork(Self::call_context(), Request::new(request))
            .await
        {
            Ok(response) => create_step(response.msg.created, response.msg.head.as_option()),
            Err(error) => rpc_error(&error),
        }
    }
}
