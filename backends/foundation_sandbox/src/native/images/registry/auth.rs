//! The authentication of the registry (feature 15, section 6.3).
//!
//! The registry asks for a bearer token with a `WWW-Authenticate` header. This
//! module parses that header and builds the `Basic` header of the token
//! request. The client sends the token request and keeps the token.
//!
//! The parser accepts quoted values, any order of the keys and extra keys. It
//! ignores a key that it does not know.

use base64::prelude::{Engine as _, BASE64_STANDARD};
use foundation_core::url::Uri;
use serde::Deserialize;

use crate::shared::error::{SandboxError, SandboxResult};

use super::client::RegistryAuth;

/// One `WWW-Authenticate` challenge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Challenge {
    /// The scheme, `Bearer` or `Basic`.
    pub scheme: String,
    /// The parameters, in the order that the header gave them.
    pub params: Vec<(String, String)>,
}

impl Challenge {
    /// One parameter, by a case-insensitive name.
    #[must_use]
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Whether this challenge names a bearer token.
    #[must_use]
    pub fn is_bearer(&self) -> bool {
        self.scheme.eq_ignore_ascii_case("bearer")
    }

    /// Whether this challenge names basic credentials.
    #[must_use]
    pub fn is_basic(&self) -> bool {
        self.scheme.eq_ignore_ascii_case("basic")
    }
}

/// Parses one `WWW-Authenticate` header.
///
/// It prefers the `Bearer` challenge when the header holds more than one. It
/// returns `None` for a header with no known scheme.
#[must_use]
pub fn parse_www_authenticate(header: &str) -> Option<Challenge> {
    let (scheme, rest) = find_scheme(header, "bearer")
        .or_else(|| find_scheme(header, "basic"))
        .or_else(|| find_scheme(header, "digest"))?;
    Some(Challenge {
        scheme,
        params: parse_params(rest),
    })
}

/// Finds one scheme word and returns its name and the text after it.
fn find_scheme<'a>(header: &'a str, wanted: &str) -> Option<(String, &'a str)> {
    let lower = header.to_ascii_lowercase();
    let mut from = 0;
    while let Some(offset) = lower[from..].find(wanted) {
        let start = from + offset;
        let before_ok = start == 0
            || header[..start]
                .chars()
                .next_back()
                .is_some_and(|character| character.is_whitespace() || character == ',');
        let end = start + wanted.len();
        let after_ok = header[end..]
            .chars()
            .next()
            .is_some_and(char::is_whitespace);
        if before_ok && after_ok {
            return Some((header[start..end].to_string(), &header[end..]));
        }
        from = end;
    }
    None
}

/// Parses comma-separated `key=value` parameters.
fn parse_params(text: &str) -> Vec<(String, String)> {
    let mut params = Vec::new();
    let mut segment = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for character in text.chars() {
        if escaped {
            segment.push(character);
            escaped = false;
            continue;
        }
        match character {
            '\\' if quoted => {
                segment.push(character);
                escaped = true;
            }
            '"' => {
                quoted = !quoted;
                segment.push(character);
            }
            ',' if !quoted => {
                push_param(&mut params, &segment);
                segment.clear();
            }
            _ => segment.push(character),
        }
    }
    push_param(&mut params, &segment);
    params
}

/// Adds one `key=value` segment when it has both parts.
fn push_param(params: &mut Vec<(String, String)>, segment: &str) {
    let Some((key, value)) = segment.split_once('=') else {
        return;
    };
    let key = key.trim();
    if key.is_empty() || key.contains(char::is_whitespace) {
        return;
    }
    params.push((key.to_string(), unquote(value.trim())));
}

/// Removes the quotes and the escapes of one value.
fn unquote(value: &str) -> String {
    let Some(inner) = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    else {
        return value.to_string();
    };
    let mut out = String::with_capacity(inner.len());
    let mut escaped = false;
    for character in inner.chars() {
        if escaped {
            out.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            out.push(character);
        }
    }
    out
}

/// The body of one token answer.
#[derive(Clone, Debug, Default, Deserialize)]
pub(crate) struct TokenResponse {
    /// The token under the `token` key.
    #[serde(default)]
    pub(crate) token: Option<String>,
    /// The token under the `access_token` key.
    #[serde(default)]
    pub(crate) access_token: Option<String>,
    /// The life of the token in seconds.
    #[serde(default)]
    pub(crate) expires_in: Option<u64>,
}

impl TokenResponse {
    /// The token that the answer carries.
    pub(crate) fn into_token(self) -> Option<String> {
        self.token.or(self.access_token)
    }
}

/// The `Basic` header of one registry, when the caller gave a name and a
/// password.
#[must_use]
pub(crate) fn basic_header(auth: Option<&RegistryAuth>) -> Option<String> {
    match auth {
        Some(RegistryAuth::Basic { username, password }) => {
            let credentials = format!("{username}:{}", password.expose());
            Some(format!("Basic {}", BASE64_STANDARD.encode(credentials)))
        }
        Some(RegistryAuth::Anonymous) | None => None,
    }
}

/// Checks that a token realm is `https`, or `http` on an allowed host.
///
/// # Errors
/// [`SandboxError::InvalidSpec`] for a realm scheme or host that the caller did
/// not allow.
pub(crate) fn check_realm(realm: &str, allow_insecure: &[String]) -> SandboxResult<()> {
    let uri = Uri::parse(realm).map_err(|error| SandboxError::InvalidSpec {
        field: "registry".to_string(),
        reason: format!("the token realm is not a URL: {realm}: {error}"),
    })?;
    let scheme = uri.scheme().as_str();
    if scheme.eq_ignore_ascii_case("https") {
        return Ok(());
    }
    if scheme.eq_ignore_ascii_case("http") {
        let host = uri.host_str().unwrap_or_default();
        let authority = match uri.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.clone(),
        };
        if allow_insecure.iter().any(|allowed| {
            allowed.eq_ignore_ascii_case(&authority) || allowed.eq_ignore_ascii_case(&host)
        }) {
            return Ok(());
        }
    }
    Err(SandboxError::InvalidSpec {
        field: "registry".to_string(),
        reason: format!("the token realm is not https and its host is not allowed: {realm}"),
    }
    .into())
}

/// Percent-encodes the characters that a query value must not carry.
///
/// The colon and the slash stay, so the repository scope keeps its usual form.
pub(crate) fn query_value(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b':' | b'/') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(b"0123456789ABCDEF"[usize::from(byte >> 4)]));
            out.push(char::from(b"0123456789ABCDEF"[usize::from(byte & 0x0f)]));
        }
    }
    out
}
