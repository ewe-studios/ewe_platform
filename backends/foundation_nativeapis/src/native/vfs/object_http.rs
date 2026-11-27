//! The S3/R2 Hypertext Transfer Protocol (HTTP) object store.
//!
//! The store builds Amazon Simple Storage Service (S3) REST requests and maps
//! the responses to [`ObjectStore`] results. The transport is injected through
//! [`ObjectTransport`], so the request shaping and the signature are testable
//! without a socket, and the host owns Transmission Control Protocol (TCP) and
//! Transport Layer Security (TLS). A host supplies a transport with its own
//! socket client; this module does not open a connection.
//!
//! # Provider settings
//!
//! | Setting | S3 | R2 |
//! |---|---|---|
//! | Region | `us-east-1` (configurable) | `auto` |
//! | Addressing | virtual-host by default | path-style |
//! | Checksum | off by default | CRC32 |
//!
//! # Signature
//!
//! The store signs every request with AWS Signature Version 4 (SigV4) when
//! credentials are present. The time comes from the injected clock. The store
//! never reads the system clock.
//!
//! # Checksums
//!
//! With CRC32 on, a single PUT sends `x-amz-checksum-crc32`. Each multipart
//! part sends its own CRC32. TLS protects the request in transit.

use crate::shared::vfs::durable::object_store::{
    DeleteReport, ObjectCapabilities, ObjectError, ObjectFuture, ObjectMeta, ObjectStore, Version,
};
use core::fmt::Write as _;
use foundation_compact::time::unix_millis;
use foundation_context::Ctx;
use std::fmt;

/// The HTTP method of one object request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpMethod {
    /// Read an object.
    Get,
    /// Write an object.
    Put,
    /// Start a multipart upload or a batch delete.
    Post,
    /// Read only the headers.
    Head,
    /// Remove an object.
    Delete,
}

impl HttpMethod {
    /// Return the wire text.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Put => "PUT",
            Self::Post => "POST",
            Self::Head => "HEAD",
            Self::Delete => "DELETE",
        }
    }
}

/// One HTTP request to the object store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectRequest {
    /// The method.
    pub method: HttpMethod,
    /// The request target without the query, for example
    /// `https://host/bucket/key`.
    pub url: String,
    /// The query parameters, in insertion order. The signer sorts them.
    pub query: Vec<(String, String)>,
    /// The request headers.
    pub headers: Vec<(String, String)>,
    /// The request body.
    pub body: Vec<u8>,
    /// The largest response body the transport may return, in bytes.
    pub max_response_bytes: usize,
}

impl ObjectRequest {
    /// Return the full URL, with the query appended.
    #[must_use]
    pub fn full_url(&self) -> String {
        if self.query.is_empty() {
            return self.url.clone();
        }
        let query = self
            .query
            .iter()
            .map(|(key, value)| format!("{}={}", encode(key), encode(value)))
            .collect::<Vec<String>>()
            .join("&");
        format!("{}?{query}", self.url)
    }

    fn set_header(&mut self, name: &str, value: impl Into<String>) {
        let value = value.into();
        if let Some((_, current)) = self
            .headers
            .iter_mut()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
        {
            *current = value;
        } else {
            self.headers.push((name.to_string(), value));
        }
    }
}

/// One HTTP response from the object store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectResponse {
    /// The status code.
    pub status: u16,
    /// The response headers.
    pub headers: Vec<(String, String)>,
    /// The response body.
    pub body: Vec<u8>,
}

impl ObjectResponse {
    /// Return a header value.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// The transport under the HTTP object store.
///
/// A host implements this trait with its own socket client. The response body
/// must not exceed `request.max_response_bytes`.
pub trait ObjectTransport: Send + Sync {
    /// Send one request and return the response.
    fn send(&self, request: ObjectRequest) -> ObjectFuture<'_, ObjectResponse>;
}

/// The credentials of one S3 or R2 principal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct S3Credentials {
    /// The access key identifier.
    pub access_key: String,
    /// The secret access key.
    pub secret_key: String,
    /// The session token of a temporary principal.
    pub session_token: Option<String>,
}

