use crate::wire::simple_http::{
    client::{HttpClientError, ParsedUrl, PreparedRequest},
    RequestDescriptor, SendSafeBody, SimpleHeader, SimpleHeaders, SimpleMethod, SimpleUrl,
};

/// Resolve a `Location` header value against a base `ParsedUrl` (Uri).
///
/// Supports:
/// - Absolute URI (contains "://") -> parsed directly
/// - Absolute-path reference (starts with '/') -> resolved against base scheme/authority
/// - Relative-path reference (no leading '/') -> resolved relative to the base's path directory
///
/// # Errors
///
/// Returns `Err(HttpClientError::InvalidLocation(_))` when the resolved location
/// cannot be parsed as a valid `ParsedUrl`. Callers should inspect the returned
/// `Result` to handle parse/resolution failures.
#[must_use = "inspect the Result to handle potential errors when resolving the Location"]
pub fn resolve_location(base: &ParsedUrl, location: &str) -> Result<ParsedUrl, HttpClientError> {
    // If the location looks like an absolute URI, parse directly.
    if location.contains("://") {
        return ParsedUrl::parse(location).map_err(|e| {
            HttpClientError::InvalidLocation(format!("failed to parse absolute location: {e}"))
        });
    }

    // Helper to get authority string (host[:port])
    let authority_str = base
        .authority()
        .map(ToString::to_string)
        .unwrap_or_default();

    // If location starts with '/', treat as absolute path on same authority
    if location.starts_with('/') {
        let candidate = format!("{}://{}{}", base.scheme().as_str(), authority_str, location);
        return ParsedUrl::parse(&candidate).map_err(|e| {
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
        return ParsedUrl::parse(&candidate).map_err(|e| {
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
    ParsedUrl::parse(&joined).map_err(|e| {
        HttpClientError::InvalidLocation(format!("failed to parse resolved relative location: {e}"))
    })
}

/// Build a follow-up `RequestDescriptor` for a redirect target.
///
/// Follow-ups default to GET with no body to avoid re-sending non-repeatable bodies.
/// Sensitive headers are stripped if the host changed.
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
    new_url: ParsedUrl,
) -> Result<RequestDescriptor, HttpClientError> {
    // Clone headers and strip request-specific headers that must not be forwarded
    let mut headers = original.headers.clone();

    // add Expect header for 100-continue
    headers.insert(SimpleHeader::EXPECT, vec!["100-continue".into()]);

    // Remove content headers since follow-up is GET/no-body
    headers.remove(&SimpleHeader::CONTENT_LENGTH);
    headers.remove(&SimpleHeader::CONTENT_TYPE);

    // Strip Authorization if host differs
    let original_host = original.request_uri.host_str().unwrap_or_default();
    let new_host = new_url.host_str().unwrap_or_default();
    strip_sensitive_headers_for_redirect(&mut headers, &original_host, &new_host);

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
/// Sensitive headers are stripped if the host changed.
#[must_use]
pub fn build_followup_request_from(
    original: &PreparedRequest,
    new_url: ParsedUrl,
) -> PreparedRequest {
    // Clone headers and strip request-specific headers that must not be forwarded
    let mut headers = original.headers.clone();

    // add Expect header for 100-continue
    headers.insert(SimpleHeader::EXPECT, vec!["100-continue".into()]);

    // Remove content headers since follow-up is GET/no-body
    headers.remove(&SimpleHeader::CONTENT_LENGTH);
    headers.remove(&SimpleHeader::CONTENT_TYPE);

    // Strip Authorization if host differs
    let original_host = original.url.host_str().unwrap_or_default();
    let new_host = new_url.host_str().unwrap_or_default();
    strip_sensitive_headers_for_redirect(&mut headers, &original_host, &new_host);

    PreparedRequest {
        method: crate::wire::simple_http::SimpleMethod::GET,
        url: new_url,
        headers,
        body: SendSafeBody::None,
    }
}

/// Strip sensitive headers when following redirects across hosts.
///
/// Current implementation strips `Authorization` when the host changes.
/// Additional sensitive headers can be stripped here in the future.
pub fn strip_sensitive_headers_for_redirect(
    headers: &mut SimpleHeaders,
    original_host: &str,
    new_host: &str,
) {
    if original_host != new_host {
        headers.remove(&SimpleHeader::AUTHORIZATION);
        // Potential future additions:
        // headers.remove(&SimpleHeader::COOKIE);
    }
}
