---
workspace_name: "ewe_platform"
spec_directory: "specifications/11-foundation-deployment"
feature_directory: "specifications/11-foundation-deployment/features/10-provider-spec-fetcher-core"
this_file: "specifications/11-foundation-deployment/features/10-provider-spec-fetcher-core/feature.md"

status: pending
priority: high
created: 2026-03-27

depends_on: ["01-foundation-deployment-core"]

tasks:
  completed: 0
  uncompleted: 8
  total: 8
  completion_percentage: 0%
---


# Provider Spec Fetcher Core

## Iron Law: Zero Warnings

> **All code must compile with zero warnings and pass all lints. No suppression. No exceptions.**
>
> - `cargo clippy -p ewe_platform -- -D warnings -W clippy::pedantic` — zero warnings
> - `cargo doc -p ewe_platform --no-deps` — zero rustdoc warnings
> - `cargo test -p ewe_platform` — zero compilation warnings
> - **No `#[allow(...)]`, `#[expect(...)]`, or `#![allow(...)]` anywhere.** Fix the code, never suppress.

## Overview

Build a Valtron-based CLI command that fetches OpenAPI specifications from multiple providers and distills them into versioned JSON snapshots. This is similar to `gen_model_descriptors` but targets deployment provider APIs instead of AI model catalogs.

The fetcher:
- **Fetches specs in parallel** using `SimpleHttpClient` and Valtron's `TaskIterator`
- **Normalizes responses** into a common `DistilledSpec` format
- **Commits to git submodules** each `distilled-spec-*` repository is updated with new specs
- **Tracks versions** each fetch produces a timestamped, versioned snapshot
- **Detects changes** compares new specs against existing to avoid unnecessary commits

This feature creates the **core infrastructure** that individual provider fetchers (Features 11-18) will use.

## Dependencies

Depends on:
- `foundation_core::wire::simple_http::client::SimpleHttpClient` - HTTP client for fetching specs
- `foundation_core::valtron` - TaskIterator for parallel async execution
- `serde_json` - JSON parsing and serialization

Required by:
- `11-fetch-flyio-spec` - Fly.io OpenAPI fetcher
- `12-fetch-planetscale-spec` - PlanetScale OpenAPI fetcher
- `13-fetch-cloudflare-spec` - Cloudflare OpenAPI fetcher
- `14-fetch-gcp-spec` - GCP Discovery Service fetcher
- `15-fetch-prisma-postgres-spec` - Prisma Postgres fetcher
- `16-fetch-supabase-spec` - Supabase fetcher
- `17-fetch-mongodb-atlas-spec` - MongoDB Atlas fetcher
- `18-fetch-neon-spec` - Neon fetcher
- `19-fetch-stripe-spec` - Stripe API fetcher

## Valtron Integration Pattern

This follows the same pattern as `bin/platform/src/gen_model_descriptors/mod.rs`:

```rust
use foundation_core::valtron::{TaskIterator, TaskIteratorExt};
use foundation_core::wire::simple_http::client::SimpleHttpClient;

// Parallel fetch with progress tracking
let specs = provider_urls
    .into_task_iter()
    .map_with_progress(|(name, url)| async move {
        let client = SimpleHttpClient::new()?;
        let response = client.get(url).send().await?;
        let spec = parse_spec(response.body).await?;
        Ok((name, spec))
    })
    .buffered(10) // Fetch up to 10 specs in parallel
    .collect::<Vec<_>>()
    .await;
```

## Requirements

### Module Structure

```text
bin/platform/src/gen_provider_specs/
├── mod.rs           # Module root, CLI registration
├── errors.rs        # ALL error types defined here (central source of truth)
├── core.rs          # Core fetcher logic, types, traits
├── fetcher.rs       # ProviderSpecFetcher implementation
└── providers/       # Provider-specific implementations
    ├── mod.rs       # Provider module registry
    ├── fly_io.rs    # Fly.io fetcher
    ├── planetscale.rs
    ├── cloudflare.rs
    ├── gcp.rs
    ├── prisma_postgres.rs
    ├── supabase.rs
    ├── mongodb_atlas.rs
    ├── neon.rs
    └── stripe.rs
```

