//! Tests for the S3/R2 HTTP object store over a stub transport.
//!
//! The stub records every shaped request and returns canned responses. No
//! socket, server or real service is involved. The `minio` and `localstack`
//! tools were checked with `which` and are absent on this host.

#![cfg(feature = "vfs-object-http")]

use core::time::Duration;
use foundation_context::Ctx;
use foundation_core::valtron::{block_on_future_scoped, SimulatedClock};
use foundation_nativeapis::native::vfs::object_http::{
    md5_base64, HttpMethod, HttpObjectStore, ObjectRequest, ObjectResponse, ObjectTransport,
    S3Credentials, S3HttpConfig,
};
use foundation_nativeapis::shared::vfs::durable::{ObjectError, ObjectFuture, ObjectStore};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct StubTransport {
    requests: Arc<Mutex<Vec<ObjectRequest>>>,
    responses: Arc<Mutex<VecDeque<ObjectResponse>>>,
}

impl StubTransport {
    fn new(responses: Vec<ObjectResponse>) -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(responses.into())),
        }
    }

    fn take_requests(&self) -> Vec<ObjectRequest> {
        std::mem::take(
            &mut *self
                .requests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

impl ObjectTransport for StubTransport {
    fn send(&self, request: ObjectRequest) -> ObjectFuture<'_, ObjectResponse> {
        let stub = self.clone();
        Box::pin(async move {
            stub.requests
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .push(request);
            Ok(stub
                .responses
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .pop_front()
                .unwrap_or(ObjectResponse {
                    status: 200,
                    headers: vec![("etag".to_string(), "\"v1\"".to_string())],
                    body: Vec::new(),
                }))
        })
    }
}

fn response(status: u16, body: &str) -> ObjectResponse {
    ObjectResponse {
        status,
        headers: vec![("etag".to_string(), "\"v1\"".to_string())],
        body: body.as_bytes().to_vec(),
    }
}

fn http_store(responses: Vec<ObjectResponse>) -> (HttpObjectStore<StubTransport>, StubTransport) {
    let transport = StubTransport::new(responses);
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock);
    let config = S3HttpConfig {
        endpoint: "https://s3.example.test".to_string(),
        bucket: "durable".to_string(),
        credentials: Some(S3Credentials {
            access_key: "AKID".to_string(),
            secret_key: "secret".to_string(),
            session_token: None,
        }),
        ..S3HttpConfig::default()
    };
    (
        HttpObjectStore::new(transport.clone(), config, &ctx),
        transport,
    )
}

fn header<'a>(request: &'a ObjectRequest, name: &str) -> Option<&'a str> {
    request
        .headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

#[test]
fn http_store_shapes_s3_requests() {
    let (store, transport) = http_store(vec![
        response(200, ""),
        response(206, "range-bytes"),
        response(200, "object-body"),
        response(412, ""),
    ]);
    let version =
        block_on_future_scoped(store.put_if_none_match("durable/a/HEAD", b"head".to_vec()))
            .expect("put");
    assert_eq!(version.as_str(), "v1");
    let requests = transport.take_requests();
    assert_eq!(requests[0].method, HttpMethod::Put);
    assert_eq!(header(&requests[0], "if-none-match"), Some("*"));
    assert_eq!(
        header(&requests[0], "x-amz-content-sha256"),
        Some("9f2e6d33a3717ee826353a404ba4618d1aeeb6879ad7936bce8ed5f46814924d"),
        "SHA-256 of the body"
    );
    assert!(header(&requests[0], "authorization")
        .expect("authorization")
        .starts_with("AWS4-HMAC-SHA256 Credential=AKID/"));
    assert!(header(&requests[0], "authorization")
        .expect("authorization")
        .contains("/us-east-1/s3/aws4_request"));
    assert!(header(&requests[0], "x-amz-date").is_some());

    let bytes = block_on_future_scoped(store.get_range("durable/a/HEAD", 5, 10)).expect("range");
    assert_eq!(bytes, b"range-bytes");
    let request = &transport.take_requests()[0];
    assert_eq!(header(request, "range"), Some("bytes=5-14"));

    let body = block_on_future_scoped(store.get("durable/a/HEAD")).expect("get");
    assert_eq!(body, b"object-body");

    let absent = block_on_future_scoped(store.put_if_match(
        "durable/a/HEAD",
        b"stale".to_vec(),
        &foundation_nativeapis::shared::vfs::durable::Version("v0".to_string()),
    ));
    assert!(matches!(absent, Err(ObjectError::PreconditionFailed)));
    let requests = transport.take_requests();
    assert_eq!(
        header(requests.last().expect("stale put"), "if-match"),
        Some("v0")
    );
}

