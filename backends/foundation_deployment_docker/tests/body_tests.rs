//! Tests for the typed body streams in `client::body`, over in-memory bodies.
//!
//! WHY: The progress, log and stats streams turn a raw response body into
//! typed items, map in-band failures to the endpoint's error, and refuse to
//! buffer a corrupt frame. These checks need no daemon: the body is built in
//! memory and handed to the stream directly.
//!
//! WHAT: Moved here from the inline `#[cfg(test)]` module that used to live in
//! `src/client/body.rs` (tests live in `tests/`), now driven by
//! `#[valtron_test]`.

#![cfg(feature = "docker")]

use foundation_core::valtron::valtron_test;
use foundation_deployment::providers::common::{ByteStream, JsonLinesStream};
use foundation_deployment_docker::client::body::{LogStream, ProgressStream};
use foundation_deployment_docker::generated::create::CreateImageInfo;
use foundation_deployment_docker::generated::shared::BuildInfo;
use foundation_deployment_docker::streaming::decoder::LogOutput;
use foundation_deployment_docker::DockerError;
use foundation_deployment_docker::DockerLimits;
use foundation_netio::shared::http::SendSafeBody;

/// The default line and frame caps, which these streams run under unless a
/// test says otherwise.
fn limits() -> DockerLimits {
    DockerLimits::default()
}

fn pull(text: &str) -> ProgressStream<CreateImageInfo> {
    ProgressStream::new(
        JsonLinesStream::new(
            SendSafeBody::Bytes(text.as_bytes().to_vec()),
            limits().max_json_line,
        ),
        DockerError::PullFailed,
    )
}

#[valtron_test]
async fn progress_items_are_typed() {
    let mut s = pull(
        "{\"status\":\"Pulling\",\"id\":\"latest\",\"progressDetail\":{\"current\":1,\"total\":4}}\n",
    );
    let item = s.next().await.expect("item").expect("ok");
    assert_eq!(item.status.as_deref(), Some("Pulling"));
    assert_eq!(item.progress_detail.and_then(|p| p.total), Some(4));
    assert!(s.next().await.is_none());
}

#[valtron_test]
async fn an_in_band_error_ends_the_stream_with_the_endpoints_error() {
    let s = pull(
        "{\"status\":\"a\"}\n{\"errorDetail\":{\"message\":\"denied\"},\"error\":\"denied\"}\n{\"status\":\"never\"}\n",
    );
    let err = s.finish().await.expect_err("in-band error");
    assert!(
        matches!(&err, DockerError::PullFailed(m) if m == "denied"),
        "{err:?}"
    );
}

#[valtron_test]
async fn a_build_reports_the_image_it_built() {
    let s = ProgressStream::<BuildInfo>::new(
        JsonLinesStream::new(
            SendSafeBody::Text(
                "{\"stream\":\"Step 1/1 : FROM scratch\\n\"}\n{\"aux\":{\"ID\":\"sha256:abc\"}}\n"
                    .into(),
            ),
            limits().max_json_line,
        ),
        DockerError::BuildFailed,
    );
    assert_eq!(
        s.image_id().await.expect("built").as_deref(),
        Some("sha256:abc")
    );
}

#[valtron_test]
async fn log_frames_are_demultiplexed_and_a_truncated_frame_is_an_error() {
    let mut raw = vec![1, 0, 0, 0, 0, 0, 0, 3];
    raw.extend_from_slice(b"out");
    raw.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 3]);
    raw.extend_from_slice(b"err");
    let (mut out, mut err) = (String::new(), String::new());
    LogStream::new(
        ByteStream::new(SendSafeBody::Bytes(raw.clone())),
        limits().max_log_frame,
    )
        .read_into(&mut out, &mut err)
        .await
        .expect("frames");
    assert_eq!((out.as_str(), err.as_str()), ("out", "err"));

    raw.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 9, b'x']);
    let result = LogStream::new(
        ByteStream::new(SendSafeBody::Bytes(raw)),
        limits().max_log_frame,
    )
        .read_into(&mut out, &mut err)
        .await;
    assert!(
        matches!(result, Err(DockerError::JsonParse(_))),
        "{result:?}"
    );
}

/// A header announcing a 4 GiB frame is refused from the header alone: the
/// body here holds nothing after it, so a stream that waited to buffer the
/// payload would instead report a truncated frame.
#[valtron_test]
async fn an_oversized_frame_header_is_an_error_not_a_huge_buffer() {
    let raw = vec![1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff];
    let mut s = LogStream::new(
        ByteStream::new(SendSafeBody::Bytes(raw)),
        limits().max_log_frame,
    );
    let first = s.next().await.expect("an error");
    assert!(
        matches!(first, Err(DockerError::JsonParse(ref m)) if m.contains("larger than")),
        "{first:?}"
    );
    assert!(s.next().await.is_none(), "the stream ends after the error");
}

/// The cap is the configured one: a frame exactly at it is read, one byte
/// over it is refused.
#[valtron_test]
async fn the_configured_frame_cap_is_the_one_enforced() {
    let frame = |len: u8| {
        let mut raw = vec![1, 0, 0, 0, 0, 0, 0, len];
        raw.extend(std::iter::repeat_n(b'x', usize::from(len)));
        raw
    };
    let mut at_cap = LogStream::new(ByteStream::new(SendSafeBody::Bytes(frame(4))), 4);
    assert!(
        matches!(at_cap.next().await, Some(Ok(LogOutput::StdOut { ref message })) if message.len() == 4),
        "a frame exactly at the cap is read"
    );
    assert!(at_cap.next().await.is_none());

    let mut over = LogStream::new(ByteStream::new(SendSafeBody::Bytes(frame(5))), 4);
    let first = over.next().await.expect("an error");
    assert!(
        matches!(first, Err(DockerError::JsonParse(ref m)) if m.contains("5 bytes is larger than 4 bytes")),
        "{first:?}"
    );
}

#[test]
fn the_default_limits_are_the_documented_ones() {
    let limits = DockerLimits::default();
    assert_eq!(limits.max_error_body, 64 * 1024);
    assert_eq!(limits.max_json_line, 1024 * 1024);
    assert_eq!(limits.max_log_frame, 16 * 1024 * 1024);
    let tuned = limits
        .with_max_error_body(1)
        .with_max_json_line(2)
        .with_max_log_frame(3);
    assert_eq!(
        (tuned.max_error_body, tuned.max_json_line, tuned.max_log_frame),
        (1, 2, 3)
    );
    let body = tuned.body_limits();
    assert_eq!((body.max_error_body, body.max_json_line), (1, 2));
}

#[test]
fn a_client_hands_its_limits_to_the_generated_functions() {
    let tuned = DockerLimits::default().with_max_error_body(7).with_max_json_line(9);
    let client = foundation_deployment_docker::DockerClient::connect_unix("/nonexistent.sock")
        .with_limits(tuned);
    assert_eq!(client.limits(), tuned);
    assert_eq!(client.api().limits(), tuned.body_limits());
    assert_eq!(
        foundation_deployment_docker::DockerClient::connect_unix("/nonexistent.sock").limits(),
        DockerLimits::default()
    );
}
