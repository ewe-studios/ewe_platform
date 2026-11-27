//! The native webhook sender.
//!
//! It calls the platform Hypertext Transfer Protocol (HTTP) client. The subscription core stays
//! transport-neutral and sees only [`WebhookSender`].

use crate::shared::config::SubscriptionConfig;
use crate::shared::webhook::{WebhookFuture, WebhookRequest, WebhookResponse, WebhookSender};
use foundation_netio::shared::client::body_reader::collect_bytes_bounded;
use foundation_netio::shared::client::request_builder::PreparedRequestBuilder;
use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::{DynNetClient, HttpClientBuilder};
use std::time::Duration;

/// A webhook sender over the platform HTTP client.
pub struct NetioWebhookSender {
    client: DynNetClient,
    max_response_bytes: usize,
}

impl NetioWebhookSender {
    /// Build a sender with the timeouts of `config`.
    #[must_use]
    pub fn new(config: &SubscriptionConfig) -> Self {
        let client = HttpClientBuilder::new()
            .connect_timeout(Duration::from_millis(config.webhook_connect_timeout_ms))
            .read_timeout(Duration::from_millis(config.webhook_read_timeout_ms))
            .build();
        Self {
            client,
            max_response_bytes: config.max_webhook_response_bytes,
        }
    }

    /// Build a sender over an existing client.
    #[must_use]
    pub fn from_client(client: DynNetClient, max_response_bytes: usize) -> Self {
        Self {
            client,
            max_response_bytes,
        }
    }
}

/// Map a header name to its typed form.
fn known_header(name: &str) -> SimpleHeader {
    if name.eq_ignore_ascii_case("content-type") {
        SimpleHeader::CONTENT_TYPE
    } else if name.eq_ignore_ascii_case("content-length") {
        SimpleHeader::CONTENT_LENGTH
    } else if name.eq_ignore_ascii_case("authorization") {
        SimpleHeader::AUTHORIZATION
    } else {
        SimpleHeader::custom(name)
    }
}

impl WebhookSender for NetioWebhookSender {
    fn send(&self, request: WebhookRequest) -> WebhookFuture<'_> {
        let client = self.client.clone();
        let max_response_bytes = self.max_response_bytes;
        Box::pin(async move {
            let mut builder =
                PreparedRequestBuilder::post(&request.url).map_err(|error| error.to_string())?;
            for (name, value) in request.headers {
                builder = builder.header(known_header(&name), value);
            }
            let builder = builder.body_bytes(request.body);
            let response = client
                .send_async(builder.build())
                .await
                .map_err(|error| error.to_string())?;
            let (status, _, body) = response.into_async_parts();
            let status: usize = status.into();
            let status = u16::try_from(status).unwrap_or(0);
            let body = collect_bytes_bounded(body, max_response_bytes).await;
            Ok(WebhookResponse { status, body })
        })
    }
}