#[test]
fn http_store_parses_a_paged_listing() {
    let page_one = "<ListBucketResult><IsTruncated>true</IsTruncated>\
        <NextContinuationToken>next</NextContinuationToken>\
        <Contents><Key>durable/a/HEAD</Key><Size>12</Size>\
        <LastModified>2026-01-01T00:00:00.000Z</LastModified><ETag>\"v1\"</ETag></Contents>\
        </ListBucketResult>";
    let page_two = "<ListBucketResult><IsTruncated>false</IsTruncated>\
        <Contents><Key>durable/a/seg/1.log</Key><Size>7</Size>\
        <LastModified>2026-01-01T00:00:01.000Z</LastModified><ETag>\"v2\"</ETag></Contents>\
        </ListBucketResult>";
    let (store, transport) = http_store(vec![response(200, page_one), response(200, page_two)]);
    let listing = block_on_future_scoped(store.list("durable/")).expect("list");
    assert_eq!(listing.len(), 2);
    assert_eq!(listing[0].key, "durable/a/HEAD");
    assert_eq!(listing[0].len, 12);
    assert_eq!(listing[0].last_modified_ms, 1_767_225_600_000);
    assert_eq!(listing[1].version.as_str(), "v2");
    let requests = transport.take_requests();
    assert_eq!(requests.len(), 2);
    assert!(requests[1]
        .query
        .iter()
        .any(|(key, value)| key == "continuation-token" && value == "next"));
}

#[test]
fn md5_matches_the_known_vector() {
    assert_eq!(md5_base64(b"abc"), "kAFQmDzST7DWlj99KOF/cg==");
    assert_eq!(md5_base64(b""), "1B2M2Y8AsgTpgAmY7PhCfg==");
}

#[test]
fn http_store_checks_every_delete_result() {
    let body = "<DeleteResult>\
        <Deleted><Key>a</Key></Deleted>\
        <Error><Key>b</Key><Code>AccessDenied</Code><Message>no</Message></Error>\
        </DeleteResult>";
    let (store, transport) = http_store(vec![response(200, body)]);
    let report = block_on_future_scoped(store.delete_many(vec!["a".to_string(), "b".to_string()]))
        .expect("delete many");
    let requests = transport.take_requests();
    assert_eq!(
        header(&requests[0], "content-md5"),
        Some(md5_base64(requests[0].body.as_slice()).as_str()),
        "the batch delete carries Content-MD5"
    );
    assert_eq!(report.deleted, vec!["a".to_string()]);
    assert_eq!(report.errors, vec![("b".to_string(), "no".to_string())]);
    assert!(!report.is_complete());

    let (store, _transport) = http_store(vec![response(
        200,
        "<DeleteResult><Deleted><Key>a</Key></Deleted></DeleteResult>",
    )]);
    let report = block_on_future_scoped(store.delete_many(vec!["a".to_string(), "c".to_string()]))
        .expect("delete many");
    assert!(
        report.errors.iter().any(|(key, _)| key == "c"),
        "a key absent from the response is an error"
    );
}

#[test]
fn http_store_uses_multipart_for_parts() {
    let start = "<InitiateMultipartUploadResult><UploadId>upload-1</UploadId></InitiateMultipartUploadResult>";
    let (store, transport) = http_store(vec![
        response(200, start),
        response(200, ""),
        response(200, ""),
        response(
            200,
            "<CompleteMultipartUploadResult><ETag>\"done\"</ETag></CompleteMultipartUploadResult>",
        ),
    ]);
    block_on_future_scoped(store.put_parts(
        "durable/a/seg/0.log",
        vec![b"part-one".to_vec(), b"part-two".to_vec()],
    ))
    .expect("multipart");
    let requests = transport.take_requests();
    assert_eq!(requests.len(), 4);
    assert!(requests[0].query.iter().any(|(key, _)| key == "uploads"));
    assert_eq!(requests[1].method, HttpMethod::Put);
    assert!(requests[1]
        .query
        .iter()
        .any(|(key, value)| key == "partNumber" && value == "1"));
    assert!(requests[3].body.starts_with(b"<CompleteMultipartUpload>"));
}

#[test]
fn r2_http_settings_use_auto_and_crc32() {
    let transport = StubTransport::new(vec![response(200, "")]);
    let clock = Arc::new(SimulatedClock::new(Duration::ZERO));
    let ctx = Ctx::fixed(clock);
    let config = S3HttpConfig::r2("https://account.r2.cloudflarestorage.com", "durable")
        .with_credentials(S3Credentials {
            access_key: "AKID".to_string(),
            secret_key: "secret".to_string(),
            session_token: None,
        });
    let store = HttpObjectStore::new(transport.clone(), config, &ctx);
    block_on_future_scoped(store.put_if_none_match("durable/a/HEAD", b"abc".to_vec()))
        .expect("put");
    let request = &transport.take_requests()[0];
    assert_eq!(
        header(request, "x-amz-checksum-crc32"),
        Some("NSRBwg=="),
        "CRC32 of abc is 0x352441c2"
    );
    assert!(header(request, "authorization")
        .expect("authorization")
        .contains("/auto/s3/aws4_request"));
    assert!(
        store.capabilities().checksum_crc32,
        "the R2 settings report CRC32"
    );
}

#[test]
fn http_store_maps_not_found() {
    let (store, _transport) = http_store(vec![ObjectResponse {
        status: 404,
        headers: Vec::new(),
        body: Vec::new(),
    }]);
    let absent = block_on_future_scoped(store.get("durable/missing"));
    assert!(matches!(absent, Err(ObjectError::NotFound)));
}