### Error Handling Pattern

**ALL errors are defined in `errors.rs` at the module root.** This is the single source of truth for error types.

```rust
// bin/platform/src/gen_provider_specs/errors.rs

use derive_more::{Display, From};

/// WHY: Centralized error type for all provider spec fetching operations.
///
/// WHAT: Covers HTTP transport, JSON parsing, file I/O, and git operations.
///
/// HOW: Uses `derive_more::From` for automatic conversions and
/// `derive_more::Display` for formatted error messages.
///
/// # Location
///
/// All errors MUST be defined in this file (`errors.rs`). Do NOT define
/// error types in provider-specific modules or other files.
#[derive(Debug, From, Display)]
pub enum SpecFetchError {
    /// HTTP transport error - wraps `SimpleHttpClientError` automatically.
    #[display("HTTP error for {provider}: {source}")]
    Http {
        provider: String,
        source: foundation_core::wire::simple_http::client::SimpleHttpClientError,
    },

    /// Server returned non-200 status.
    #[display("HTTP {status} from {provider}")]
    BadStatus { provider: String, status: u16 },

    /// JSON deserialization failed - wraps `serde_json::Error` automatically.
    #[display("JSON parse error for {provider}: {source}")]
    Json { provider: String, source: serde_json::Error },

    /// SHA256 hash computation failed.
    #[display("SHA256 hash error for {provider}: {source}")]
    Hash { provider: String, source: std::io::Error },

    /// Failed to write file to disk.
    #[display("failed to write {path}: {source}")]
    WriteFile { path: String, source: std::io::Error },

    /// Git operation failed (clone, pull, commit, etc.).
    #[display("git operation failed for {repo}: {reason}")]
    Git { repo: String, reason: String },
}

impl std::error::Error for SpecFetchError {}
```

**Key Points:**
- `#[derive(From)]` automatically generates `From<T>` implementations for each field type
- `#[derive(Display)]` generates `Display` from the `#[display(...)]` attributes
- No manual `impl From<>` or `impl Display` needed
- All provider-specific code imports from `errors.rs`: `use crate::errors::SpecFetchError;`

### Core Fetcher Infrastructure

```rust
// bin/platform/src/gen_provider_specs/mod.rs

use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_core::valtron::{TaskIterator, TaskIteratorExt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// WHY: Tracks progress of individual spec fetches during parallel execution.
///
/// WHAT: Progress states with source identification for observability.
///
/// HOW: Used as the `Pending` type in `TaskIterator` combinators.
#[derive(Debug, Clone)]
pub enum SpecFetchPending {
    Connecting { provider: &'static str },
    AwaitingResponse { provider: &'static str },
    Parsing { provider: &'static str },
}

impl std::fmt::Display for SpecFetchPending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecFetchPending::Connecting { provider } => {
                write!(f, "{provider}: Connecting...")
            }
            SpecFetchPending::AwaitingResponse { provider } => {
                write!(f, "{provider}: Awaiting response...")
            }
            SpecFetchPending::Parsing { provider } => {
                write!(f, "{provider}: Parsing JSON...")
            }
        }
    }
}

/// WHY: Unified representation of a distilled OpenAPI spec.
///
/// WHAT: Common metadata extracted from any provider's spec format.
///
/// HOW: Normalized format that all providers can be converted to.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DistilledSpec {
    /// Provider name (e.g., "fly-io", "gcp", "neon")
    pub provider: String,

    /// Spec version or timestamp
    pub version: String,

    /// Fetched at timestamp
    pub fetched_at: chrono::DateTime<chrono::Utc>,

    /// Source URL
    pub source_url: String,

    /// Raw OpenAPI spec (preserved as-is)
    pub raw_spec: serde_json::Value,

    /// Extracted API endpoints (optional, provider-specific)
    pub endpoints: Option<Vec<SpecEndpoint>>,

    /// Change detection hash
    pub content_hash: String,
}

/// WHY: Represents a single API endpoint extracted from a spec.
///
/// WHAT: Normalized endpoint info for cross-provider comparisons.
///
/// HOW: Extracted from OpenAPI paths during distillation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecEndpoint {
    pub path: String,
    pub methods: Vec<String>,
    pub operation_id: Option<String>,
    pub summary: Option<String>,
}

/// WHY: Result of a spec fetch operation.
///
/// WHAT: Contains the distilled spec and metadata about the fetch.
///
/// HOW: Returned by individual fetch tasks.
pub type FetchResult = Result<DistilledSpec, SpecFetchError>;

/// WHY: Error types for spec fetching.
///
/// WHAT: Covers HTTP, JSON parsing, and I/O errors.
///
/// HOW: Uses `derive_more` for `Display`/`From`. All errors are defined in
/// `bin/platform/src/gen_provider_specs/errors.rs` - the central error module
/// for the provider spec fetcher. Use `#[derive(From, Display)]` to automatically
/// implement `From` conversions and `Display` formatting.
///
/// # Location
///
/// All error types MUST be defined in `errors.rs` at the root of the module:
/// ```text
/// bin/platform/src/gen_provider_specs/
/// ├── errors.rs      <- All error types defined here
/// ├── mod.rs         <- Module root
/// ├── core.rs        <- Core fetcher logic
/// └── providers/     <- Provider-specific implementations
/// ```
#[derive(Debug, derive_more::From, derive_more::Display)]
pub enum SpecFetchError {
    /// HTTP transport error - wraps `SimpleHttpClientError` automatically via `#[from]`.
    #[display("HTTP error for {provider}: {source}")]
    Http {
        provider: String,
        source: foundation_core::wire::simple_http::client::SimpleHttpClientError,
    },

