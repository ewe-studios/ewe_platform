---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/09-aws-lambda-api-provider"
this_file: "specifications/11-foundation-deployment/features/09-aws-lambda-api-provider/feature.md"

status: pending
priority: high
created: 2026-04-06

depends_on: ["01-foundation-deployment-core", "02-state-stores", "03-deployment-engine", "06-aws-lambda-cli-provider"]

tasks:
  completed: 0
  uncompleted: 11
  total: 11
  completion_percentage: 0%
---


# AWS Lambda API Provider (API-First)

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Implement the **API-first** AWS Lambda deployment provider. This provider calls the AWS Lambda API directly via `SimpleHttpClient` with **SigV4 request signing implemented in-house** — **no aws-sdk or sam CLI dependency**.

This is distinct from `06-aws-lambda-provider` which is CLI-based (sam/aws wrapper). The API provider:

- **Deploys via REST API only** - function create/update via `lambda.{region}.amazonaws.com`
- **SigV4 signing** - implement AWS Signature Version 4 from scratch (no aws-sdk)
- **S3 upload** - upload deployment artifacts to S3 via SigV4-signed requests
- **No CLI fallback** - this provider is API-only; use `06-aws-lambda-provider` for CLI mode
- **Full Lambda features** - function management, versions, aliases, environment variables, layers
- **API Gateway integration** - create/update HTTP API endpoints

### Relationship to CLI Provider

| Feature | `06-aws-lambda-provider` (CLI) | `06-aws-lambda-api-provider` (API-First) |
|---------|--------------------------------|------------------------------------------|
| Dependency | Requires `sam` or `aws` CLI | No external dependencies |
| Auth | CLI handles SigV4 | In-house SigV4 implementation |
| Deploy | `sam deploy` command | Direct REST API calls |
| S3 Upload | `aws s3 cp` command | SigV4-signed S3 API |
| Use Case | Local dev, SAM templates | CI/CD, production automation |

## Dependencies

Depends on:
- `01-foundation-deployment-core` - `DeploymentProvider` trait, `ShellExecutor`
- `02-state-stores` - `StateStore` for persistence
- `03-deployment-engine` - `DeploymentPlanner` for orchestration
- `06-aws-lambda-provider` - Reference for config parsing (`SamTemplate`)

Required by:
- `07-templates` - AWS-specific template configs
- `09-examples-documentation` - AWS examples

## Requirements

### Authentication

```rust
// providers/aws_api/auth.rs

use foundation_core::simple_http::client::SimpleHttpClient;

/// AWS SigV4 authentication using access key and secret key.
///
/// Required environment variables:
/// - `AWS_ACCESS_KEY_ID` - AWS access key ID
/// - `AWS_SECRET_ACCESS_KEY` - AWS secret access key
/// - `AWS_DEFAULT_REGION` - AWS region (e.g., "us-east-1")
///
/// IAM permissions required:
/// - `lambda:*` - Full Lambda access
/// - `s3:*` - S3 access for code uploads
/// - `apigateway:*` - API Gateway access (optional)
#[derive(Debug, Clone)]
pub struct AwsAuth {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub region: String,
    pub session_token: Option<String>,  // For temporary credentials
}

impl AwsAuth {
    /// Create from environment variables.
    /// Returns `None` if required variables are missing.
    pub fn from_env() -> Option<Self> {
        let access_key_id = std::env::var("AWS_ACCESS_KEY_ID").ok()?;
        let secret_access_key = std::env::var("AWS_SECRET_ACCESS_KEY").ok()?;
        let region = std::env::var("AWS_DEFAULT_REGION").ok()?;
        let session_token = std::env::var("AWS_SESSION_TOKEN").ok();
        
        Some(Self {
            access_key_id,
            secret_access_key,
            region,
            session_token,
        })
    }

    /// Create from explicit values.
    pub fn new(access_key_id: &str, secret_access_key: &str, region: &str) -> Self {
        Self {
            access_key_id: access_key_id.to_string(),
            secret_access_key: secret_access_key.to_string(),
            region: region.to_string(),
            session_token: None,
        }
    }

    /// Create with session token (for temporary credentials from STS).
    pub fn with_session_token(
        access_key_id: &str,
        secret_access_key: &str,
        region: &str,
        session_token: &str,
    ) -> Self {
        Self {
            access_key_id: access_key_id.to_string(),
            secret_access_key: secret_access_key.to_string(),
            region: region.to_string(),
            session_token: Some(session_token.to_string()),
        }
    }

    /// Validate credentials format (basic check).
    pub fn validate(&self) -> Result<(), AwsAuthError> {
        if self.access_key_id.is_empty() {
            return Err(AwsAuthError::MissingAccessKeyId);
        }
        if self.secret_access_key.is_empty() {
            return Err(AwsAuthError::MissingSecretAccessKey);
        }
        if self.region.is_empty() {
            return Err(AwsAuthError::MissingRegion);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum AwsAuthError {
    MissingAccessKeyId,
    MissingSecretAccessKey,
    MissingRegion,
}
```

