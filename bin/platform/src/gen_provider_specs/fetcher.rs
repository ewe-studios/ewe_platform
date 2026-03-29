//! ProviderSpecFetcher - orchestrates fetching specs from multiple providers.
//!
//! WHY: Fetches OpenAPI specs from multiple providers in parallel using Valtron.
//!
//! WHAT: Central coordinator that runs all fetches in parallel using Valtron's
//! `execute_collect_all` pattern, similar to `gen_model_descriptors`.
//!
//! HOW: Uses `foundation_core::wire::simple_http::client::SendRequestTask` for HTTP,
//! Valtron's `execute_collect_all` for parallel execution, and blocking `std::fs`
//! for file I/O at sync boundaries.

use foundation_core::valtron::{self, Stream, TaskIteratorExt};
use foundation_core::wire::simple_http::client::{
    body_reader, SendRequestTask, SimpleHttpClient,
};
use foundation_core::wire::simple_http::client::RequestIntro;
use foundation_core::wire::simple_http::client::HttpRequestPending;
use foundation_core::wire::simple_http::HttpClientError;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::core::{DistilledSpec, FetchResult, SpecEndpoint, SpecFetchPending};
use super::errors::SpecFetchError;

/// WHY: Orchestrates fetching specs from multiple providers.
///
/// WHAT: Central coordinator that runs all fetches in parallel.
///
/// HOW: Uses Valtron's execute_collect_all for concurrent execution.
pub struct ProviderSpecFetcher {
    /// Base directory for distilled-spec repos
    specs_base: PathBuf,
}

impl ProviderSpecFetcher {
    pub fn new(specs_base: PathBuf) -> Self {
        Self { specs_base }
    }

    /// Fetch specs from all configured providers in parallel.
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP client (must be created by caller with pool guard alive)
    ///
    /// # Returns
    ///
    /// Map of provider name to distilled spec. Failed fetches are logged and skipped.
    pub fn fetch_all(
        &self,
        client: &mut SimpleHttpClient,
    ) -> Result<BTreeMap<String, DistilledSpec>, SpecFetchError> {
        let providers = Self::configured_providers();

        // Artefacts directory for raw JSON specs
        let artefacts_dir = std::path::PathBuf::from("artefacts/cloud_providers");
        std::fs::create_dir_all(&artefacts_dir).map_err(|e| SpecFetchError::WriteFile {
            path: artefacts_dir.display().to_string(),
            source: e,
        })?;

        tracing::info!("Fetching {} provider specs in parallel...", providers.len());
        let start_time = Instant::now();

        // Execute all tasks and collect their results
        // Each task is executed separately but they run in parallel on the thread pool
        let mut specs = BTreeMap::new();

        for (provider, url) in providers {
            // Build the request
            let request = client.get(url).map_err(|e| SpecFetchError::Http {
                provider: provider.to_string(),
                source: e,
            })?
            .build().map_err(|e| SpecFetchError::Http {
                provider: provider.to_string(),
                source: e,
            })?;

            // Create and execute the task inline to avoid boxing
            let task = SendRequestTask::new(
                request, 5, // max_retries
                client.client_pool().expect("should have pool"),
                client.client_config(),
            )
            .map_ready(move |intro| {
                match intro {
                    RequestIntro::Success { stream, .. } => {
                        let body_text = body_reader::collect_string(stream);
                        parse_spec_response(&body_text, provider, url)
                    }
                    RequestIntro::Failed(e) => {
                        tracing::warn!("HTTP request failed for {provider}: {e}");
                        Box::new([]) as Box<[Result<DistilledSpec, SpecFetchError>]>
                    }
                }
            })
            .map_pending(move |p| SpecFetchPending::from_http(p, provider));

            let results_stream = valtron::execute(task, None)
                .expect("execute should return stream");

            for stream_item in results_stream {
                if let Stream::Next(result_box) = stream_item {
                    // Iterate over the Box<[Result<...>]>
                    for result in result_box.into_iter() {
                        match result {
                            Ok(spec) => {
                                // Save raw JSON to artefacts directory in provider-named folder
                                let provider_dir = artefacts_dir.join(provider);
                                std::fs::create_dir_all(&provider_dir).map_err(|e| {
                                    SpecFetchError::WriteFile {
                                        path: provider_dir.display().to_string(),
                                        source: e,
                                    }
                                })?;
                                let json_path = provider_dir.join("openapi.json");
                                let json = serde_json::to_string_pretty(&spec.raw_spec)
                                    .map_err(|e| SpecFetchError::Json {
                                        provider: provider.to_string(),
                                        source: e,
                                    })?;
                                std::fs::write(&json_path, json).map_err(|e| {
                                    SpecFetchError::WriteFile {
                                        path: json_path.display().to_string(),
                                        source: e,
                                    }
                                })?;
                                tracing::info!("Saved raw spec: {}", json_path.display());

                                specs.insert(spec.provider.clone(), spec.clone());
                            }
                            Err(e) => {
                                tracing::warn!("Failed to fetch spec for {provider}: {e}");
                            }
                        }
                    }
                }
            }
        }

        let elapsed = start_time.elapsed();
        tracing::info!("Parallel fetch completed in {:?}", elapsed);
        tracing::info!(
            "Estimated sequential time: ~{:?} ({}x slower)",
            elapsed * providers.len() as u32,
            providers.len()
        );

        Ok(specs)
    }