    /// Server returned non-200 status.
    #[display("HTTP {status} from {provider}")]
    BadStatus {
        provider: String,
        status: u16,
    },

    /// JSON deserialization failed - wraps `serde_json::Error` automatically.
    #[display("JSON parse error for {provider}: {source}")]
    Json {
        provider: String,
        source: serde_json::Error,
    },

    /// SHA256 hash computation failed.
    #[display("SHA256 hash error for {provider}: {source}")]
    Hash {
        provider: String,
        source: std::io::Error,
    },

    /// Failed to write file to disk.
    #[display("failed to write {path}: {source}")]
    WriteFile {
        path: String,
        source: std::io::Error,
    },

    /// Git operation failed (clone, pull, commit, etc.).
    #[display("git operation failed for {repo}: {reason}")]
    Git {
        repo: String,
        reason: String,
    },
}

impl std::error::Error for SpecFetchError {}

// ---------------------------------------------------------------------------
// Error Definition Pattern
// ---------------------------------------------------------------------------
//
// ALL error types are defined in a central `errors.rs` file at the module root.
// This ensures:
// 1. Single source of truth for all error variants
// 2. Consistent error handling across all provider implementations
// 3. Easy to extend with new error types
// 4. Automatic `From` implementations via `#[derive(From)]`
//
// Example structure:
//
// ```rust
// // bin/platform/src/gen_provider_specs/errors.rs
//
// use derive_more::{Display, From};
// use foundation_core::wire::simple_http::client::SimpleHttpClientError;
//
// /// WHY: Centralized error type for all provider spec fetching operations.
// ///
// /// WHAT: Covers HTTP transport, JSON parsing, file I/O, and git operations.
// ///
// /// HOW: Uses `derive_more::From` for automatic conversions and
// /// `derive_more::Display` for formatted error messages.
// #[derive(Debug, From, Display)]
// pub enum SpecFetchError {
//     #[display("HTTP error for {provider}: {source}")]
//     Http {
//         provider: String,
//         source: SimpleHttpClientError,
//     },
//
//     #[display("HTTP {status} from {provider}")]
//     BadStatus { provider: String, status: u16 },
//
//     #[display("JSON parse error for {provider}: {source}")]
//     Json { provider: String, source: serde_json::Error },
//
//     #[display("failed to write {path}: {source}")]
//     WriteFile { path: String, source: std::io::Error },
//
//     #[display("git operation failed for {repo}: {reason}")]
//     Git { repo: String, reason: String },
// }
//
// impl std::error::Error for SpecFetchError {}
// ```