### SigV4 Implementation

```rust
// providers/aws_api/sigv4.rs

use chrono::{Utc, Duration};
use sha2::{Sha256, Digest};
use hmac::{Hmac, Mac};

type HmacSha256 = Hmac<Sha256>;

/// AWS Signature Version 4 implementation.
///
/// This is a complete in-house implementation with no aws-sdk dependency.
/// Reference: https://docs.aws.amazon.com/general/latest/gr/sigv4-signed-request-examples.html
pub struct SigV4Signer {
    access_key_id: String,
    secret_access_key: String,
    region: String,
    service: String,  // "lambda", "s3", "apigateway"
    session_token: Option<String>,
}

impl SigV4Signer {
    pub fn new(access_key_id: &str, secret_access_key: &str, region: &str, service: &str) -> Self {
        Self {
            access_key_id: access_key_id.to_string(),
            secret_access_key: secret_access_key.to_string(),
            region: region.to_string(),
            service: service.to_string(),
            session_token: None,
        }
    }

    pub fn with_session_token(mut self, session_token: &str) -> Self {
        self.session_token = Some(session_token.to_string());
        self
    }

    /// Sign an HTTP request with SigV4.
    ///
    /// Returns the Authorization header value and updated headers.
    pub fn sign_request(
        &self,
        method: &str,
        url: &str,
        headers: &mut Vec<(String, String)>,
        body: &[u8],
    ) -> String {
        let now = Utc::now();
        let datestamp = now.format("%Y%m%d").to_string();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();

        // Add required headers
        self.add_required_headers(headers, &amz_date, &datestamp);

        // Create canonical request
        let canonical_request = self.create_canonical_request(method, url, headers, body);

        // Create string to sign
        let credential_scope = format!("{}/{}/{}/aws4_request", datestamp, self.region, self.service);
        let string_to_sign = self.create_string_to_sign(&canonical_request, &amz_date, &credential_scope);

        // Calculate signature
        let signature = self.calculate_signature(&string_to_sign, &datestamp, &credential_scope);

        // Build Authorization header
        let credential = format!("{}/{}/{}", self.access_key_id, datestamp, credential_scope);
        format!(
            "AWS4-HMAC-SHA256 Credential={}, SignedHeaders={}, Signature={}",
            credential,
            self.get_signed_headers(headers),
            signature
        )
    }

    fn add_required_headers(
        &self,
        headers: &mut Vec<(String, String)>,
        amz_date: &str,
        datestamp: &str,
    ) {
        // Add or replace x-amz-date
        headers.retain(|(k, _)| k.to_lowercase() != "x-amz-date");
        headers.push(("x-amz-date".to_string(), amz_date.to_string()));

        // Add or replace host
        headers.retain(|(k, _)| k.to_lowercase() != "host");
        if let Ok(url_parsed) = url::Url::parse(url) {
            if let Some(host) = url_parsed.host_str() {
                headers.push(("host".to_string(), host.to_string()));
            }
        }

        // Add x-amz-content-sha256
        let body_hash = hex::encode(Sha256::digest(body));
        headers.push(("x-amz-content-sha256".to_string(), body_hash));

        // Add x-amz-security-token if using temporary credentials
        if let Some(token) = &self.session_token {
            headers.push(("x-amz-security-token".to_string(), token.to_string()));
        }
    }

    fn create_canonical_request(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: &[u8],
    ) -> String {
        let url_parsed = url::Url::parse(url).expect("Invalid URL");
        
        // Canonical URI
        let canonical_uri = url_parsed.path().to_string();

        // Canonical query string
        let canonical_query = self.canonicalize_query(url_parsed.query());

        // Canonical headers
        let canonical_headers = self.canonicalize_headers(headers);

        // Signed headers
        let signed_headers = self.get_signed_headers(headers);

        // Payload hash
        let payload_hash = hex::encode(Sha256::digest(body));

        format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method.to_uppercase(),
            canonical_uri,
            canonical_query,
            canonical_headers,
            signed_headers,
            payload_hash
        )
    }

    fn canonicalize_query(&self, query: Option<&str>) -> String {
        match query {
            None => String::new(),
            Some(q) => {
                let mut params: Vec<_> = url::form_urlencoded::parse(q.as_bytes()).collect();
                params.sort_by(|a, b| a.0.cmp(&b.0));
                params
                    .iter()
                    .map(|(k, v)| format!("{}={}", self.uri_encode(k), self.uri_encode(v)))
                    .collect::<Vec<_>>()
                    .join("&")
            }
        }
    }

    fn canonicalize_headers(&self, headers: &[(String, String)]) -> String {
        let mut sorted: Vec<_> = headers
            .iter()
            .map(|(k, v)| (k.to_lowercase(), v.trim().to_string()))
            .collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        
        sorted
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn get_signed_headers(&self, headers: &[(String, String)]) -> String {
        let mut sorted: Vec<_> = headers
            .iter()
            .map(|(k, _)| k.to_lowercase())
            .collect();
        sorted.sort();
        sorted.dedup();
        sorted.join(";")
    }

    fn create_string_to_sign(
        &self,
        canonical_request: &str,
        amz_date: &str,
        credential_scope: &str,
    ) -> String {
        let canonical_hash = hex::encode(Sha256::digest(canonical_request.as_bytes()));
        format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date,
            credential_scope,
            canonical_hash
        )
    }

    fn calculate_signature(
        &self,
        string_to_sign: &str,
        datestamp: &str,
        credential_scope: &str,
    ) -> String {
        // kSecret = "AWS4" + secret_key
        let k_secret = format!("AWS4{}", self.secret_access_key);
        
        // kDate = HMAC-SHA256(kSecret, datestamp)
        let k_date = self.hmac_sha256(k_secret.as_bytes(), datestamp.as_bytes());
        
        // kRegion = HMAC-SHA256(kDate, region)
        let k_region = self.hmac_sha256(&k_date, self.region.as_bytes());
        
        // kService = HMAC-SHA256(kRegion, service)
        let k_service = self.hmac_sha256(&k_region, self.service.as_bytes());
        
        // kSigning = HMAC-SHA256(kService, "aws4_request")
        let k_signing = self.hmac_sha256(&k_service, b"aws4_request");
        
        // signature = HMAC-SHA256(kSigning, string_to_sign)
        let signature = self.hmac_sha256(&k_signing, string_to_sign.as_bytes());
        
        hex::encode(signature)
    }

    fn hmac_sha256(&self, key: &[u8], data: &[u8]) -> Vec<u8> {
        let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    }

    fn uri_encode(&self, s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}
```

