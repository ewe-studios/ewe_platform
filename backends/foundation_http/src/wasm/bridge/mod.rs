//! Wasm-bindgen bridge entry points for web and Cloudflare Workers.

#[cfg(feature = "wasm-bindgen-http")]
pub mod web;

#[cfg(feature = "wasm-bindgen-http")]
pub mod cf;

#[cfg(feature = "wasm-bindgen-http")]
pub use cf::{CfHttpAppGuard, CfHttpAppSingleton};

#[cfg(feature = "wasm-bindgen-http")]
pub use web::{WasmHttpAppGuard, WasmHttpAppSingleton};

/// Parse a browser or worker request URL and percent-decode its path.
///
/// WHY: `Request.url()` is percent-encoded. A handler expects the path it
/// identifies, so `%2F` must arrive as `/` (spec 88 feature 02, story S5).
/// WHAT: the parsed [`Uri`](foundation_core::url::Uri) with the decoded path and
/// the original query.
/// HOW: only the path is decoded. The scheme, authority, query and fragment
/// stay as the host sent them, so a signed URL keeps its exact form.
///
/// # Errors
///
/// Returns the message of a malformed URL, an incomplete or invalid percent
/// escape, or a path that is not UTF-8 after decoding.
#[cfg(feature = "wasm-bindgen-http")]
pub(crate) fn parse_request_uri(url: &str) -> Result<foundation_core::url::Uri, String> {
    let uri = foundation_core::url::Uri::parse(url)
        .map_err(|error| format!("failed to parse the request URL {url}: {error}"))?;
    let decoded = decode_percent_path(uri.path())?;
    Ok(uri.with_path(decoded))
}

/// Decode `%XX` escapes in a path.
///
/// A literal `+` stays `+`: it is not a space in a path.
#[cfg(feature = "wasm-bindgen-http")]
fn decode_percent_path(path: &str) -> Result<String, String> {
    if !path.as_bytes().contains(&b'%') {
        return Ok(path.to_string());
    }

    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes
                .get(index + 1)
                .ok_or_else(|| format!("incomplete percent-encoding in path {path}"))?;
            let low = *bytes
                .get(index + 2)
                .ok_or_else(|| format!("incomplete percent-encoding in path {path}"))?;
            let both = [high, low];
            let hex = std::str::from_utf8(&both)
                .map_err(|_| format!("invalid percent-encoding in path {path}"))?;
            let byte = u8::from_str_radix(hex, 16)
                .map_err(|_| format!("invalid percent-encoding %{hex} in path {path}"))?;
            decoded.push(byte);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded)
        .map_err(|error| format!("the decoded path {path} is not UTF-8: {error}"))
}