/// WHY: Orchestrates fetching specs from multiple providers.
///
/// WHAT: Central coordinator that runs all fetches in parallel.
///
/// HOW: Uses Valtron's TaskIterator for concurrent execution.
pub struct ProviderSpecFetcher {
    /// Base directory for distilled-spec repos
    specs_base: PathBuf,

    /// HTTP client (shared across fetches)
    client: SimpleHttpClient,
}

impl ProviderSpecFetcher {
    pub fn new(specs_base: PathBuf) -> Self {
        Self {
            specs_base,
            client: SimpleHttpClient::default(),
        }
    }

    /// Fetch specs from all configured providers in parallel.
    pub async fn fetch_all(&self) -> Result<BTreeMap<String, DistilledSpec>, SpecFetchError> {
        let providers = self.configured_providers();

        let results = providers
            .into_iter()
            .into_task_iter()
            .map_with_progress(|(name, url)| self.fetch_single_spec(name, url))
            .buffered(5) // Fetch up to 5 providers in parallel
            .collect::<Vec<_>>()
            .await;

        // Collect successful results
        let mut specs = BTreeMap::new();
        for result in results {
            match result {
                Ok(spec) => {
                    let provider = spec.provider.clone();
                    specs.insert(provider, spec);
                }
                Err(e) => {
                    // Log but don't fail entire operation
                    eprintln!("Warning: {e}");
                }
            }
        }

        Ok(specs)
    }

    /// Fetch a single provider's spec.
    async fn fetch_single_spec(
        &self,
        provider: &'static str,
        url: &'static str,
    ) -> FetchResult {
        // 1. Fetch the spec
        let response = self.client.get(url)
            .send()
            .await
            .map_err(|e| SpecFetchError::Http { provider: provider.to_string(), source: e })?;

        // 2. Parse as JSON
        let raw_spec: serde_json::Value = serde_json::from_str(&response.body)
            .map_err(|e| SpecFetchError::Json { provider: provider.to_string(), source: e })?;

        // 3. Compute content hash for change detection
        let content_hash = compute_sha256(&raw_spec.to_string());

        // 4. Extract endpoints (provider-specific logic)
        let endpoints = extract_endpoints(&raw_spec, provider);

        // 5. Determine version from spec or timestamp
        let version = extract_version(&raw_spec, provider)
            .unwrap_or_else(|| chrono::Utc::now().format("%Y%m%d").to_string());

        Ok(DistilledSpec {
            provider: provider.to_string(),
            version,
            fetched_at: chrono::Utc::now(),
            source_url: url.to_string(),
            raw_spec,
            endpoints,
            content_hash,
        })
    }

    /// Write a distilled spec to its repository directory.
    pub async fn write_spec(&self, spec: &DistilledSpec, repo_name: &str) -> Result<PathBuf, SpecFetchError> {
        let repo_path = self.specs_base.join(repo_name);
        let specs_dir = repo_path.join("specs");

        // Ensure directory exists
        tokio::fs::create_dir_all(&specs_dir).await
            .map_err(|e| SpecFetchError::WriteFile {
                path: specs_dir.display().to_string(),
                source: e,
            })?;

        // Write the spec
        let filename = format!("openapi-{}.json", spec.version);
        let spec_path = specs_dir.join(&filename);

        let json = serde_json::to_string_pretty(&spec.raw_spec)
            .map_err(|e| SpecFetchError::Json {
                provider: spec.provider.clone(),
                source: e,
            })?;

        tokio::fs::write(&spec_path, json).await
            .map_err(|e| SpecFetchError::WriteFile {
                path: spec_path.display().to_string(),
                source: e,
            })?;

        // Write manifest
        self.write_manifest(&specs_dir, spec).await?;

        Ok(spec_path)
    }