/// The HTTP object-store configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct S3HttpConfig {
    /// The service endpoint, for example `https://s3.us-east-1.amazonaws.com`.
    pub endpoint: String,
    /// The bucket name.
    pub bucket: String,
    /// The signing region.
    pub region: String,
    /// True for path-style addressing.
    pub path_style: bool,
    /// True to send CRC32 checksums.
    pub crc32: bool,
    /// The credentials, when the endpoint requires them.
    pub credentials: Option<S3Credentials>,
    /// The largest response body the transport may return, in bytes.
    pub max_response_bytes: usize,
    /// The largest error body the store reads, in bytes.
    pub max_error_body_bytes: usize,
}

impl Default for S3HttpConfig {
    fn default() -> Self {
        Self {
            endpoint: "https://s3.us-east-1.amazonaws.com".to_string(),
            bucket: String::new(),
            region: "us-east-1".to_string(),
            path_style: false,
            crc32: false,
            credentials: None,
            // Amazon S3 object metadata and listings are small. 8 MiB bounds a
            // large range read; a caller that needs more reads in pages.
            max_response_bytes: 8 * 1024 * 1024,
            // nginx's `proxy_buffer_size` default is 8 KiB. An error body
            // larger than this adds nothing to the message.
            max_error_body_bytes: 8 * 1024,
        }
    }
}

impl S3HttpConfig {
    /// Build the R2 defaults: region `auto`, path-style and CRC32.
    #[must_use]
    pub fn r2(endpoint: impl Into<String>, bucket: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            bucket: bucket.into(),
            region: "auto".to_string(),
            path_style: true,
            crc32: true,
            ..Self::default()
        }
    }

    /// Set the credentials.
    #[must_use]
    pub fn with_credentials(mut self, credentials: S3Credentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Set the region.
    #[must_use]
    pub fn with_region(mut self, region: impl Into<String>) -> Self {
        self.region = region.into();
        self
    }

    /// Set path-style addressing.
    #[must_use]
    pub fn with_path_style(mut self, path_style: bool) -> Self {
        self.path_style = path_style;
        self
    }

    /// Set the CRC32 checksum.
    #[must_use]
    pub fn with_crc32(mut self, crc32: bool) -> Self {
        self.crc32 = crc32;
        self
    }

    /// Set the largest response body.
    #[must_use]
    pub fn with_max_response_bytes(mut self, bytes: usize) -> Self {
        self.max_response_bytes = bytes.max(1);
        self
    }
}

/// An object store over S3 REST requests and an injected transport.
pub struct HttpObjectStore<T: ObjectTransport> {
    transport: T,
    config: S3HttpConfig,
    ctx: Ctx,
}