### API Client Structure

```rust
// providers/aws_api/client.rs

use foundation_core::simple_http::client::SimpleHttpClient;

const LAMBDA_API_BASE: &str = "https://lambda.{region}.amazonaws.com/2015-03-31";
const S3_API_BASE: &str = "https://s3.{region}.amazonaws.com";
const API_GATEWAY_BASE: &str = "https://apigateway.{region}.amazonaws.com";

/// AWS Lambda REST API client using SimpleHttpClient with SigV4 signing.
/// All methods are API-only — no CLI fallback.
pub struct AwsLambdaApiClient {
    client: SimpleHttpClient,
    signer: SigV4Signer,
    region: String,
}

impl AwsLambdaApiClient {
    pub fn new(access_key_id: &str, secret_access_key: &str, region: &str) -> Self {
        let signer = SigV4Signer::new(access_key_id, secret_access_key, region, "lambda");
        Self {
            client: SimpleHttpClient::new(),
            signer,
            region: region.to_string(),
        }
    }

    // =======================================================================
    // Lambda Functions API
    // =======================================================================

    /// POST /2015-03-31/functions
    ///
    /// Create a new Lambda function.
    pub async fn create_function(
        &self,
        function_name: &str,
        runtime: &str,
        handler: &str,
        role_arn: &str,
        code_s3_bucket: &str,
        code_s3_key: &str,
        environment: Option<&HashMap<String, String>>,
    ) -> Result<LambdaFunction, AwsApiError> {
        let url = format!(
            "https://lambda.{}.amazonaws.com/2015-03-31/functions",
            self.region
        );

        let body = serde_json::json!({
            "FunctionName": function_name,
            "Runtime": runtime,
            "Handler": handler,
            "Role": role_arn,
            "Code": {
                "S3Bucket": code_s3_bucket,
                "S3Key": code_s3_key,
            },
            "Environment": environment.map(|vars| {
                serde_json::json!({ "Variables": vars })
            }),
        });

        self.signed_post(&url, body.to_string().as_bytes()).await
    }

    /// PUT /2015-03-31/functions/{function_name}/code
    ///
    /// Update the code of an existing Lambda function.
    pub async fn update_function_code(
        &self,
        function_name: &str,
        code_s3_bucket: &str,
        code_s3_key: &str,
    ) -> Result<LambdaFunction, AwsApiError> {
        let url = format!(
            "https://lambda.{}.amazonaws.com/2015-03-31/functions/{}/code",
            self.region, function_name
        );

        let body = serde_json::json!({
            "S3Bucket": code_s3_bucket,
            "S3Key": code_s3_key,
        });

        self.signed_put(&url, body.to_string().as_bytes()).await
    }

    /// PUT /2015-03-31/functions/{function_name}/configuration
    ///
    /// Update the configuration of an existing Lambda function.
    pub async fn update_function_configuration(
        &self,
        function_name: &str,
        runtime: Option<&str>,
        handler: Option<&str>,
        memory_size: Option<u32>,
        timeout: Option<u32>,
        environment: Option<&HashMap<String, String>>,
    ) -> Result<LambdaFunction, AwsApiError> {
        let url = format!(
            "https://lambda.{}.amazonaws.com/2015-03-31/functions/{}/configuration",
            self.region, function_name
        );

        let mut body = serde_json::Map::new();
        if let Some(r) = runtime {
            body.insert("Runtime".to_string(), serde_json::json!(r));
        }
        if let Some(h) = handler {
            body.insert("Handler".to_string(), serde_json::json!(h));
        }
        if let Some(m) = memory_size {
            body.insert("MemorySize".to_string(), serde_json::json!(m));
        }
        if let Some(t) = timeout {
            body.insert("Timeout".to_string(), serde_json::json!(t));
        }
        if let Some(env) = environment {
            body.insert(
                "Environment".to_string(),
                serde_json::json!({ "Variables": env }),
            );
        }

        self.signed_put(&url, serde_json::to_string(&body)?.as_bytes()).await
    }

    /// GET /2015-03-31/functions/{function_name}
    pub async fn get_function(
        &self,
        function_name: &str,
    ) -> Result<LambdaFunction, AwsApiError> {
        let url = format!(
            "https://lambda.{}.amazonaws.com/2015-03-31/functions/{}",
            self.region, function_name
        );

        self.signed_get(&url).await
    }

    /// DELETE /2015-03-31/functions/{function_name}
    pub async fn delete_function(&self, function_name: &str) -> Result<(), AwsApiError> {
        let url = format!(
            "https://lambda.{}.amazonaws.com/2015-03-31/functions/{}",
            self.region, function_name
        );

        self.signed_delete(&url).await
    }

    // =======================================================================
    // Lambda Versions and Aliases API
    // =======================================================================

    /// POST /2015-03-31/functions/{function_name}/versions
    ///
    /// Publish a new version of the function.
    pub async fn publish_version(
        &self,
        function_name: &str,
        description: Option<&str>,
    ) -> Result<LambdaVersion, AwsApiError> {
        let url = format!(
            "https://lambda.{}.amazonaws.com/2015-03-31/functions/{}/versions",
            self.region, function_name
        );

        let body = description.map(|d| serde_json::json!({ "Description": d }))
            .unwrap_or_else(|| serde_json::json!({}));

        self.signed_post(&url, body.to_string().as_bytes()).await
    }

    /// POST /2015-03-31/functions/{function_name}/aliases
    ///
    /// Create a new alias.
    pub async fn create_alias(
        &self,
        function_name: &str,
        alias_name: &str,
        function_version: &str,
    ) -> Result<LambdaAlias, AwsApiError> {
        let url = format!(
            "https://lambda.{}.amazonaws.com/2015-03-31/functions/{}/aliases",
            self.region, function_name
        );

        let body = serde_json::json!({
            "Name": alias_name,
            "FunctionVersion": function_version,
        });

        self.signed_post(&url, body.to_string().as_bytes()).await
    }

    /// PUT /2015-03-31/functions/{function_name}/aliases/{alias_name}
    ///
    /// Update an alias to point to a different version.
    pub async fn update_alias(
        &self,
        function_name: &str,
        alias_name: &str,
        function_version: &str,
    ) -> Result<LambdaAlias, AwsApiError> {
        let url = format!(
            "https://lambda.{}.amazonaws.com/2015-03-31/functions/{}/aliases/{}",
            self.region, function_name, alias_name
        );

        let body = serde_json::json!({
            "FunctionVersion": function_version,
        });

        self.signed_put(&url, body.to_string().as_bytes()).await
    }

    /// GET /2015-03-31/functions/{function_name}/aliases/{alias_name}
    pub async fn get_alias(
        &self,
        function_name: &str,
        alias_name: &str,
    ) -> Result<LambdaAlias, AwsApiError> {
        let url = format!(
            "https://lambda.{}.amazonaws.com/2015-03-31/functions/{}/aliases/{}",
            self.region, function_name, alias_name
        );

        self.signed_get(&url).await
    }

    // =======================================================================
    // S3 API for Code Upload
    // =======================================================================

    /// PUT /{bucket}/{key}
    ///
    /// Upload a deployment artifact (zip file) to S3.
    pub async fn upload_to_s3(
        &self,
        bucket: &str,
        key: &str,
        code: &[u8],
    ) -> Result<(), AwsApiError> {
        let url = format!(
            "https://s3.{}.amazonaws.com/{}/{}",
            self.region, bucket, key
        );

        let s3_signer = SigV4Signer::new(
            &self.signer.access_key_id,
            &self.signer.secret_access_key,
            &self.region,
            "s3",
        );

        let mut headers = vec![];
        let auth = s3_signer.sign_request("PUT", &url, &mut headers, code);
        headers.push(("Authorization".to_string(), auth));
        headers.push(("Content-Type".to_string(), "application/zip".to_string()));
        headers.push(("Content-Length".to_string(), code.len().to_string()));

        let response = self.client.put(&url, code, headers).await?;
        
        if response.status().is_success() {
            Ok(())
        } else {
            Err(AwsApiError::S3UploadFailed(format!(
                "S3 upload returned status {}",
                response.status()
            )))
        }
    }

    // =======================================================================
    // API Gateway Integration
    // =======================================================================

    /// Create HTTP API integration for Lambda function.
    pub async fn create_http_api(
        &self,
        api_name: &str,
        function_arn: &str,
    ) -> Result<HttpApi, AwsApiError> {
        // API Gateway v2 (HTTP APIs)
        let url = "https://apigateway.us-east-1.amazonaws.com/v2/apis";

        let body = serde_json::json!({
            "Name": api_name,
            "ProtocolType": "HTTP",
            "Target": function_arn,
        });

        // Note: API Gateway requires separate SigV4 signer with "apigateway" service
        todo!("Implement API Gateway integration")
    }

    // =======================================================================
    // Helper Methods
    // =======================================================================

    async fn signed_get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, AwsApiError> {
        let mut headers = vec![];
        let body: &[u8] = &[];
        let auth = self.signer.sign_request("GET", url, &mut headers, body);
        headers.push(("Authorization".to_string(), auth));

        let response = self.client.get_with_headers(url, headers).await?;
        serde_json::from_slice(response.body()).map_err(|e| {
            AwsApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    async fn signed_post<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &[u8],
    ) -> Result<T, AwsApiError> {
        let mut headers = vec![];
        let auth = self.signer.sign_request("POST", url, &mut headers, body);
        headers.push(("Authorization".to_string(), auth));
        headers.push(("Content-Type".to_string(), "application/json".to_string()));

        let response = self.client.post(url, body, headers).await?;
        serde_json::from_slice(response.body()).map_err(|e| {
            AwsApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    async fn signed_put<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
        body: &[u8],
    ) -> Result<T, AwsApiError> {
        let mut headers = vec![];
        let auth = self.signer.sign_request("PUT", url, &mut headers, body);
        headers.push(("Authorization".to_string(), auth));
        headers.push(("Content-Type".to_string(), "application/json".to_string()));

        let response = self.client.put(url, body, headers).await?;
        serde_json::from_slice(response.body()).map_err(|e| {
            AwsApiError::ParseError(format!("Failed to parse response: {}", e))
        })
    }

    async fn signed_delete(&self, url: &str) -> Result<(), AwsApiError> {
        let mut headers = vec![];
        let body: &[u8] = &[];
        let auth = self.signer.sign_request("DELETE", url, &mut headers, body);
        headers.push(("Authorization".to_string(), auth));

        let response = self.client.delete_with_headers(url, headers).await?;
        
        if response.status().is_success() {
            Ok(())
        } else {
            Err(AwsApiError::ApiError(format!(
                "Delete returned status {}",
                response.status()
            )))
        }
    }
}
```