    /// Check if spec has changed from previous fetch.
    pub async fn has_changed(&self, repo_name: &str, new_hash: &str) -> Result<bool, SpecFetchError> {
        let manifest_path = self.specs_base.join(repo_name).join("specs").join("_manifest.json");

        match tokio::fs::read_to_string(&manifest_path).await {
            Ok(content) => {
                let manifest: serde_json::Value = serde_json::from_str(&content)
                    .map_err(|e| SpecFetchError::Json {
                        provider: repo_name.to_string(),
                        source: e,
                    })?;

                let old_hash = manifest.get("content_hash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                Ok(old_hash != new_hash)
            }
            Err(_) => Ok(true), // No previous spec, consider it changed
        }
    }

    fn configured_providers(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("fly-io", "https://docs.machines.dev/spec/openapi3.json"),
            ("planetscale", "https://api.planetscale.com/v1/openapi-spec"),
            ("cloudflare", "https://github.com/cloudflare/api-schemas"),
            ("gcp", "https://discovery.googleapis.com/discovery/v1/apis"),
            ("prisma-postgres", "https://api.prisma.io/v1/doc"),
            ("supabase", "https://api.supabase.com/api/v1-json"),
            ("mongodb-atlas", "https://www.mongodb.com/docs/api/doc/atlas-admin-api-v2.json"),
            ("neon", "https://neon.com/api_spec/release/v2.json"),
            ("stripe", "https://docs.stripe.com/api"),
        ]
    }

    async fn write_manifest(
        &self,
        specs_dir: &Path,
        spec: &DistilledSpec,
    ) -> Result<(), SpecFetchError> {
        let manifest_path = specs_dir.join("_manifest.json");

        let manifest = serde_json::json!({
            "provider": spec.provider,
            "version": spec.version,
            "fetched_at": spec.fetched_at.to_rfc3339(),
            "source_url": spec.source_url,
            "content_hash": spec.content_hash,
            "endpoint_count": spec.endpoints.as_ref().map(|e| e.len()).unwrap_or(0),
        });

        let json = serde_json::to_string_pretty(&manifest)?;
        tokio::fs::write(manifest_path, json).await?;

        Ok(())
    }
}

fn compute_sha256(content: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn extract_endpoints(spec: &serde_json::Value, provider: &str) -> Option<Vec<SpecEndpoint>> {
    // Provider-specific extraction logic
    match provider {
        "gcp" => extract_gcp_endpoints(spec),
        "stripe" => extract_stripe_endpoints(spec),
        _ => None, // Single-spec providers don't need extraction
    }
}

fn extract_gcp_endpoints(spec: &serde_json::Value) -> Option<Vec<SpecEndpoint>> {
    spec.get("items")
        .and_then(|items| items.as_array())
        .map(|apis| {
            apis.iter()
                .filter_map(|api| {
                    Some(SpecEndpoint {
                        path: api.get("name")?.as_str()?.to_string(),
                        methods: vec!["GET".to_string()],
                        operation_id: api.get("id").and_then(|v| v.as_str()).map(String::from),
                        summary: api.get("title").and_then(|v| v.as_str()).map(String::from),
                    })
                })
                .collect()
        })
}

fn extract_stripe_endpoints(spec: &serde_json::Value) -> Option<Vec<SpecEndpoint>> {
    // Stripe uses a different format - extract from paths
    spec.get("paths")
        .and_then(|paths| paths.as_object())
        .map(|paths_obj| {
            paths_obj
                .keys()
                .map(|path| SpecEndpoint {
                    path: path.clone(),
                    methods: vec![], // Would need deeper traversal
                    operation_id: None,
                    summary: None,
                })
                .collect()
        })
}

fn extract_version(spec: &serde_json::Value, provider: &str) -> Option<String> {
    match provider {
        "neon" => spec.get("info").and_then(|i| i.get("version"))
            .and_then(|v| v.as_str()).map(String::from),
        "fly-io" => spec.get("info").and_then(|i| i.get("version"))
            .and_then(|v| v.as_str()).map(String::from),
        _ => spec.get("info").and_then(|i| i.get("version"))
            .and_then(|v| v.as_str()).map(String::from),
    }
}
```

### CLI Command Registration

```rust
// bin/platform/src/gen_provider_specs/mod.rs (continued)

use clap::{ArgMatches, Command};

pub fn register(cmd: Command) -> Command {
    cmd.subcommand(
        Command::new("gen_provider_specs")
            .about("Fetch and distill OpenAPI specs from deployment providers")
            .arg(
                clap::Arg::new("provider")
                    .long("provider")
                    .short('p')
                    .help("Fetch only this provider's spec (default: all)")
                    .value_name("PROVIDER"),
            )
            .arg(
                clap::Arg::new("output")
                    .long("output")
                    .short('o')
                    .help("Base directory for distilled-spec repos")
                    .default_value("../../@formulas/src.rust/src.deployAnywhere")
                    .value_name("DIR"),
            )
            .arg(
                clap::Arg::new("dry-run")
                    .long("dry-run")
                    .help("Fetch specs but don't write to disk"),
            )
            .arg(
                clap::Arg::new("force")
                    .long("force")
                    .help("Write specs even if content hasn't changed"),
            ),
    )
}

pub fn run(matches: &ArgMatches) -> Result<(), SpecFetchError> {
    let specs_base: PathBuf = matches
        .get_one::<String>("output")
        .map(PathBuf::from)
        .unwrap();

    let fetcher = ProviderSpecFetcher::new(specs_base);

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        if let Some(provider) = matches.get_one::<String>("provider") {
            // Fetch single provider
            fetch_single_provider(&fetcher, provider).await
        } else {
            // Fetch all providers
            fetch_all_providers(&fetcher, matches).await
        }
    })
}

