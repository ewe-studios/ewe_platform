---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/07-provider-api-clients"
this_file: "specifications/11-foundation-deployment/features/07-provider-api-clients/feature.md"

status: pending
priority: high
created: 2026-04-11
updated: 2026-04-11

depends_on: ["01-foundation-deployment-core", "02-state-stores", "03-deployment-engine"]

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---


# Provider API Clients

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
> - `cargo test -p foundation_deployment` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Implement **API clients** for all supported cloud providers. These clients enable deployments without CLI dependencies.

**Design decision:** CLI vs API selection is **automatic** based on credential availability:
- API credentials present → API client
- No credentials → CLI fallback  
- Neither → Error with helpful message

This consolidates what was previously split across separate CLI and API provider features (07, 08, 09 in the old spec).

## Providers

| Provider | API Client Location | Auth Method | Status |
|----------|---------------------|-------------|--------|
| Cloudflare | `providers/cloudflare/clients/` | Bearer token | Generated from OpenAPI |
| GCP | `providers/gcp/clients/` | OAuth2/JWT | Generated from OpenAPI |
| AWS | `providers/aws/clients/` | SigV4 | Generated from OpenAPI |
| Fly.io | `providers/fly_io/clients/` | Bearer token | Generated from OpenAPI |
| HuggingFace | `providers/huggingface/` | Bearer token | **COMPLETE** |
| Stripe | `providers/stripe/clients/` | Bearer token | Generated from OpenAPI |
| Supabase | `providers/supabase/clients/` | Bearer token | Generated from OpenAPI |
| Neon | `providers/neon/clients/` | Bearer token | Generated from OpenAPI |
| PlanetScale | `providers/planetscale/clients/` | Bearer token | Generated from OpenAPI |
| MongoDB Atlas | `providers/mongodb_atlas/clients/` | Digest auth | Generated from OpenAPI |
| Prisma Postgres | `providers/prisma_postgres/clients/` | Bearer token | Generated from OpenAPI |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  Deployment Plan                             │
│  provider: gcp                                               │
│  credentials: auto | api | cli                               │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  Credential Resolver                         │
│  - Check for API credentials in env vars                     │
│  - Check for CLI availability                                │
│  - Select best available auth method                         │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                  Provider Client                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐          │
│  │ Cloudflare  │  │    GCP      │  │    AWS      │  ...     │
│  │   Client    │  │   Client    │  │   Client    │          │
│  └─────────────┘  └─────────────┘  └─────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

## Requirements

### Credential Resolver

```rust
// providers/credential_resolver.rs

/// Resolves the best available authentication method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    /// API credentials available - use API client
    Api,
    /// No API credentials - use CLI
    Cli,
    /// Neither available
    None,
}

pub struct CredentialResolver;

impl CredentialResolver {
    /// Check which auth method is available for a provider.
    pub fn check_available(provider: &str) -> AuthMethod;
}

/// Trait for types that can resolve credentials.
pub trait ResolveCredentials: Send + Sync {
    /// Resolve credentials for this provider.
    /// Returns `None` if no credentials available.
    fn resolve(&self) -> Option<Credentials>;
}

/// Resolved credentials ready for API calls.
#[derive(Debug, Clone)]
pub struct Credentials {
    /// API token, JWT, or signed key
    pub token: String,
    /// Account/project identifier
    pub account_id: Option<String>,
    /// Additional headers to include
    pub extra_headers: HashMap<String, String>,
}
```

### Cloudflare API Client

```rust
// providers/cloudflare/clients/mod.rs

use foundation_core::simple_http::client::SimpleHttpClient;

/// Cloudflare API client with Bearer token auth.
///
/// Required environment variables:
/// - `CLOUDFLARE_API_TOKEN` - API token
/// - `CLOUDFLARE_ACCOUNT_ID` - Account ID
#[derive(Debug, Clone)]
pub struct CloudflareClient {
    client: SimpleHttpClient,
    account_id: String,
}

impl CloudflareClient {
    /// Create from environment variables.
    pub fn from_env() -> Option<Self>;
    
    /// Upload a Worker script.
    pub async fn put_worker_script(
        &self,
        script_name: &str,
        script: &str,
        bindings: Option<Vec<WorkerBinding>>,
    ) -> Result<WorkerScript, CloudflareError>;
    
    /// Get a Worker script.
    pub async fn get_worker_script(
        &self,
        script_name: &str,
    ) -> Result<WorkerScript, CloudflareError>;
    
    /// Delete a Worker script.
    pub async fn delete_worker_script(
        &self,
        script_name: &str,
    ) -> Result<(), CloudflareError>;
    
    // ... additional methods for KV, D1, R2, Queues, etc.
}
```

### GCP Cloud Run Client

```rust
// providers/gcp/clients/mod.rs

use foundation_core::simple_http::client::SimpleHttpClient;

/// GCP Cloud Run API v2 client with OAuth2 JWT auth.
///
/// Required environment variables (one of):
/// - `GOOGLE_APPLICATION_CREDENTIALS` - Path to service account JSON
/// - Or explicit: `GCP_SA_EMAIL`, `GCP_SA_PRIVATE_KEY`, `GCP_PROJECT_ID`
#[derive(Debug, Clone)]
pub struct GcpClient {
    client: SimpleHttpClient,
    project_id: String,
}

impl GcpClient {
    /// Create from environment variables.
    pub fn from_env() -> Option<Self>;
    
    /// Deploy a Cloud Run service.
    pub async fn deploy_service(
        &self,
        service_name: &str,
        location: &str,
        image: &str,
        service_account: Option<String>,
    ) -> Result<CloudRunService, GcpError>;
    
    /// Get a Cloud Run service.
    pub async fn get_service(
        &self,
        service_name: &str,
        location: &str,
    ) -> Result<CloudRunService, GcpError>;
    
    /// Delete a Cloud Run service.
    pub async fn delete_service(
        &self,
        service_name: &str,
        location: &str,
    ) -> Result<(), GcpError>;
    
    // ... additional methods for Jobs, Revisions, etc.
}
```