### Error Types

```rust
// providers/aws_api/error.rs

use foundation_core::simple_http::client::HttpClientError;

/// AWS API-specific error types.
#[derive(Debug)]
pub enum AwsApiError {
    /// Invalid credentials.
    InvalidCredentials(String),

    /// SigV4 signing failed.
    SigningError(String),

    /// API returned an error response.
    ApiError(String),

    /// HTTP client error.
    HttpError(HttpClientError),

    /// Unexpected response structure.
    UnexpectedResponse,

    /// JSON parse error.
    ParseError(String),

    /// Serialization error.
    SerializeError(String),

    /// Resource not found.
    NotFound(String),

    /// Permission denied (insufficient IAM permissions).
    PermissionDenied(String),

    /// S3 upload failed.
    S3UploadFailed(String),

    /// Function already exists.
    FunctionAlreadyExists(String),

    /// Invalid function configuration.
    InvalidConfiguration(String),

    /// Service limit exceeded.
    LimitExceeded(String),
}

impl std::error::Error for AwsApiError {}

impl std::fmt::Display for AwsApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCredentials(reason) => {
                write!(f, "Invalid AWS credentials: {}", reason)
            }
            Self::SigningError(msg) => write!(f, "SigV4 signing failed: {}", msg),
            Self::ApiError(msg) => write!(f, "AWS API error: {}", msg),
            Self::HttpError(e) => write!(f, "HTTP error: {}", e),
            Self::UnexpectedResponse => write!(f, "Unexpected response from AWS API"),
            Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
            Self::SerializeError(msg) => write!(f, "Serialization error: {}", msg),
            Self::NotFound(resource) => write!(f, "Resource not found: {}", resource),
            Self::PermissionDenied(reason) => write!(f, "Permission denied: {}", reason),
            Self::S3UploadFailed(msg) => write!(f, "S3 upload failed: {}", msg),
            Self::FunctionAlreadyExists(name) => {
                write!(f, "Lambda function already exists: {}", name)
            }
            Self::InvalidConfiguration(reason) => {
                write!(f, "Invalid Lambda configuration: {}", reason)
            }
            Self::LimitExceeded(limit) => write!(f, "Service limit exceeded: {}", limit),
        }
    }
}

impl From<HttpClientError> for AwsApiError {
    fn from(e: HttpClientError) -> Self {
        Self::HttpError(e)
    }
}

impl From<serde_json::Error> for AwsApiError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializeError(e.to_string())
    }
}
```