impl<T: ObjectTransport> fmt::Debug for HttpObjectStore<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HttpObjectStore")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl<T: ObjectTransport> HttpObjectStore<T> {
    /// Build the store.
    #[must_use]
    pub fn new(transport: T, config: S3HttpConfig, ctx: &Ctx) -> Self {
        Self {
            transport,
            config,
            ctx: ctx.clone(),
        }
    }

    /// Return the configuration.
    #[must_use]
    pub fn config(&self) -> &S3HttpConfig {
        &self.config
    }

    /// Return the transport.
    #[must_use]
    pub fn transport(&self) -> &T {
        &self.transport
    }

    fn object_url(&self, key: &str) -> String {
        let encoded = key
            .split('/')
            .map(encode)
            .collect::<Vec<String>>()
            .join("/");
        if self.config.path_style {
            format!(
                "{}/{}/{}",
                self.config.endpoint.trim_end_matches('/'),
                encode(&self.config.bucket),
                encoded
            )
        } else {
            let (scheme, rest) = self
                .config
                .endpoint
                .split_once("://")
                .unwrap_or(("https", self.config.endpoint.as_str()));
            format!("{scheme}://{}.{rest}/{encoded}", self.config.bucket)
        }
    }

    fn bucket_url(&self) -> String {
        if self.config.path_style {
            format!(
                "{}/{}",
                self.config.endpoint.trim_end_matches('/'),
                encode(&self.config.bucket)
            )
        } else {
            let (scheme, rest) = self
                .config
                .endpoint
                .split_once("://")
                .unwrap_or(("https", self.config.endpoint.as_str()));
            format!("{scheme}://{}.{rest}", self.config.bucket)
        }
    }

    fn host(&self) -> String {
        let url = self.bucket_url();
        url.split_once("://").map_or(url.clone(), |(_, rest)| {
            rest.split('/').next().unwrap_or(rest).to_string()
        })
    }

    fn request(
        &self,
        method: HttpMethod,
        url: String,
        query: Vec<(String, String)>,
    ) -> ObjectRequest {
        ObjectRequest {
            method,
            url,
            query,
            headers: Vec::new(),
            body: Vec::new(),
            max_response_bytes: self.config.max_response_bytes,
        }
    }

    async fn send(&self, mut request: ObjectRequest) -> Result<ObjectResponse, ObjectError> {
        request.set_header("host", self.host());
        if let Some(credentials) = &self.config.credentials {
            sign_v4(
                &mut request,
                credentials,
                &self.config.region,
                self.now_ms(),
            );
        }
        let response = self.transport.send(request).await?;
        if response.status == 404 {
            return Err(ObjectError::NotFound);
        }
        if response.status == 412 {
            return Err(ObjectError::PreconditionFailed);
        }
        Ok(response)
    }

    fn now_ms(&self) -> i64 {
        unix_millis(self.ctx.clock().wall_now())
    }

    fn backend(status: u16, response: &ObjectResponse, limit: usize) -> ObjectError {
        let text = String::from_utf8_lossy(&response.body[..response.body.len().min(limit)]);
        ObjectError::Backend(Box::new(HttpError(format!(
            "S3 request failed with status {status}: {text}"
        ))))
    }

    fn put(&self, key: &str, bytes: Vec<u8>, condition: PutCondition) -> ObjectFuture<'_, Version> {
        let url = self.object_url(key);
        Box::pin(async move {
            let mut request = self.request(HttpMethod::Put, url, Vec::new());
            match condition {
                PutCondition::None => {}
                PutCondition::IfNoneMatch => request.set_header("if-none-match", "*"),
                PutCondition::IfMatch(ref version) => {
                    request.set_header("if-match", version.as_str());
                }
            }
            self.add_checksum(&mut request, &bytes);
            request.body = bytes;
            let response = self.send(request).await?;
            if response.status != 200 && response.status != 204 {
                return Err(Self::backend(
                    response.status,
                    &response,
                    self.config.max_error_body_bytes,
                ));
            }
            Ok(version_of(&response))
        })
    }

    fn add_checksum(&self, request: &mut ObjectRequest, bytes: &[u8]) {
        if self.config.crc32 {
            request.set_header("x-amz-checksum-crc32", crc32_base64(bytes));
            request.set_header("x-amz-sdk-checksum-algorithm", "CRC32");
        }
    }

    fn multipart(&self, key: &str, parts: Vec<Vec<u8>>) -> ObjectFuture<'_, Version> {
        let url = self.object_url(key);
        Box::pin(async move {
            let mut create = self.request(
                HttpMethod::Post,
                url.clone(),
                vec![("uploads".to_string(), String::new())],
            );
            create.set_header("content-length", "0");
            let created = self.send(create).await?;
            if created.status != 200 {
                return Err(Self::backend(
                    created.status,
                    &created,
                    self.config.max_error_body_bytes,
                ));
            }
            let text = String::from_utf8_lossy(&created.body);
            let upload_id = xml_tag(&text, "UploadId").ok_or_else(|| {
                ObjectError::Backend(Box::new(HttpError(
                    "S3 multipart start returned no UploadId".to_string(),
                )))
            })?;

            let mut completed = Vec::new();
            for (index, part) in parts.iter().enumerate() {
                let number = index.saturating_add(1);
                let mut request = self.request(
                    HttpMethod::Put,
                    url.clone(),
                    vec![
                        ("partNumber".to_string(), number.to_string()),
                        ("uploadId".to_string(), upload_id.clone()),
                    ],
                );
                self.add_checksum(&mut request, part);
                request.body = part.clone();
                let response = self.send(request).await?;
                if response.status != 200 {
                    return Err(Self::backend(
                        response.status,
                        &response,
                        self.config.max_error_body_bytes,
                    ));
                }
                completed.push((number, version_of(&response).0));
            }

            let mut body = String::from("<CompleteMultipartUpload>");
            for (number, etag) in &completed {
                let _ = write!(
                    body,
                    "<Part><PartNumber>{number}</PartNumber><ETag>{etag}</ETag></Part>"
                );
            }
            body.push_str("</CompleteMultipartUpload>");
            let mut complete = self.request(
                HttpMethod::Post,
                url,
                vec![("uploadId".to_string(), upload_id)],
            );
            complete.body = body.into_bytes();
            let response = self.send(complete).await?;
            if response.status != 200 {
                return Err(Self::backend(
                    response.status,
                    &response,
                    self.config.max_error_body_bytes,
                ));
            }
            Ok(version_of(&response))
        })
    }

    fn parse_listing(text: &str, prefix: &str) -> Vec<ObjectMeta> {
        let mut out = Vec::new();
        for block in xml_blocks(text, "Contents") {
            let Some(key) = xml_tag(&block, "Key") else {
                continue;
            };
            if !key.starts_with(prefix) {
                continue;
            }
            let len = xml_tag(&block, "Size")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            let version = xml_tag(&block, "ETag").unwrap_or_default();
            let last_modified_ms = xml_tag(&block, "LastModified")
                .and_then(|value| parse_iso8601_ms(&value))
                .unwrap_or(0);
            out.push(ObjectMeta {
                key,
                len,
                version: Version(version.trim_matches('"').to_string()),
                last_modified_ms,
            });
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PutCondition {
    None,
    IfNoneMatch,
    IfMatch(Version),
}

#[derive(Debug)]
struct HttpError(String);

impl fmt::Display for HttpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl core::error::Error for HttpError {}

fn version_of(response: &ObjectResponse) -> Version {
    Version(
        response
            .header("etag")
            .unwrap_or_default()
            .trim_matches('"')
            .to_string(),
    )
}

impl<T: ObjectTransport> ObjectStore for HttpObjectStore<T> {
    fn put_if_none_match(&self, key: &str, bytes: Vec<u8>) -> ObjectFuture<'_, Version> {
        self.put(key, bytes, PutCondition::IfNoneMatch)
    }

    fn put_if_match(
        &self,
        key: &str,
        bytes: Vec<u8>,
        version: &Version,
    ) -> ObjectFuture<'_, Version> {
        self.put(key, bytes, PutCondition::IfMatch(version.clone()))
    }

    fn put(&self, key: &str, bytes: Vec<u8>) -> ObjectFuture<'_, Version> {
        self.put(key, bytes, PutCondition::None)
    }

    fn put_parts(&self, key: &str, parts: Vec<Vec<u8>>) -> ObjectFuture<'_, Version> {
        self.multipart(key, parts)
    }

    fn get_range(&self, key: &str, start: u64, len: usize) -> ObjectFuture<'_, Vec<u8>> {
        let url = self.object_url(key);
        Box::pin(async move {
            if len == 0 {
                return Ok(Vec::new());
            }
            let mut request = self.request(HttpMethod::Get, url, Vec::new());
            let end = start.saturating_add(u64::try_from(len).unwrap_or(u64::MAX));
            request.set_header("range", format!("bytes={start}-{}", end.saturating_sub(1)));
            request.max_response_bytes = len;
            let response = self.send(request).await?;
            if response.status != 206 && response.status != 200 {
                return Err(Self::backend(
                    response.status,
                    &response,
                    self.config.max_error_body_bytes,
                ));
            }
            if response.status == 206 {
                return Ok(response.body);
            }
            // A server that ignores Range returns the whole object. Keep the
            // requested window, and never return more than the limit.
            let start = usize::try_from(start).unwrap_or(usize::MAX);
            if start > response.body.len() {
                return Err(ObjectError::Backend(Box::new(HttpError(
                    "S3 range starts past the object end".to_string(),
                ))));
            }
            let end = start.saturating_add(len).min(response.body.len());
            Ok(response.body[start..end].to_vec())
        })
    }

    fn get(&self, key: &str) -> ObjectFuture<'_, Vec<u8>> {
        let url = self.object_url(key);
        Box::pin(async move {
            let request = self.request(HttpMethod::Get, url, Vec::new());
            let response = self.send(request).await?;
            if response.status != 200 {
                return Err(Self::backend(
                    response.status,
                    &response,
                    self.config.max_error_body_bytes,
                ));
            }
            Ok(response.body)
        })
    }

    fn head(&self, key: &str) -> ObjectFuture<'_, Option<ObjectMeta>> {
        let url = self.object_url(key);
        let key = key.to_string();
        Box::pin(async move {
            let request = self.request(HttpMethod::Head, url, Vec::new());
            match self.send(request).await {
                Ok(response) if response.status == 200 => Ok(Some(ObjectMeta {
                    key,
                    len: response
                        .header("content-length")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(0),
                    version: version_of(&response),
                    // HEAD carries no portable last-write time. The lifecycle
                    // pass reads the listing, which carries LastModified.
                    last_modified_ms: 0,
                })),
                Ok(response) => Err(Self::backend(
                    response.status,
                    &response,
                    self.config.max_error_body_bytes,
                )),
                Err(ObjectError::NotFound) => Ok(None),
                Err(error) => Err(error),
            }
        })
    }

    fn list(&self, prefix: &str) -> ObjectFuture<'_, Vec<ObjectMeta>> {
        let url = self.bucket_url();
        let prefix = prefix.to_string();
        Box::pin(async move {
            let mut out = Vec::new();
            let mut token: Option<String> = None;
            loop {
                let mut query = vec![
                    ("list-type".to_string(), "2".to_string()),
                    ("prefix".to_string(), prefix.clone()),
                ];
                if let Some(token) = &token {
                    query.push(("continuation-token".to_string(), token.clone()));
                }
                let request = self.request(HttpMethod::Get, url.clone(), query);
                let response = self.send(request).await?;
                if response.status != 200 {
                    return Err(Self::backend(
                        response.status,
                        &response,
                        self.config.max_error_body_bytes,
                    ));
                }
                let text = String::from_utf8_lossy(&response.body);
                out.extend(Self::parse_listing(&text, &prefix));
                let truncated = xml_tag(&text, "IsTruncated")
                    .is_some_and(|value| value.eq_ignore_ascii_case("true"));
                if !truncated {
                    return Ok(out);
                }
                token = xml_tag(&text, "NextContinuationToken");
                if token.is_none() {
                    return Err(ObjectError::Backend(Box::new(HttpError(
                        "S3 listing is truncated with no continuation token".to_string(),
                    ))));
                }
            }
        })
    }

    fn delete(&self, key: &str) -> ObjectFuture<'_, ()> {
        let url = self.object_url(key);
        Box::pin(async move {
            let request = self.request(HttpMethod::Delete, url, Vec::new());
            match self.send(request).await {
                Ok(response) if response.status == 204 || response.status == 200 => Ok(()),
                Ok(response) => Err(Self::backend(
                    response.status,
                    &response,
                    self.config.max_error_body_bytes,
                )),
                // An absent key is not an error.
                Err(ObjectError::NotFound) => Ok(()),
                Err(error) => Err(error),
            }
        })
    }

    fn delete_many(&self, keys: Vec<String>) -> ObjectFuture<'_, DeleteReport> {
        let url = self.bucket_url();
        Box::pin(async move {
            if keys.is_empty() {
                return Ok(DeleteReport::default());
            }
            let mut body = String::from("<Delete><Quiet>false</Quiet>");
            for key in &keys {
                body.push_str("<Object><Key>");
                body.push_str(&xml_escape(key));
                body.push_str("</Key></Object>");
            }
            body.push_str("</Delete>");
            let mut request = self.request(
                HttpMethod::Post,
                url,
                vec![("delete".to_string(), String::new())],
            );
            // Amazon S3 requires Content-MD5 for DeleteObjects. The checksum
            // covers the request body. R2 follows the same rule.
            request.set_header("content-md5", md5_base64(body.as_bytes()));
            request.max_response_bytes = self.config.max_error_body_bytes;
            request.body = body.into_bytes();
            let response = self.send(request).await?;
            if response.status != 200 {
                return Err(Self::backend(
                    response.status,
                    &response,
                    self.config.max_error_body_bytes,
                ));
            }
            let text = String::from_utf8_lossy(&response.body);
            let mut report = DeleteReport::default();
            for block in xml_blocks(&text, "Deleted") {
                if let Some(key) = xml_tag(&block, "Key") {
                    report.deleted.push(key);
                }
            }
            for block in xml_blocks(&text, "Error") {
                let Some(key) = xml_tag(&block, "Key") else {
                    continue;
                };
                let message = xml_tag(&block, "Message")
                    .or_else(|| xml_tag(&block, "Code"))
                    .unwrap_or_default();
                report.errors.push((key, message));
            }
            // Every requested key must appear in exactly one result.
            for key in keys {
                if !report.deleted.contains(&key)
                    && !report.errors.iter().any(|(failed, _)| failed == &key)
                {
                    report
                        .errors
                        .push((key, "the delete response omits the key".to_string()));
                }
            }
            Ok(report)
        })
    }

    fn capabilities(&self) -> ObjectCapabilities {
        ObjectCapabilities {
            conditional_write: true,
            multipart: true,
            // Amazon S3 accepts a single PUT up to 5 GiB.
            max_single_put_bytes: 5 * 1024 * 1024 * 1024,
            checksum_crc32: self.config.crc32,
            strong_consistency: true,
        }
    }
}

