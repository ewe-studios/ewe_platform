//! The golden wire form of a multipart request body.

use foundation_deployment::providers::common::{MultipartBody, MultipartPart};

#[test]
fn multipart_encodes_named_parts() {
    let body = MultipartBody::new()
        .with_boundary("B")
        .with_part(
            MultipartPart::new("metadata", br#"{"main_module":"index.js"}"#.to_vec())
                .with_content_type("application/json"),
        )
        .with_part(
            MultipartPart::new("files", b"export default {}".to_vec())
                .with_filename("index.js")
                .with_content_type("application/javascript+module"),
        );
    let (content_type, bytes) = body.encode();
    assert_eq!(content_type, "multipart/form-data; boundary=B");
    let text = String::from_utf8(bytes).expect("the body is text");
    assert!(text.contains(
        "--B\r\nContent-Disposition: form-data; name=\"metadata\"\r\nContent-Type: application/json\r\n\r\n{\"main_module\":\"index.js\"}\r\n"
    ));
    assert!(text.contains(
        "--B\r\nContent-Disposition: form-data; name=\"files\"; filename=\"index.js\"\r\nContent-Type: application/javascript+module\r\n\r\nexport default {}\r\n"
    ));
    assert!(text.ends_with("--B--\r\n"));
}