### AwsLambdaApiProvider Implementation

```rust
// providers/aws_api/mod.rs

use std::path::{Path, PathBuf};
use crate::core::traits::DeploymentProvider;
use crate::core::types::{BuildOutput, DeploymentResult, ArtifactType};
use crate::error::DeploymentError;
use super::aws_lambda::SamTemplate; // Reuse config from CLI provider

pub struct AwsLambdaApiProvider {
    working_dir: PathBuf,
    auth: AwsAuth,
    client: AwsLambdaApiClient,
    s3_bucket: String,
}

impl AwsLambdaApiProvider {
    /// Create new API provider from explicit credentials.
    pub fn new(
        working_dir: &Path,
        access_key_id: &str,
        secret_access_key: &str,
        region: &str,
        s3_bucket: &str,
    ) -> Self {
        let auth = AwsAuth::new(access_key_id, secret_access_key, region);
        let client = AwsLambdaApiClient::new(access_key_id, secret_access_key, region);
        Self {
            working_dir: working_dir.to_path_buf(),
            auth,
            client,
            s3_bucket: s3_bucket.to_string(),
        }
    }

    /// Create from environment variables.
    pub fn from_env(working_dir: &Path, s3_bucket: &str) -> Option<Self> {
        let auth = AwsAuth::from_env()?;
        let client = AwsLambdaApiClient::new(
            &auth.access_key_id,
            &auth.secret_access_key,
            &auth.region,
        );
        Some(Self {
            working_dir: working_dir.to_path_buf(),
            auth,
            client,
            s3_bucket: s3_bucket.to_string(),
        })
    }

    /// Detect AWS project by finding template.yaml.
    pub fn detect(project_dir: &Path) -> Option<SamTemplate> {
        let config_path = project_dir.join("template.yaml");
        SamTemplate::load(&config_path).ok()
    }
}

impl DeploymentProvider for AwsLambdaApiProvider {
    type Config = SamTemplate;
    type Resources = AwsApiResources;

    fn name(&self) -> &str {
        "aws-api"
    }

    fn detect(project_dir: &Path) -> Option<Self::Config> {
        Self::detect(project_dir)
    }

    fn validate(&self, config: &Self::Config) -> Result<(), DeploymentError> {
        config.validate()?;
        self.auth.validate().map_err(|e| {
            DeploymentError::Aws {
                status: 0,
                message: e.to_string(),
                request_id: None,
            }
        })
    }

    fn build(&self, config: &Self::Config, _env: Option<&str>) -> Result<BuildOutput, DeploymentError> {
        // Build Rust Lambda: cargo lambda build --release
        let has_cargo = self.working_dir.join("Cargo.toml").exists();
        
        if has_cargo {
            use crate::core::shell::{execute_and_collect, ShellExecutor};
            let output = execute_and_collect(
                ShellExecutor::new("cargo")
                    .args(["lambda", "build", "--release"])
                    .current_dir(&self.working_dir)
            )?;

            if !output.success {
                return Err(DeploymentError::BuildFailed(output.stderr));
            }
        }

        // Create deployment package (zip)
        let zip_path = self.create_deployment_package(config)?;

        Ok(BuildOutput {
            artifacts: vec![BuildArtifact {
                path: zip_path,
                size_bytes: 0,
                artifact_type: ArtifactType::ZipArchive,
            }],
            duration_ms: 0,
        })
    }

    fn deploy(
        &self,
        config: &Self::Config,
        env: Option<&str>,
        dry_run: bool,
    ) -> Result<DeploymentResult, DeploymentError> {
        if dry_run {
            return Ok(DeploymentResult::dry_run("aws-api", &config.stack_name()));
        }

        let function_name = match env {
            Some(e) => format!("{}-{}", config.stack_name(), e),
            None => config.stack_name(),
        };

        // Get the first function from template
        let functions = config.functions();
        if functions.is_empty() {
            return Err(DeploymentError::ConfigInvalid {
                file: "template.yaml".into(),
                reason: "no Lambda functions defined".into(),
            });
        }

        let (func_logical_id, _func_resource) = functions[0];

        // Read built artifact
        let zip_path = self.working_dir.join("target/lambda/bootstrap");
        let zip_content = std::fs::read(&zip_path)
            .map_err(|e| DeploymentError::IoError(e))?;

        // Upload to S3
        let s3_key = format!("deployments/{}/{}.zip", function_name, chrono::Utc::now().timestamp());
        
        use futures::executor::block_on;
        block_on(self.client.upload_to_s3(&self.s3_bucket, &s3_key, &zip_content))?;

        // Check if function exists
        match block_on(self.client.get_function(&function_name)) {
            Ok(_func) => {
                // Update existing function
                block_on(self.client.update_function_code(
                    &function_name,
                    &self.s3_bucket,
                    &s3_key,
                ))?;
            }
            Err(AwsApiError::NotFound(_)) => {
                // Create new function
                block_on(self.client.create_function(
                    &function_name,
                    "provided.al2023",
                    "bootstrap",
                    &self.get_role_arn_from_config(config),
                    &self.s3_bucket,
                    &s3_key,
                    self.extract_environment(config, env),
                ))?;
            }
            Err(e) => return Err(e.into()),
        }

        // Publish new version
        let version = block_on(self.client.publish_version(&function_name, None))?;

        // Update alias to point to new version
        let alias_name = env.unwrap_or("live");
        match block_on(self.client.get_alias(&function_name, alias_name)) {
            Ok(_alias) => {
                block_on(self.client.update_alias(&function_name, alias_name, &version.version))?;
            }
            Err(_) => {
                block_on(self.client.create_alias(&function_name, alias_name, &version.version))?;
            }
        }

        Ok(DeploymentResult {
            deployment_id: version.version,
            provider: "aws-api".to_string(),
            resource_name: function_name,
            environment: env.map(String::from),
            url: None,  // Lambda URL or API Gateway URL would be extracted here
            deployed_at: chrono::Utc::now(),
        })
    }

    fn rollback(
        &self,
        config: &Self::Config,
        env: Option<&str>,
        previous_state: Option<&Self::Resources>,
    ) -> Result<(), DeploymentError> {
        match previous_state {
            Some(prev) => {
                // Switch alias back to previous version
                let alias_name = env.unwrap_or("live");
                use futures::executor::block_on;
                block_on(self.client.update_alias(
                    &prev.function_name,
                    alias_name,
                    &prev.previous_version,
                ))?;
                Ok(())
            }
            None => self.destroy(config, env),
        }
    }

    fn logs(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError> {
        // Lambda logs are in CloudWatch Logs
        // For API-first approach, use CloudWatch Logs API
        Err(DeploymentError::ConfigInvalid {
            file: "logs".into(),
            reason: "Use CloudWatch Logs API or AWS Console for Lambda logs".into(),
        })
    }

    fn destroy(&self, config: &Self::Config, env: Option<&str>) -> Result<(), DeploymentError> {
        let function_name = match env {
            Some(e) => format!("{}-{}", config.stack_name(), e),
            None => config.stack_name(),
        };

        use futures::executor::block_on;
        block_on(self.client.delete_function(&function_name))?;
        Ok(())
    }

    fn status(&self, config: &Self::Config, env: Option<&str>) -> Result<Self::Resources, DeploymentError> {
        let function_name = match env {
            Some(e) => format!("{}-{}", config.stack_name(), e),
            None => config.stack_name(),
        };

        use futures::executor::block_on;
        let function = block_on(self.client.get_function(&function_name))?;

        Ok(AwsApiResources {
            function_name,
            version: function.version,
            runtime: function.runtime,
            memory_mb: function.memory_size,
            timeout_sec: function.timeout,
            last_modified: function.last_modified,
        })
    }

    fn verify(&self, result: &DeploymentResult) -> Result<bool, DeploymentError> {
        // Invoke Lambda function to verify it's responsive
        // Use Lambda Invoke API
        Ok(true)
    }
}
```