/// Sign a request with AWS Signature Version 4.
pub fn sign_v4(
    request: &mut ObjectRequest,
    credentials: &S3Credentials,
    region: &str,
    now_ms: i64,
) {
    let (amz_date, date) = amz_date(now_ms);
    let payload_hash = hex(&sha256(&request.body));
    request.set_header("x-amz-date", amz_date.clone());
    request.set_header("x-amz-content-sha256", payload_hash.clone());
    if let Some(token) = &credentials.session_token {
        request.set_header("x-amz-security-token", token.clone());
    }

    let canonical_uri = canonical_uri(&request.url);
    let canonical_query = canonical_query(&request.query);
    let mut headers: Vec<(String, String)> = request
        .headers
        .iter()
        .map(|(key, value)| (key.to_ascii_lowercase(), value.trim().to_string()))
        .collect();
    headers.sort_by(|left, right| left.0.cmp(&right.0));
    let mut canonical_headers = String::new();
    for (key, value) in &headers {
        let _ = writeln!(canonical_headers, "{key}:{value}");
    }
    let signed_headers = headers
        .iter()
        .map(|(key, _)| key.clone())
        .collect::<Vec<String>>()
        .join(";");
    let canonical_request = format!(
        "{}\n{canonical_uri}\n{canonical_query}\n{canonical_headers}\n{signed_headers}\n{payload_hash}",
        request.method.as_str()
    );
    let scope = format!("{date}/{region}/s3/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex(&sha256(canonical_request.as_bytes()))
    );
    let key = signing_key(&credentials.secret_key, &date, region);
    let signature = hex(&hmac_sha256(&key, string_to_sign.as_bytes()));
    request.set_header(
        "authorization",
        format!(
            "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed_headers}, Signature={signature}",
            credentials.access_key
        ),
    );
}

