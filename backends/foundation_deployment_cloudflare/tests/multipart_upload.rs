//! The golden request of a Worker module upload (#910).
//!
//! The generated call sends a `multipart/form-data` body. This test reads the
//! request from a local listening socket and checks that it carries the module
//! part and the metadata part.

use std::io::{Read, Write};
use std::net::TcpListener;

use foundation_cloudflare_workers::workers::{
    worker_script_upload_worker_module_request, WorkerScriptUploadWorkerModuleArgs,
};
use foundation_core::valtron::{block_on_future, initialize_pool};
use foundation_deployment::providers::common::{MultipartBody, MultipartPart};
use foundation_netio::{HttpClientBuilder, PreparedRequestBuilder};

/// Reads one HTTP request: its head and the body of its `Content-Length`.
fn read_request(stream: &mut std::net::TcpStream) -> std::io::Result<Vec<u8>> {
    let mut data = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            return Ok(data);
        }
        data.extend_from_slice(&buffer[..read]);
        let Some(head_end) = find(&data, b"\r\n\r\n") else {
            continue;
        };
        let head = String::from_utf8_lossy(&data[..head_end + 4]).to_lowercase();
        let length = head
            .lines()
            .find_map(|line| line.strip_prefix("content-length:"))
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(0);
        if data.len() >= head_end + 4 + length {
            return Ok(data);
        }
    }
}

/// The first position of `needle` in `haystack`.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[test]
fn worker_upload_carries_the_module_part() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("the test socket");
    let address = listener.local_addr().expect("the test address");
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("the test connection");
        let request = read_request(&mut stream).expect("the request reads");
        stream
            .write_all(
                b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\n\r\n{}",
            )
            .expect("the response writes");
        request
    });

    let _pool = initialize_pool(13, Some(3));
    block_on_future(async move {
        let client = HttpClientBuilder::new().build();
        let args = WorkerScriptUploadWorkerModuleArgs {
            account_id: "account".to_string(),
            script_name: "ewe-sandbox".to_string(),
            bindings_inherit: None,
        };
        let module = b"export default { async fetch() {} };".to_vec();
        let body = MultipartBody::new()
            .with_part(
                MultipartPart::new("metadata", br#"{"main_module":"index.js"}"#.to_vec())
                    .with_content_type("application/json"),
            )
            .with_part(
                MultipartPart::new("files", module.clone())
                    .with_filename("index.js")
                    .with_content_type("application/javascript+module"),
            );
        // The response body is `{}`, so the typed decode fails after the send;
        // the request is what this test checks.
        let _ = worker_script_upload_worker_module_request(
            client,
            &args,
            &format!("http://{address}"),
            &body,
            None::<fn(&mut PreparedRequestBuilder)>,
        )
        .await;
    });

    let request = String::from_utf8_lossy(&server.join().expect("the server thread")).to_string();
    assert!(request.starts_with("PUT /accounts/account/workers/scripts/ewe-sandbox "));
    assert!(
        request
            .to_lowercase()
            .contains("content-type: multipart/form-data; boundary="),
        "the request is multipart: {request}"
    );
    assert!(request.contains("name=\"metadata\""));
    assert!(request.contains(r#"{"main_module":"index.js"}"#));
    assert!(request.contains("name=\"files\"; filename=\"index.js\""));
    assert!(request.contains("application/javascript+module"));
    assert!(request.contains("export default { async fetch() {} };"));
}