## Tasks

1. **Create module structure**
   - [ ] Create `src/providers/aws_api/mod.rs`, `client.rs`, `auth.rs`, `error.rs`, `sigv4.rs`, `resources.rs`
   - [ ] Register in `src/providers/mod.rs`
   - [ ] Add to feature flags (separate from CLI provider)

2. **Implement SigV4 signing**
   - [ ] Implement `SigV4Signer` struct
   - [ ] Implement canonical request creation
   - [ ] Implement string-to-sign creation
   - [ ] Implement HMAC-SHA256 signature calculation
   - [ ] Implement Authorization header construction
   - [ ] Handle temporary credentials (session token)

3. **Implement authentication**
   - [ ] Implement `AwsAuth` struct
   - [ ] Implement `from_env()` and `new()` constructors
   - [ ] Implement credential validation

4. **Implement API client**
   - [ ] Implement `AwsLambdaApiClient` with SigV4 signing
   - [ ] Implement Lambda Functions API (create, get, update, delete)
   - [ ] Implement update_function_code
   - [ ] Implement update_function_configuration
   - [ ] Implement Versions API (publish_version)
   - [ ] Implement Aliases API (create, get, update)
   - [ ] Implement S3 upload with SigV4 signing

5. **Implement error types**
   - [ ] Define `AwsApiError` enum
   - [ ] Implement `Display` and `Error` traits
   - [ ] Implement `From` conversions

