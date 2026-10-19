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
use foundation_deployment_docker::client::body::{LogStream, ProgressStream, MAX_LOG_FRAME};
use foundation_deployment_docker::generated::create::CreateImageInfo;
use foundation_deployment_docker::generated::shared::BuildInfo;
use foundation_deployment_docker::DockerError;
use foundation_netio::shared::http::SendSafeBody;

fn pull(text: &str) -> ProgressStream<CreateImageInfo> {
    ProgressStream::new(
        JsonLinesStream::new(SendSafeBody::Bytes(text.as_bytes().to_vec())),
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
        JsonLinesStream::new(SendSafeBody::Text(
            "{\"stream\":\"Step 1/1 : FROM scratch\\n\"}\n{\"aux\":{\"ID\":\"sha256:abc\"}}\n"
                .into(),
        )),
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
    LogStream::new(ByteStream::new(SendSafeBody::Bytes(raw.clone())))
        .read_into(&mut out, &mut err)
        .await
        .expect("frames");
    assert_eq!((out.as_str(), err.as_str()), ("out", "err"));

    raw.extend_from_slice(&[1, 0, 0, 0, 0, 0, 0, 9, b'x']);
    let result = LogStream::new(ByteStream::new(SendSafeBody::Bytes(raw)))
        .read_into(&mut out, &mut err)
        .await;
    assert!(
        matches!(result, Err(DockerError::JsonParse(_))),
        "{result:?}"
    );
}

#[valtron_test]
async fn an_oversized_frame_header_is_an_error_not_a_huge_buffer() {
    let mut raw = vec![1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff];
    raw.extend(std::iter::repeat_n(b'x', MAX_LOG_FRAME + 16));
    let mut s = LogStream::new(ByteStream::new(SendSafeBody::Bytes(raw)));
    let first = s.next().await.expect("an error");
    assert!(
        matches!(first, Err(DockerError::JsonParse(ref m)) if m.contains("larger than")),
        "{first:?}"
    );
}