fn signing_key(secret: &str, date: &str, region: &str) -> Vec<u8> {
    let initial = format!("AWS4{secret}");
    let date_key = hmac_sha256(initial.as_bytes(), date.as_bytes());
    let region_key = hmac_sha256(&date_key, region.as_bytes());
    let service_key = hmac_sha256(&region_key, b"s3");
    hmac_sha256(&service_key, b"aws4_request")
}

fn canonical_uri(url: &str) -> String {
    // The path is already percent-encoded by the caller. A second encoding
    // would change the signed path and break the signature.
    url.split_once("://")
        .and_then(|(_, rest)| rest.find('/').map(|index| rest[index..].to_string()))
        .unwrap_or_else(|| "/".to_string())
}

fn canonical_query(query: &[(String, String)]) -> String {
    let mut items: Vec<(String, String)> = query
        .iter()
        .map(|(key, value)| (encode(key), encode(value)))
        .collect();
    items.sort();
    items
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<String>>()
        .join("&")
}

fn amz_date(now_ms: i64) -> (String, String) {
    let seconds = now_ms.div_euclid(1000);
    let days = seconds.div_euclid(86_400);
    let rem = seconds.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let second = rem % 60;
    (
        format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z"),
        format!("{year:04}{month:02}{day:02}"),
    )
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_part = (5 * day_of_year + 2) / 153;
    let day = u32::try_from(day_of_year - (153 * month_part + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if month_part < 10 {
        month_part + 3
    } else {
        month_part - 9
    })
    .unwrap_or(1);
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = i64::from(month);
    let day = i64::from(day);
    let day_of_year = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

fn parse_iso8601_ms(text: &str) -> Option<i64> {
    let text = text.trim();
    let date = text.get(..10)?;
    let mut parts = date.split('-');
    let year: i64 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    let time = text.get(11..19).unwrap_or("00:00:00");
    let mut parts = time.split(':');
    let hour: i64 = parts.next()?.parse().ok()?;
    let minute: i64 = parts.next()?.parse().ok()?;
    let second: i64 = parts.next()?.parse().ok()?;
    let seconds = days_from_civil(year, month, day) * 86_400 + hour * 3600 + minute * 60 + second;
    Some(seconds.saturating_mul(1000))
}

fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            const HEX: &[u8; 16] = b"0123456789ABCDEF";
            out.push('%');
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    out
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn xml_blocks(text: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(&open) {
        let after = &rest[start + open.len()..];
        let Some(end) = after.find(&close) else {
            break;
        };
        out.push(after[..end].to_string());
        rest = &after[end + close.len()..];
    }
    out
}

fn xml_tag(text: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = text.find(&open)? + open.len();
    let end = text[start..].find(&close)? + start;
    Some(text[start..end].to_string())
}

/// The base64 form of the Message Digest 5 (MD5) of `bytes`.
///
/// Amazon S3 requires this value as `Content-MD5` on a batch delete.
#[must_use]
pub fn md5_base64(bytes: &[u8]) -> String {
    base64(&md5(bytes))
}

// The MD5 state names follow the published algorithm (a, b, c, d), so the
// single-character names match the source of truth. The unrolled rounds also
// make the function long by design.
#[allow(clippy::many_single_char_names, clippy::too_many_lines)]
fn md5(data: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    const K: [u32; 64] = [
        0xd76a_a478,
        0xe8c7_b756,
        0x2420_70db,
        0xc1bd_ceee,
        0xf57c_0faf,
        0x4787_c62a,
        0xa830_4613,
        0xfd46_9501,
        0x6980_98d8,
        0x8b44_f7af,
        0xffff_5bb1,
        0x895c_d7be,
        0x6b90_1122,
        0xfd98_7193,
        0xa679_438e,
        0x49b4_0821,
        0xf61e_2562,
        0xc040_b340,
        0x265e_5a51,
        0xe9b6_c7aa,
        0xd62f_105d,
        0x0244_1453,
        0xd8a1_e681,
        0xe7d3_fbc8,
        0x21e1_cde6,
        0xc337_07d6,
        0xf4d5_0d87,
        0x455a_14ed,
        0xa9e3_e905,
        0xfcef_a3f8,
        0x676f_02d9,
        0x8d2a_4c8a,
        0xfffa_3942,
        0x8771_f681,
        0x6d9d_6122,
        0xfde5_380c,
        0xa4be_ea44,
        0x4bde_cfa9,
        0xf6bb_4b60,
        0xbebf_bc70,
        0x289b_7ec6,
        0xeaa1_27fa,
        0xd4ef_3085,
        0x0488_1d05,
        0xd9d4_d039,
        0xe6db_99e5,
        0x1fa2_7cf8,
        0xc4ac_5665,
        0xf429_2244,
        0x432a_ff97,
        0xab94_23a7,
        0xfc93_a039,
        0x655b_59c3,
        0x8f0c_cc92,
        0xffef_f47d,
        0x8584_5dd1,
        0x6fa8_7e4f,
        0xfe2c_e6e0,
        0xa301_4314,
        0x4e08_11a1,
        0xf753_7e82,
        0xbd3a_f235,
        0x2ad7_d2bb,
        0xeb86_d391,
    ];
    let bit_len = u64::try_from(data.len())
        .unwrap_or(u64::MAX)
        .saturating_mul(8);
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_le_bytes());

    let mut a0 = 0x6745_2301_u32;
    let mut b0 = 0xefcd_ab89_u32;
    let mut c0 = 0x98ba_dcfe_u32;
    let mut d0 = 0x1032_5476_u32;
    for chunk in message.as_chunks::<64>().0 {
        let mut words = [0_u32; 16];
        for (index, word) in words.iter_mut().enumerate() {
            let start = index * 4;
            let mut bytes = [0_u8; 4];
            bytes.copy_from_slice(&chunk[start..start + 4]);
            *word = u32::from_le_bytes(bytes);
        }
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for index in 0..64 {
            let (f, g) = match index {
                0..=15 => ((b & c) | (!b & d), index),
                16..=31 => ((d & b) | (!d & c), (5 * index + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * index + 5) % 16),
                _ => (c ^ (b | !d), (7 * index) % 16),
            };
            let rotated = a
                .wrapping_add(f)
                .wrapping_add(K[index])
                .wrapping_add(words[g])
                .rotate_left(S[index]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(rotated);
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    let mut out = [0_u8; 16];
    out[..4].copy_from_slice(&a0.to_le_bytes());
    out[4..8].copy_from_slice(&b0.to_le_bytes());
    out[8..12].copy_from_slice(&c0.to_le_bytes());
    out[12..].copy_from_slice(&d0.to_le_bytes());
    out
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn crc32_base64(data: &[u8]) -> String {
    base64(&crc32(data).to_be_bytes())
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = u32::from(chunk[0]);
        let second = chunk.get(1).copied().map_or(0, u32::from);
        let third = chunk.get(2).copied().map_or(0, u32::from);
        let triple = (first << 16) | (second << 8) | third;
        out.push(char::from(ALPHABET[((triple >> 18) & 0x3f) as usize]));
        out.push(char::from(ALPHABET[((triple >> 12) & 0x3f) as usize]));
        out.push(if chunk.len() > 1 {
            char::from(ALPHABET[((triple >> 6) & 0x3f) as usize])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(ALPHABET[(triple & 0x3f) as usize])
        } else {
            '='
        });
    }
    out
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{KeyInit as _, Mac as _};
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(key).expect("HMAC accepts any key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn sha256(data: &[u8]) -> Vec<u8> {
    use sha2::Digest as _;
    sha2::Sha256::digest(data).to_vec()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}