6. **Implement AwsLambdaApiProvider trait**
   - [ ] Implement `detect()` - find template.yaml
   - [ ] Implement `validate()` - validate config + credentials
   - [ ] Implement `build()` - cargo lambda build + zip packaging
   - [ ] Implement `deploy()` - upload to S3, create/update function, manage versions/aliases
   - [ ] Implement `destroy()` - delete function
   - [ ] Implement `status()` - get function info
   - [ ] Implement `rollback()` - alias switch to previous version
   - [ ] Implement `verify()` - invoke function

7. **Implement S3 artifact upload**
   - [ ] Create deployment zip package
   - [ ] Upload to S3 with SigV4 signing
   - [ ] Generate unique S3 keys per deployment
   - [ ] Handle large file uploads

8. **Implement version and alias management**
   - [ ] Publish new versions after code update
   - [ ] Create/update aliases to point to versions
   - [ ] Handle alias not found for first deployment

9. **Write unit tests**
   - [ ] Test SigV4 signing with known test vectors
   - [ ] Test canonical request generation
   - [ ] Test API response parsing
   - [ ] Test error type conversions
   - [ ] Test config-to-function conversion

10. **Write integration tests**
    - [ ] Test function create/update (requires credentials, mark `#[ignore]`)
    - [ ] Test S3 upload (requires credentials, mark `#[ignore]`)
    - [ ] Test version/alias management (requires credentials, mark `#[ignore]`)
    - [ ] Test full deploy-verify-destroy cycle

