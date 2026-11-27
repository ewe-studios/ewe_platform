use super::request::PreparedRequest;
use crate::shared::http::Extensions;
use crate::shared::http::{
    HttpClientError, RequestDescriptor, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod,
    SimpleUrl,
};
use foundation_core::url::Uri;

/// Resolve a `Location` header value against a base `Uri` (Uri).
///
/// Supports:
/// - Absolute URI (contains "://") -> parsed directly
/// - Absolute-path reference (starts with '/') -> resolved against base scheme/authority
/// - Relative-path reference (no leading '/') -> resolved relative to the base's path directory
///
/// # Errors
///
/// Returns `Err(HttpClientError::InvalidLocation(_))` when the resolved location
/// cannot be parsed as a valid `Uri`. Callers should inspect the returned
/// `Result` to handle parse/resolution failures.
#[must_use = "inspect the Result to handle potential errors when resolving the Location"]
pub fn resolve_location(base: &Uri, location: &str) -> Result<Uri, HttpClientError> {
    // If the location looks like an absolute URI, parse directly.
    if location.contains("://") {
        return Uri::parse(location).map_err(|e| {
            HttpClientError::InvalidLocation(format!("failed to parse absolute location: {e}"))
        });
    }

    // Helper to get authority string (host[:port])
    let authority_str = base.authority().map_or_default(ToString::to_string);

    // If location starts with '/', treat as absolute path on same authority
    if location.starts_with('/') {
        let candidate = format!("{}://{}{}", base.scheme().as_str(), authority_str, location);
        return Uri::parse(&candidate).map_err(|e| {
            HttpClientError::InvalidLocation(format!("failed to parse resolved location: {e}"))
        });
    }

    // If location is a query starting with '?', append to base's path
    if location.starts_with('?') {
        let candidate = format!(
            "{}://{}{}{}",
            base.scheme().as_str(),
            authority_str,
            base.path(),
            location
        );
        return Uri::parse(&candidate).map_err(|e| {
            HttpClientError::InvalidLocation(format!("failed to parse resolved location: {e}"))
        });
    }

    // Otherwise treat as relative path: join with base path's directory
    let base_path = base.path();
    let parent = if base_path.ends_with('/') {
        base_path.to_string()
    } else {
        match base_path.rfind('/') {
            Some(pos) => base_path[..=pos].to_string(), // include trailing '/'
            None => "/".to_string(),
        }
    };

    let joined = format!(
        "{}://{}{}{}",
        base.scheme().as_str(),
        authority_str,
        parent,
        location
    );
    Uri::parse(&joined).map_err(|e| {
        HttpClientError::InvalidLocation(format!("failed to parse resolved relative location: {e}"))
    })
}

/// Build a follow-up `RequestDescriptor` for a redirect target.
///
/// Follow-ups default to GET with no body to avoid re-sending non-repeatable bodies.
/// Sensitive headers are stripped if the host changed (unless preserve flags are true).
///
/// # Arguments
///
/// * `original` - Original request descriptor
/// * `new_url` - Redirect target URL
/// * `preserve_auth` - If true, Authorization header is preserved even on cross-host redirects
/// * `preserve_cookies` - If true, Cookie header is preserved even on cross-host redirects
///
/// # Errors
///
/// This function currently never fails and always returns `Ok(RequestDescriptor)`.
/// The `Result` return type is retained for parity with other APIs and to allow
/// future fallible transformations (for example, additional validation of the
/// resolved URL). Callers should inspect the returned `Result` to handle any
/// potential future errors.
#[must_use = "inspect the Result to handle potential future errors when resolving the redirect target"]
pub fn build_followup_request_from_request_descriptor(
    original: &RequestDescriptor,
    new_url: Uri,
    preserve_auth: bool,
    preserve_cookies: bool,
) -> Result<RequestDescriptor, HttpClientError> {
    // Clone headers and strip request-specific headers that must not be forwarded
    let mut headers = original.headers.clone();

    // The follow-up request is a GET with no body, so it must NOT carry an
    // `Expect: 100-continue` header. Sending it on a bodyless GET causes some
    // CDNs (e.g. HuggingFace's xet bridge) to emit an interim `100 Continue`
    // that the response reader mistakes for the final response, aborting the
    // download. Drop any inherited Expect header here.
    headers.remove(&SimpleHeader::EXPECT);

    // Remove content headers since follow-up is GET/no-body
    headers.remove(&SimpleHeader::CONTENT_LENGTH);
    headers.remove(&SimpleHeader::CONTENT_TYPE);

    // Rewrite the Host header to the redirect target. The cloned headers still
    // reference the original host; leaving it stale sends the wrong Host to the
    // new origin (e.g. `Host: huggingface.co` to `cas-bridge.xethub.hf.co`).
    set_host_header(&mut headers, &new_url);

    // Strip sensitive headers if the origin differs (unless preserve flags are
    // true). The origin is the scheme, the host and the port together: another
    // port on the same host is another origin.
    let original_origin = origin_key(&original.request_uri);
    let new_origin = origin_key(&new_url);
    strip_sensitive_headers_for_redirect(
        &mut headers,
        &original_origin,
        &new_origin,
        preserve_auth,
        preserve_cookies,
    );

    Ok(RequestDescriptor {
        request_url: SimpleUrl::url_with_query(new_url.to_string()),
        method: SimpleMethod::GET,
        proto: original.proto.clone(),
        request_uri: new_url,
        headers,
    })
}