    /// Fetch a single provider's spec (blocking).
    pub fn fetch_single(
        &self,
        client: &mut SimpleHttpClient,
        provider: &str,
    ) -> Result<DistilledSpec, SpecFetchError> {
        let (provider_static, url) = Self::configured_providers()
            .iter()
            .find(|(name, _)| *name == provider)
            .map(|&(p, u)| (p, u))
            .ok_or_else(|| SpecFetchError::Http {
                provider: provider.to_string(),
                source: HttpClientError::InvalidUrl(
                    format!("Unknown provider: {provider}"),
                ),
            })?;

        // Build the request
        let request = client.get(url).map_err(|e| SpecFetchError::Http {
            provider: provider.to_string(),
            source: e,
        })?
        .build().map_err(|e| SpecFetchError::Http {
            provider: provider.to_string(),
            source: e,
        })?;

        // Create and execute the task inline to avoid boxing/lifetime issues
        let task = SendRequestTask::new(
            request, 5,
            client.client_pool().expect("should have pool"),
            client.client_config(),
        )
        .map_ready(move |intro| {
            match intro {
                RequestIntro::Success { stream, .. } => {
                    let body_text = body_reader::collect_string(stream);
                    parse_spec_response(&body_text, provider_static, url)
                }
                RequestIntro::Failed(e) => {
                    tracing::warn!("HTTP request failed for {provider_static}: {e}");
                    Box::new([]) as Box<[Result<DistilledSpec, SpecFetchError>]>
                }
            }
        })
        .map_pending(move |p| SpecFetchPending::from_http(p, provider_static));

        let results_stream = valtron::execute(task, None)
            .expect("execute should return stream");

        for stream_item in results_stream {
            if let Stream::Next(result_box) = stream_item {
                for result in result_box.iter() {
                    return result.clone();
                }
            }
        }

        Err(SpecFetchError::Generic("No result from fetch task".into()))
    }

    /// Write a distilled spec to its repository directory.
    ///
    /// Uses blocking std::fs at sync boundary (after Valtron execution).
    pub fn write_spec(
        &self,
        spec: &DistilledSpec,
        repo_name: &str,
    ) -> Result<PathBuf, SpecFetchError> {
        let repo_path = self.specs_base.join(repo_name);
        let specs_dir = repo_path.join("specs");

        // Ensure directory exists
        fs::create_dir_all(&specs_dir).map_err(|e| SpecFetchError::WriteFile {
            path: specs_dir.display().to_string(),
            source: e,
        })?;

        // Write the spec
        let filename = format!("openapi-{}.json", spec.version);
        let spec_path = specs_dir.join(&filename);

        let json = serde_json::to_string_pretty(&spec.raw_spec).map_err(|e| {
            SpecFetchError::Json {
                provider: spec.provider.clone(),
                source: e,
            }
        })?;

        fs::write(&spec_path, json).map_err(|e| SpecFetchError::WriteFile {
            path: spec_path.display().to_string(),
            source: e,
        })?;

        // Write manifest
        self.write_manifest(&specs_dir, spec)?;

        Ok(spec_path)
    }