11. **Documentation**
    - [ ] Document all public API methods
    - [ ] Add usage examples for API provider
    - [ ] Document required environment variables
    - [ ] Document IAM permissions required

## AWS API Endpoints Used

| Method | Endpoint | Purpose |
|--------|----------|---------|
| `POST` | `lambda.{region}.amazonaws.com/2015-03-31/functions` | Create function |
| `GET` | `lambda.{region}.amazonaws.com/2015-03-31/functions/{name}` | Get function |
| `PUT` | `lambda.{region}.amazonaws.com/2015-03-31/functions/{name}/code` | Update function code |
| `PUT` | `lambda.{region}.amazonaws.com/2015-03-31/functions/{name}/configuration` | Update configuration |
| `DELETE` | `lambda.{region}.amazonaws.com/2015-03-31/functions/{name}` | Delete function |
| `POST` | `lambda.{region}.amazonaws.com/2015-03-31/functions/{name}/versions` | Publish version |
| `POST` | `lambda.{region}.amazonaws.com/2015-03-31/functions/{name}/aliases` | Create alias |
| `PUT` | `lambda.{region}.amazonaws.com/2015-03-31/functions/{name}/aliases/{alias}` | Update alias |
| `PUT` | `s3.{region}.amazonaws.com/{bucket}/{key}` | Upload to S3 |

## Implementation Notes

### SigV4 Signing Reference

The AWS Signature Version 4 algorithm:
1. Create canonical request (method, URI, query, headers, signed headers, payload hash)
2. Create string to sign (algorithm, date, credential scope, canonical hash)
3. Calculate signature (HMAC-SHA256 chain: kSecret -> kDate -> kRegion -> kService -> kSigning -> signature)
4. Build Authorization header

### Testing SigV4 Implementation

AWS provides test vectors for SigV4:
https://github.com/aws/aws-sdk-java-v2/tree/master/aws-core/src/test/resources/software/amazon/awssdk/protocol/tests

### Lambda Runtimes

For Rust Lambda functions:
- Runtime: `provided.al2023`
- Handler: `bootstrap`
- Build with: `cargo lambda build --release --arm64`

### IAM Permissions Required

- `lambda:*` - Full Lambda access
- `s3:*` - S3 bucket access for code uploads
- `apigateway:*` - API Gateway access (optional)

### S3 Bucket

Users must create an S3 bucket beforehand or the provider can attempt to create one via API.

## Success Criteria

- [ ] All 11 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere
- [ ] SigV4 signing passes AWS test vectors
- [ ] Lambda function create/update/delete via API
- [ ] S3 upload with SigV4 signing works
- [ ] Version/alias management works correctly
- [ ] Error handling provides clear messages

## Verification

```bash
cd backends/foundation_deployment
cargo test aws_api -- --nocapture

# Integration (requires AWS credentials)
export AWS_ACCESS_KEY_ID="your_key"
export AWS_SECRET_ACCESS_KEY="your_secret"
export AWS_DEFAULT_REGION="us-east-1"
cargo test aws_api_integration -- --ignored --nocapture
```

---

_Created: 2026-04-06_
_Status: pending — API-first AWS Lambda provider implementation_
