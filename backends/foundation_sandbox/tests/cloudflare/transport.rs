//! The tests of the container port transport.

use std::sync::Arc;

use foundation_sandbox::shared::cloudflare::testing::transport_round_trip;
use foundation_sandbox::shared::cloudflare::transport::ContainerPortTransport;
use foundation_sandbox::shared::cloudflare::PortResponse;

use super::support::Harness;

#[test]
fn transport_refuses_request_streaming() {
    let h = Harness::new(50, true, false);
    let container: Arc<dyn foundation_sandbox::shared::cloudflare::ContainerApi> =
        h.container.clone();
    let transport = ContainerPortTransport::over_container(container, 1024);
    assert!(!transport.accepts_request_stream());
}

#[test]
fn transport_streams_response() {
    let h = Harness::new(51, true, false);
    let chunks = 10;
    let body: foundation_sandbox::shared::BoxByteStream = Box::pin(futures::stream::iter(
        (0..chunks).map(|_| Ok(vec![1_u8; 1024 * 1024])),
    ));
    h.container.set_raw_response(PortResponse {
        status: 200,
        headers: vec![(
            "content-type".to_string(),
            "application/octet-stream".to_string(),
        )],
        body,
    });
    let container: Arc<dyn foundation_sandbox::shared::cloudflare::ContainerApi> =
        h.container.clone();
    let transport = ContainerPortTransport::over_container(container, 8080);
    let (status, lengths) =
        transport_round_trip(&transport, "/large", Vec::new()).expect("the exchange works");
    assert_eq!(status, 200);
    assert!(lengths.len() >= chunks, "{lengths:?}");
    assert_eq!(lengths.iter().sum::<usize>(), chunks * 1024 * 1024);
}
