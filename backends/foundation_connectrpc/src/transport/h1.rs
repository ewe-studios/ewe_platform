//! Native HTTP/1.1 `Transport` implementation (Decision 11 §Transport, Feature 44
//! walking skeleton).
//!
//! WHY: `Transport` byte-level client-seam contract.
//! WHAT: `open()` spawns an OS thread driving `client_req.send()` (sync), feeds
//! head+body into pipes, returns three caller-facing halves synchronously.

use std::sync::Arc;

use bytes::Bytes;
use foundation_netio::simple_http::client::{ClientRequestBuilder, SimpleHttpClient};
use foundation_netio::simple_http::client::shared::body_reader::try_collect_bytes;
use foundation_netio::simple_http::shared::{
    pushable_request_body_with_depth, HttpClientError, Proto, RequestDescriptor,
    SimpleHeaders, Status, DEFAULT_PUSHABLE_DEPTH,
};

use super::{ByteSink, ByteSource, HeadSource, Transport, TransportCapabilities, TransportError, TransportStream};

#[derive(Clone)]
pub struct H1Transport { client: Arc<SimpleHttpClient> }

impl H1Transport {
    #[must_use] pub fn new(client: SimpleHttpClient) -> Self { Self { client: Arc::new(client) } }
}

impl Transport for H1Transport {
    fn capabilities(&self) -> TransportCapabilities {
        TransportCapabilities { request_streaming: true, full_duplex: false, h2_trailers: false, http_versions: &[Proto::HTTP11], multiplexed: false }
    }

    fn open(&self, request: RequestDescriptor) -> Result<TransportStream, TransportError> {
        let method = request.method.clone();
        let headers = request.headers.clone();
        let url_str = request_url_string(&request);

        let (pushable, body_stream) = pushable_request_body_with_depth(DEFAULT_PUSHABLE_DEPTH);
        let send_body: ByteSink = pushable.into_sender();

        let client_req = self.client.request(
            ClientRequestBuilder::<foundation_netio::simple_http::client::shared::SystemDnsResolver>::new(method, &url_str)
                .map_err(http_to_transport_error)?
                .body(body_stream)
                .headers(headers),
        ).map_err(http_to_transport_error)?;

        let (head_tx, head_rx): (foundation_core::valtron::PipeSender<(Status, SimpleHeaders)>, HeadSource) =
            foundation_core::valtron::Pipe::with_depth(1);
        let (recv_tx, recv_body): (ByteSink, ByteSource) =
            foundation_core::valtron::Pipe::with_depth(DEFAULT_PUSHABLE_DEPTH);

        std::thread::spawn(move || {
            match client_req.send() {
                Ok(finalized) => {
                    let (status, headers, body, _pool, _conn) = finalized.into_parts();
                    let _ = head_tx.try_send((status, headers));
                    if let Ok(bytes) = try_collect_bytes(body) {
                        if !bytes.is_empty() { let _ = recv_tx.try_send(Bytes::from(bytes)); }
                    }
                }
                Err(_) => {}
            }
        });

        Ok(TransportStream { send_body, head: head_rx, recv_body })
    }
}

fn request_url_string(req: &RequestDescriptor) -> String {
    format!("http://{}:{}{}", req.request_uri.host_str().unwrap_or_else(|| "localhost".to_string()), req.request_uri.port_or_default(), req.request_uri.path())
}

fn http_to_transport_error(e: HttpClientError) -> TransportError { TransportError::Connect(Box::new(e)) }