### AWS Lambda Client

```rust
// providers/aws/clients/mod.rs

use foundation_core::simple_http::client::SimpleHttpClient;

/// AWS Lambda API client with SigV4 signing.
///
/// Required environment variables:
/// - `AWS_ACCESS_KEY_ID` - Access key ID
/// - `AWS_SECRET_ACCESS_KEY` - Secret access key
/// - `AWS_DEFAULT_REGION` - Region
#[derive(Debug, Clone)]
pub struct AwsClient {
    client: SimpleHttpClient,
    region: String,
    access_key_id: String,
    secret_access_key: String,
}

impl AwsClient {
    /// Create from environment variables.
    pub fn from_env() -> Option<Self>;
    
    /// Create or update a Lambda function.
    pub async fn create_or_update_function(
        &self,
        function_name: &str,
        runtime: &str,
        handler: &str,
        code: &[u8],  // Zip archive
    ) -> Result<LambdaFunction, AwsError>;
    
    /// Invoke a Lambda function.
    pub async fn invoke_function(
        &self,
        function_name: &str,
        payload: &str,
    ) -> Result<String, AwsError>;
    
    /// Delete a Lambda function.
    pub async fn delete_function(
        &self,
        function_name: &str,
    ) -> Result<(), AwsError>;
    
    // ... additional methods for versions, aliases, layers, etc.
}
```

### SigV4 Signing (AWS)

```rust
// providers/aws/sigv4.rs

use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use hmac::{Hmac, Mac};

type HmacSha256 = Hmac<Sha256>;

/// AWS Signature Version 4 implementation.
///
/// Reference: https://docs.aws.amazon.com/general/latest/gr/sigv4-signed-request-examples.html
pub struct SigV4Signer {
    access_key_id: String,
    secret_access_key: String,
    region: String,
    service: String,  // "lambda", "s3", etc.
}

impl SigV4Signer {
    pub fn new(access_key_id: &str, secret_access_key: &str, region: &str, service: &str) -> Self;
    
    /// Sign an HTTP request.
    pub fn sign(
        &self,
        method: &str,
        url: &str,
        headers: &mut HashMap<String, String>,
        body: &[u8],
    );
    
    /// Generate the authorization header.
    fn generate_authorization_header(
        &self,
        amz_date: DateTime<Utc>,
        credential_scope: &str,
        signature: &str,
    ) -> String;
    
    /// Create the string to sign.
    fn create_string_to_sign(
        &self,
        amz_date: DateTime<Utc>,
        credential_scope: &str,
        hashed_canonical_request: &str,
    ) -> String;
    
    /// Calculate the signature.
    fn calculate_signature(
        &self,
        string_to_sign: &str,
        credential_scope: &str,
    ) -> String;
}
```

## Tasks

1. **Credential resolver**
   - [ ] Implement `CredentialResolver` with `check_available()`
   - [ ] Implement `ResolveCredentials` trait
   - [ ] Add environment variable detection for all providers
   - [ ] Write unit tests

2. **Cloudflare client**
   - [ ] Generate client code from OpenAPI spec (already done via spec fetcher)
   - [ ] Implement `CloudflareClient` with Bearer token auth
   - [ ] Add methods for Workers, KV, D1, R2, Queues
   - [ ] Write integration tests (requires API credentials)

3. **GCP client**
   - [ ] Generate client code from OpenAPI spec (already done via spec fetcher)
   - [ ] Implement `GcpClient` with OAuth2 JWT auth
   - [ ] Add methods for Cloud Run Services and Jobs
   - [ ] Write integration tests (requires service account)

4. **AWS client**
   - [ ] Generate client code from OpenAPI spec (already done via spec fetcher)
   - [ ] Implement `AwsClient` with SigV4 signing
   - [ ] Add methods for Lambda, S3, API Gateway
   - [ ] Write integration tests (requires AWS credentials)

5. **Other provider clients**
   - [ ] Fly.io - Bearer token auth
   - [ ] Stripe - Bearer token auth
   - [ ] Supabase - Bearer token auth
   - [ ] Neon - Bearer token auth
   - [ ] PlanetScale - Bearer token auth
   - [ ] MongoDB Atlas - Digest auth

6. **Auto-selection logic**
   - [ ] Integrate credential resolver with `DeploymentProvider` implementations
   - [ ] Add fallback from API to CLI when credentials missing
   - [ ] Write tests for auto-selection

7. **Error handling**
   - [ ] Define provider-specific error types
   - [ ] Add `From` conversions for `HttpClientError`
   - [ ] Implement retry logic for transient errors

8. **Documentation**
   - [ ] Document required environment variables per provider
   - [ ] Document IAM permissions required per provider
   - [ ] Add usage examples

## Success Criteria

- [ ] All 8 tasks completed
- [ ] `cargo clippy -p foundation_deployment -- -D warnings -W clippy::pedantic` — zero warnings
- [ ] `cargo doc -p foundation_deployment --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere
- [ ] Credential resolver correctly detects all auth methods
- [ ] Cloudflare client can deploy workers via API
- [ ] GCP client can deploy Cloud Run services via API
- [ ] AWS client can deploy Lambda functions via API (SigV4 signing works)
- [ ] Auto-selection picks API when credentials available, CLI otherwise

## Verification

```bash
cd backends/foundation_deployment
cargo check
cargo clippy -- -D warnings
cargo test providers -- --nocapture
```

---

_Created: 2026-04-11 (consolidated from features 07, 08, 09)_