/// Build a follow-up `PreparedRequest` for a redirect target.
///
/// Follow-ups default to GET with no body to avoid re-sending non-repeatable bodies.
/// Sensitive headers are stripped if the host changed (unless preserve flags are true).
///
/// # Arguments
///
/// * `original` - Original prepared request
/// * `new_url` - Redirect target URL
/// * `preserve_auth` - If true, Authorization header is preserved even on cross-host redirects
/// * `preserve_cookies` - If true, Cookie header is preserved even on cross-host redirects
#[must_use]
pub fn build_followup_request_from(
    original: &PreparedRequest,
    new_url: Uri,
    preserve_auth: bool,
    preserve_cookies: bool,
) -> PreparedRequest {
    // Clone headers and strip request-specific headers that must not be forwarded
    let mut headers = original.headers.clone();

    // Follow-up is a GET with no body — never carry `Expect: 100-continue`.
    // See `build_followup_request_from_request_descriptor` for why.
    headers.remove(&SimpleHeader::EXPECT);

    // Remove content headers since follow-up is GET/no-body
    headers.remove(&SimpleHeader::CONTENT_LENGTH);
    headers.remove(&SimpleHeader::CONTENT_TYPE);

    // Rewrite the Host header to the redirect target.
    set_host_header(&mut headers, &new_url);

    // Strip sensitive headers if the origin differs (unless preserve flags are
    // true). The origin is the scheme, the host and the port together.
    let original_origin = origin_key(&original.url);
    let new_origin = origin_key(&new_url);
    strip_sensitive_headers_for_redirect(
        &mut headers,
        &original_origin,
        &new_origin,
        preserve_auth,
        preserve_cookies,
    );

    PreparedRequest {
        method: crate::shared::http::SimpleMethod::GET,
        url: new_url,
        headers,
        body: SendSafeBody::None,
        extensions: Extensions::new(),
        body_readiness: None,
    }
}

/// The origin of one URL: its scheme, its host and its port.
///
/// The port is the default port of the scheme when the URL names none, so
/// `https://example.com/a` and `https://example.com:443/b` have one origin.
/// The host is compared without case, as the domain name rules say.
#[must_use]
pub fn origin_key(uri: &Uri) -> String {
    let scheme = uri.scheme().as_str().to_ascii_lowercase();
    let host = uri.host_str().unwrap_or_default().to_ascii_lowercase();
    format!("{scheme}://{host}:{}", uri.port_or_default())
}

/// Set (or overwrite) the `Host` header to match the given URL.
///
/// Includes the port only when the URL specifies a non-default one, mirroring
/// the format used when the request is first built. If the URL has no host the
/// header is left untouched.
pub fn set_host_header(headers: &mut SimpleHeaders, url: &Uri) {
    if let Some(host_str) = url.host_str() {
        let host = if url.port().is_some() {
            format!("{host_str}:{}", url.port_or_default())
        } else {
            host_str
        };
        headers.insert(SimpleHeader::HOST, vec![host]);
    }
}

/// Strip sensitive headers when following redirects across origins.
///
/// `Authorization` and `Cookie` are stripped when the origin changes, unless
/// the matching preserve flag is set. The origin is the scheme, the host and
/// the port together, so a redirect to another port on the same host strips
/// the headers too.
///
/// # Arguments
///
/// * `headers` - Headers to modify
/// * `original_origin` - The origin of the original request, from [`origin_key`]
/// * `new_origin` - The origin of the redirect target, from [`origin_key`]
/// * `preserve_auth` - If true, Authorization header is preserved on a change
/// * `preserve_cookies` - If true, Cookie header is preserved on a change
pub fn strip_sensitive_headers_for_redirect(
    headers: &mut SimpleHeaders,
    original_origin: &str,
    new_origin: &str,
    preserve_auth: bool,
    preserve_cookies: bool,
) {
    if original_origin != new_origin {
        if !preserve_auth {
            headers.remove(&SimpleHeader::AUTHORIZATION);
        }
        if !preserve_cookies {
            headers.remove(&SimpleHeader::COOKIE);
        }
    }
}