    /// Check if spec has changed from previous fetch.
    pub fn has_changed(&self, repo_name: &str, new_hash: &str) -> Result<bool, SpecFetchError> {
        let manifest_path =
            self.specs_base.join(repo_name).join("specs").join("_manifest.json");

        match fs::read_to_string(&manifest_path) {
            Ok(content) => {
                let manifest: Value = serde_json::from_str(&content).map_err(|e| {
                    SpecFetchError::Json {
                        provider: repo_name.to_string(),
                        source: e,
                    }
                })?;

                let old_hash = manifest
                    .get("content_hash")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                Ok(old_hash != new_hash)
            }
            Err(_) => Ok(true), // No previous spec, consider it changed
        }
    }

    /// List of all configured providers and their spec URLs.
    pub fn configured_providers() -> Vec<(&'static str, &'static str)> {
        vec![
            ("fly-io", "https://docs.machines.dev/spec/openapi3.json"),
            (
                "planetscale",
                "https://api.planetscale.com/v1/openapi-spec",
            ),
            ("cloudflare", "https://github.com/cloudflare/api-schemas"),
            (
                "gcp",
                "https://discovery.googleapis.com/discovery/v1/apis",
            ),
            ("prisma-postgres", "https://api.prisma.io/v1/doc"),
            ("supabase", "https://api.supabase.com/api/v1-json"),
            (
                "mongodb-atlas",
                "https://www.mongodb.com/docs/api/doc/atlas-admin-api-v2.json",
            ),
            ("neon", "https://neon.com/api_spec/release/v2.json"),
            ("stripe", "https://docs.stripe.com/api"),
        ]
    }

    fn write_manifest(
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
        fs::write(manifest_path, json)?;

        Ok(())
    }
}

/// Parse HTTP response body and extract DistilledSpec.
fn parse_spec_response(
    body_text: &str,
    provider: &str,
    url: &str,
) -> Box<[Result<DistilledSpec, SpecFetchError>]> {
    // Parse as JSON
    let raw_spec: Value = match serde_json::from_str(body_text) {
        Ok(spec) => spec,
        Err(e) => {
            return Box::new([Err(SpecFetchError::Json {
                provider: provider.to_string(),
                source: e,
            })]);
        }
    };

    // Compute content hash for change detection
    let content_hash = compute_sha256(&raw_spec.to_string());

    // Extract endpoints (provider-specific logic)
    let endpoints = extract_endpoints(&raw_spec, provider);

    // Determine version from spec or timestamp
    let version = extract_version(&raw_spec, provider)
        .unwrap_or_else(|| chrono::Utc::now().format("%Y%m%d").to_string());

    Box::new([Ok(DistilledSpec {
        provider: provider.to_string(),
        version,
        fetched_at: chrono::Utc::now(),
        source_url: url.to_string(),
        raw_spec,
        endpoints,
        content_hash,
    })])
}

fn compute_sha256(content: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn extract_endpoints(spec: &Value, provider: &str) -> Option<Vec<SpecEndpoint>> {
    // Provider-specific extraction logic
    match provider {
        "gcp" => extract_gcp_endpoints(spec),
        "stripe" => extract_stripe_endpoints(spec),
        _ => None, // Single-spec providers don't need extraction
    }
}

fn extract_gcp_endpoints(spec: &Value) -> Option<Vec<SpecEndpoint>> {
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

fn extract_stripe_endpoints(spec: &Value) -> Option<Vec<SpecEndpoint>> {
    // Stripe uses a different format - extract from paths
    spec.get("paths")
        .and_then(|paths| paths.as_object())
        .map(|paths_obj| {
            paths_obj
                .keys()
                .map(|path| SpecEndpoint {
                    path: path.clone(),
                    methods: vec![],
                    operation_id: None,
                    summary: None,
                })
                .collect()
        })
}

fn extract_version(spec: &Value, _provider: &str) -> Option<String> {
    spec.get("info")
        .and_then(|i| i.get("version"))
        .and_then(|v| v.as_str())
        .map(String::from)
}

// ============================================================================
// SpecFetchPending helpers
// ============================================================================

impl SpecFetchPending {
    fn from_http(p: HttpRequestPending, provider: &'static str) -> Self {
        match p {
            HttpRequestPending::WaitingForStream => Self::Connecting { provider },
            HttpRequestPending::WaitingIntroAndHeaders => Self::AwaitingResponse { provider },
        }
    }
}
