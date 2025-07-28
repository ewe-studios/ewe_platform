use http_body_util::{combinators::BoxBody, BodyExt, Empty, Full};

pub(crate) fn host_addr(uri: &http::Uri) -> Option<String> {
    uri.authority().map(std::string::ToString::to_string)
}

pub fn empty() -> BoxBody<bytes::Bytes, hyper::Error> {
    Empty::<bytes::Bytes>::new()
        .map_err(|never| match never {})
        .boxed()
}

pub fn full<T: Into<bytes::Bytes>>(chunk: T) -> BoxBody<bytes::Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}