async fn fetch_all_providers(
    fetcher: &ProviderSpecFetcher,
    matches: &ArgMatches,
) -> Result<(), SpecFetchError> {
    let dry_run = matches.get_flag("dry-run");
    let force = matches.get_flag("force");

    let specs = fetcher.fetch_all().await?;

    for (provider, spec) in specs {
        let repo_name = format!("distilled-spec-{provider}");

        if !dry_run {
            // Check for changes
            if force || fetcher.has_changed(&repo_name, &spec.content_hash).await? {
                fetcher.write_spec(&spec, &repo_name).await?;
                println!("Updated: {provider} -> {repo_name}/specs/");
            } else {
                println!("Unchanged: {provider}");
            }
        } else {
            println!("Would update: {provider} (dry-run)");
        }
    }

    Ok(())
}

async fn fetch_single_provider(
    fetcher: &ProviderSpecFetcher,
    provider: &str,
) -> Result<(), SpecFetchError> {
    let url = fetcher
        .configured_providers()
        .iter()
        .find(|(name, _)| *name == provider)
        .map(|(_, url)| *url)
        .ok_or_else(|| SpecFetchError::Http {
            provider: provider.to_string(),
            source: foundation_core::wire::simple_http::client::SimpleHttpClientError::InvalidUrl(
                format!("Unknown provider: {provider}"),
            ),
        })?;

    let spec = fetcher.fetch_single_spec(provider, url).await?;
    let repo_name = format!("distilled-spec-{provider}");

    fetcher.write_spec(&spec, &repo_name).await?;
    println!("Updated: {provider} -> {repo_name}/specs/");

    Ok(())
}
```

## Tasks

1. **Create module structure**
   - [ ] Create `bin/platform/src/gen_provider_specs/mod.rs`
   - [ ] Create `bin/platform/src/gen_provider_specs/errors.rs` - **central error definitions**
   - [ ] Create `bin/platform/src/gen_provider_specs/core.rs` - core types
   - [ ] Create `bin/platform/src/gen_provider_specs/fetcher.rs` - fetcher logic
   - [ ] Create `bin/platform/src/gen_provider_specs/providers/mod.rs`
   - [ ] Register in `bin/platform/src/main.rs`
   - [ ] Add necessary dependencies to `bin/platform/Cargo.toml`

2. **Implement error types (errors.rs)**
   - [ ] Create `SpecFetchError` enum with `#[derive(From, Display)]`
   - [ ] Define all error variants: `Http`, `BadStatus`, `Json`, `Hash`, `WriteFile`, `Git`
   - [ ] Import in all provider modules: `use crate::errors::SpecFetchError;`

3. **Implement core types (core.rs)**
   - [ ] Define `DistilledSpec` struct
   - [ ] Define `SpecEndpoint` struct
   - [ ] Define `SpecFetchPending` enum with `Display`
   - [ ] Define `FetchResult` type alias

