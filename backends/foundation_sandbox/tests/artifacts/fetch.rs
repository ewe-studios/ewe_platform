//! The streamed download tests.

use foundation_sandbox::native::artifacts::{
    sha256_hex, ArtifactKind, ArtifactLimits, ArtifactProgress, ArtifactSpec, ArtifactUpdate,
};
use foundation_sandbox::shared::error::SandboxError;
use futures::StreamExt;

use super::support::{self, Reply, TestServer};

/// A file spec for `url`.
fn file_spec(url: String, hash: &str) -> ArtifactSpec {
    ArtifactSpec {
        name: "test-file".to_string(),
        url,
        sha256: hash.to_string(),
        kind: ArtifactKind::File,
    }
}

#[test]
fn sha256_hex_matches_the_known_value() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}

#[test]
fn fetch_checks_hash() {
    let body = vec![7u8; 4096];
    let payload = body.clone();
    let server = TestServer::start(move |_| Reply::ok(payload.clone()));
    let (store, directory) = support::store("fetch-hash", ArtifactLimits::default());
    let wrong = "0".repeat(64);
    let error = support::block_on(store.ensure(&file_spec(server.url("/f"), &wrong)))
        .expect_err("the wrong hash fails");
    assert!(
        matches!(
            error.current_context(),
            SandboxError::ArtifactHashMismatch { .. }
        ),
        "unexpected error: {error}"
    );
    let found = support::entries(&directory.path().join("cache"));
    assert!(
        found.iter().all(|path| !path.contains(".tmp-")),
        "a temporary entry stays: {found:?}"
    );
    assert!(
        found
            .iter()
            .all(|path| std::path::Path::new(path).file_name()
                != Some(std::ffi::OsStr::new(".ready"))),
        "a ready marker stays: {found:?}"
    );
    let good = sha256_hex(&body);
    let path = support::block_on(store.ensure(&file_spec(server.url("/f"), &good)))
        .expect("the correct hash passes");
    assert_eq!(std::fs::read(path).expect("the file is there"), body);
}

#[test]
fn fetch_stops_at_limit() {
    let server = TestServer::start(|_| Reply::repeat(1024 * 1024, 9));
    let limits = ArtifactLimits::default().with_max_download_bytes(1024);
    let (store, _directory) = support::store("fetch-limit", limits);
    let hash = "1".repeat(64);
    let error = support::block_on(store.ensure(&file_spec(server.url("/big"), &hash)))
        .expect_err("the body is too large");
    let SandboxError::ArtifactTooLarge { limit, message } = error.current_context() else {
        panic!("the error is not ArtifactTooLarge: {error}");
    };
    assert_eq!(*limit, 1024);
    let done = message
        .rsplit(' ')
        .find_map(|word| word.parse::<u64>().ok())
        .unwrap_or(u64::MAX);
    assert!(
        done < 64 * 1024,
        "the client read {done} bytes, more than 64 KiB"
    );
}

#[test]
fn fetch_reads_bounded_error() {
    let server = TestServer::start(|_| Reply {
        status: 500,
        reason: "Internal Server Error",
        body: support::ReplyBody::Repeat {
            total: 10 * 1024 * 1024,
            chunk: vec![b'x'; 64 * 1024],
        },
        declared: None,
    });
    let limit = 4096;
    let limits = ArtifactLimits::default().with_max_error_message(limit);
    let (store, _directory) = support::store("fetch-error", limits);
    let hash = "2".repeat(64);
    let error = support::block_on(store.ensure(&file_spec(server.url("/error"), &hash)))
        .expect_err("the status is not 200");
    let SandboxError::Io { message, .. } = error.current_context() else {
        panic!("the error is not Io: {error}");
    };
    let body = message
        .strip_prefix("status 500: ")
        .expect("the message names the status");
    assert!(
        body.len() <= limit,
        "the error body is {} bytes, more than {limit}",
        body.len()
    );
}

#[test]
fn fetch_streams() {
    let total = 64u64 * 1024 * 1024;
    let server = TestServer::start(move |_| Reply::repeat(total, 3));
    let limits = ArtifactLimits::default();
    let (store, _directory, recorder) = support::recording_store("fetch-streams", limits);
    let hash = "3".repeat(64);
    // The test does not know the hash before the body. It reads the body once
    // to learn it, then expects that hash.
    let first = support::block_on(store.ensure(&file_spec(server.url("/s"), &hash)));
    assert!(first.is_err(), "a wrong hash fails after the stream");
    // The temporary file is removed, so the counters are from that attempt.
    assert_eq!(recorder.written(), total);
    assert!(
        recorder.max_write() <= 1024 * 1024,
        "one write held {} bytes",
        recorder.max_write()
    );
}

#[test]
fn ensure_with_progress_names_each_stage() {
    let body = vec![5u8; 2048];
    let payload = body.clone();
    let server = TestServer::start(move |_| Reply::ok(payload.clone()));
    let (store, _directory) = support::store("progress", ArtifactLimits::default());
    let hash = sha256_hex(&body);
    let mut stream = store.ensure_with_progress(file_spec(server.url("/p"), &hash));
    let mut updates = Vec::new();
    while let Some(item) = support::block_on(stream.next()) {
        updates.push(item.expect("the build does not fail"));
    }
    assert!(matches!(
        updates.first(),
        Some(ArtifactUpdate::Progress(ArtifactProgress::Downloading {
            done: 0,
            ..
        }))
    ));
    assert!(matches!(
        updates.get(1),
        Some(ArtifactUpdate::Progress(ArtifactProgress::Downloading { done, .. })) if *done == 2048
    ));
    assert_eq!(
        updates.get(2),
        Some(&ArtifactUpdate::Progress(ArtifactProgress::Verifying))
    );
    assert!(matches!(updates.last(), Some(ArtifactUpdate::Ready(_))));
}