3. **Implement ProviderSpecFetcher**
   - [ ] Implement `new()`, `configured_providers()`
   - [ ] Implement `fetch_all()` with Valtron parallelism
   - [ ] Implement `fetch_single_spec()` HTTP + JSON parsing
   - [ ] Implement `compute_sha256()` for change detection

4. **Implement endpoint extraction**
   - [ ] Implement `extract_endpoints()` dispatcher
   - [ ] Implement `extract_gcp_endpoints()` for GCP directory
   - [ ] Implement `extract_stripe_endpoints()` for Stripe paths
   - [ ] Handle provider-specific spec formats

5. **Implement file I/O**
   - [ ] Implement `write_spec()` to save JSON files
   - [ ] Implement `write_manifest()` for metadata
   - [ ] Implement `has_changed()` for change detection
   - [ ] Handle versioned filenames

6. **Implement CLI interface**
   - [ ] Implement `register()` for clap
   - [ ] Implement `run()` entry point
   - [ ] Support `--provider` flag for single-provider fetch
   - [ ] Support `--output` flag for custom output dir
   - [ ] Support `--dry-run` and `--force` flags

7. **Add progress reporting**
   - [ ] Implement `Display` for `SpecFetchPending`
   - [ ] Use `map_with_progress()` in TaskIterator
   - [ ] Show real-time fetch progress (like gen_model_descriptors)

8. **Write tests**
   - [ ] Test JSON parsing with sample specs
   - [ ] Test endpoint extraction
   - [ ] Test change detection logic
   - [ ] Test CLI argument parsing

## Provider URLs

| Provider | Spec URL | Format |
|----------|----------|--------|
| FlyIO | `https://docs.machines.dev/spec/openapi3.json` | OpenAPI 3 JSON |
| PlanetScale | `https://api.planetscale.com/v1/openapi-spec` | OpenAPI JSON |
| Cloudflare | `https://github.com/cloudflare/api-schemas` | GitHub repo (special handling) |
| GCP | `https://discovery.googleapis.com/discovery/v1/apis` | Discovery directory |
| Prisma Postgres | `https://api.prisma.io/v1/doc` | OpenAPI JSON |
| Supabase | `https://api.supabase.com/api/v1-json` | OpenAPI JSON |
| MongoDB Atlas | `https://www.mongodb.com/docs/api/doc/atlas-admin-api-v2.json` | OpenAPI JSON |
| Neon | `https://neon.com/api_spec/release/v2.json` | OpenAPI JSON |
| Stripe | `https://docs.stripe.com/api` | OpenAPI JSON |

**Note on Cloudflare**: The Cloudflare "spec" is a GitHub repo, not a direct JSON URL. This may require:
- Cloning the repo and extracting schemas
- Or finding a direct JSON endpoint if available

**Note on GCP**: Returns a directory of APIs, not a single spec. Requires special handling to fetch all `discoveryRestUrl` entries (see `distilled-spec-gcp/.meta/fetch-specs.ts`).

## Success Criteria

- [ ] All 8 tasks completed
- [ ] `cargo clippy -p ewe_platform -- -D warnings -W clippy::pedantic` — zero warnings
- [ ] `cargo doc -p ewe_platform --no-deps` — zero rustdoc warnings
- [ ] No `#[allow(...)]` or `#[expect(...)]` anywhere
- [ ] `ewe_platform gen_provider_specs` fetches all 9 providers
- [ ] Specs are written to correct `distilled-spec-*` directories
- [ ] Change detection works (unchanged specs skip write)
- [ ] Progress is displayed during parallel fetch
- [ ] `--dry-run` shows what would be updated without writing
- [ ] `--provider fly-io` fetches only Fly.io spec

## Verification

```bash
cd bin/platform

# Dry run - see what would be fetched
cargo run -- gen_provider_specs --dry-run

# Fetch all providers
cargo run -- gen_provider_specs

# Fetch single provider
cargo run -- gen_provider_specs --provider neon

# Verify output directories
ls ../../@formulas/src.rust/src.deployAnywhere/distilled-spec-*/specs/
```

---

_Created: 2026-03-27_
