//! ProviderSpecFetcher - orchestrates fetching specs from multiple providers.
//!
//! WHY: Fetches OpenAPI specs from multiple providers in parallel using Valtron.
//!
//! WHAT: Central coordinator that runs all fetches in parallel using Valtron's
//! `execute_collect_all` pattern, same as `gen_model_descriptors`.
//!
//! HOW: Uses `foundation_deployment::providers::spec_fetch` for provider-specific
//! fetches, all wrapped in Valtron's `from_future` for parallel execution.

use foundation_core::valtron::{Stream, StreamIteratorExt};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_deployment::providers::resources::{cloudflare, gcp, standard};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use super::core::DistilledSpec;
use super::errors::SpecFetchError;

/// WHY: Orchestrates fetching specs from multiple providers.
///
/// WHAT: Central coordinator that runs all fetches in parallel.
///
/// HOW: Uses `foundation_deployment::providers::spec_fetch` for each provider,
/// then consolidates results.
pub struct ProviderSpecFetcher;

impl ProviderSpecFetcher {
    pub fn new() -> Self {
        Self
    }

    /// Fetch specs from all configured providers in parallel.
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP client (must be created by caller with pool guard alive)
    /// * `gcp_api_filter` - Optional list of GCP API names to fetch. If None, fetches all.
    ///
    /// # Returns
    ///
    /// Map of provider name to distilled spec. Failed fetches are logged and skipped.
    pub fn fetch_all(
        &self,
        client: &SimpleHttpClient,
        gcp_api_filter: Option<Vec<String>>,
    ) -> Result<BTreeMap<String, DistilledSpec>, crate::gen_provider_specs::errors::SpecFetchError>
    {
        // Artefacts directory for raw JSON specs
        let artefacts_dir = PathBuf::from("artefacts/cloud_providers");
        std::fs::create_dir_all(&artefacts_dir).map_err(|e| {
            crate::gen_provider_specs::errors::SpecFetchError::WriteFile {
                path: artefacts_dir.display().to_string(),
                source: e,
            }
        })?;

        tracing::info!("Fetching provider specs in parallel...");
        let start_time = Instant::now();

        // Create temp dir for cloudflare
        let temp_dir = std::env::temp_dir().join("cloudflare-spec-fetch");
        std::fs::create_dir_all(&temp_dir).map_err(|e| {
            crate::gen_provider_specs::errors::SpecFetchError::WriteFile {
                path: temp_dir.display().to_string(),
                source: e,
            }
        })?;

        // Build fetch streams for each provider
        // Each returns a StreamIterator that runs on the Valtron thread pool
        // Streams yield PathBuf results that we'll consolidate into DistilledSpec
        let mut streams: Vec<
            Box<
                dyn foundation_core::valtron::StreamIterator<
                        Item = Stream<Result<PathBuf, SpecFetchError>, ()>,
                        D = Result<PathBuf, SpecFetchError>,
                        P = (),
                    > + Send
                    + 'static,
            >,
        > = Vec::new();

        for (provider, _url) in Self::configured_providers() {
            let provider_dir = artefacts_dir.join(provider);

            if provider == "cloudflare" {
                let stream =
                    cloudflare::fetch::fetch_cloudflare_specs(temp_dir.clone(), provider_dir)
                        .map_err(|e| SpecFetchError::Generic(format!("Cloudflare: {e}")))?;
                streams.push(Box::new(stream.map_done(|result| {
                    result.map_err(|e| SpecFetchError::Generic(format!("Cloudflare: {e}")))
                })));
            } else if provider == "gcp" {
                let stream = gcp::fetch::fetch_gcp_specs(client, provider_dir, gcp_api_filter.clone())
                    .map_err(|e| SpecFetchError::Generic(format!("GCP: {e}")))?;
                streams.push(Box::new(stream.map_pending(|_| ()).map_done(|result| {
                    result.map_err(|e| SpecFetchError::Generic(format!("GCP: {e}")))
                })));
            } else {
                // Standard HTTP fetch for providers with direct JSON endpoints
                let provider_name = provider.to_string();
                let stream =
                    standard::fetch::fetch_standard_spec(provider, _url, provider_dir)
                        .map_err(|e| SpecFetchError::Generic(format!("{provider}: {e}")))?;
                streams.push(Box::new(stream.map_done(move |result| {
                    result.map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))
                })));
            }
        }

        // Execute all streams and collect results, building DistilledSpec for each provider
        let mut specs = BTreeMap::new();

        for stream in streams {
            // Collect all paths for this provider
            let mut paths: Vec<PathBuf> = Vec::new();
            let mut provider_name: Option<String> = None;

            for item in stream {
                if let Stream::Next(result) = item {
                    match result {
                        Ok(path) => {
                            // Extract provider from first path
                            if provider_name.is_none() {
                                if let Some(parent) = path.parent() {
                                    if let Some(name) = parent.file_name() {
                                        provider_name = Some(name.to_string_lossy().to_string());
                                    }
                                }
                            }
                            paths.push(path);
                        }
                        Err(e) => {
                            tracing::warn!("Provider fetch failed: {e}");
                        }
                    }
                }
            }

            // Build DistilledSpec from collected paths
            if !paths.is_empty() {
                let provider = provider_name.unwrap_or_else(|| "unknown".to_string());

                // Get source URL from provider config
                let url = Self::configured_providers()
                    .iter()
                    .find(|(name, _)| *name == provider)
                    .map(|(_, url)| url.to_string())
                    .unwrap_or_else(|| String::new());

                // Convert paths to relative strings (relative to artefacts dir)
                let spec_files: Vec<String> = paths
                    .iter()
                    .filter_map(|p| {
                        p.strip_prefix(&artefacts_dir)
                            .ok()
                            .map(|rel| rel.display().to_string())
                    })
                    .collect();

                specs.insert(
                    provider.clone(),
                    DistilledSpec {
                        provider,
                        version: chrono::Utc::now().format("%Y%m%d").to_string(),
                        fetched_at: chrono::Utc::now(),
                        source_url: url,
                        raw_spec: serde_json::Value::Null,
                        endpoints: None,
                        content_hash: String::new(),
                        spec_files,
                    },
                );
            }
        }

        let elapsed = start_time.elapsed();
        tracing::info!("Parallel fetch completed in {:?}", elapsed);
        tracing::info!(
            "Estimated sequential time: ~{:?} ({}x slower)",
            elapsed * Self::configured_providers().len() as u32,
            Self::configured_providers().len()
        );

        Ok(specs)
    }

    /// Fetch a single provider's spec (blocking).
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP client
    /// * `provider` - Provider name to fetch
    /// * `gcp_api_filter` - Optional list of GCP API names to fetch. If None, fetches all.
    pub fn fetch_single(
        &self,
        client: &SimpleHttpClient,
        provider: &str,
        gcp_api_filter: Option<Vec<String>>,
    ) -> Result<DistilledSpec, crate::gen_provider_specs::errors::SpecFetchError> {
        let artefacts_dir = PathBuf::from("artefacts/cloud_providers");
        let provider_dir = artefacts_dir.join(provider);

        // Collect all paths for this provider
        let mut paths: Vec<PathBuf> = Vec::new();

        if provider == "cloudflare" {
            let temp_dir = std::env::temp_dir().join("cloudflare-spec-fetch");
            std::fs::create_dir_all(&temp_dir).map_err(|e| SpecFetchError::WriteFile {
                path: temp_dir.display().to_string(),
                source: e,
            })?;

            let stream = cloudflare::fetch::fetch_cloudflare_specs(temp_dir, provider_dir)
                .map_err(|e| SpecFetchError::Generic(format!("Cloudflare: {e}")))?;

            for item in stream {
                if let Stream::Next(result) = item {
                    match result {
                        Ok(path) => paths.push(path),
                        Err(e) => return Err(SpecFetchError::Generic(format!("Cloudflare: {e}"))),
                    }
                }
            }
        } else if provider == "gcp" {
            let stream = gcp::fetch::fetch_gcp_specs(client, provider_dir, gcp_api_filter)
                .map_err(|e| SpecFetchError::Generic(format!("GCP: {e}")))?;

            for item in stream {
                if let Stream::Next(result) = item {
                    match result {
                        Ok(path) => paths.push(path),
                        Err(e) => return Err(SpecFetchError::Generic(format!("GCP: {e}"))),
                    }
                }
            }

            tracing::info!("GCP: Wrote {} API specs", paths.len());
        } else {
            // Standard HTTP fetch for providers with direct JSON endpoints
            let provider_config = Self::configured_providers()
                .into_iter()
                .find(|(name, _)| *name == provider);

            let (_name, url) = provider_config.ok_or_else(|| {
                SpecFetchError::Generic(format!("Unknown provider: {provider}"))
            })?;

            let stream = standard::fetch::fetch_standard_spec(provider, url, provider_dir)
                .map_err(|e| SpecFetchError::Generic(format!("{provider}: {e}")))?;

            for item in stream {
                if let Stream::Next(result) = item {
                    match result {
                        Ok(path) => paths.push(path),
                        Err(e) => {
                            return Err(SpecFetchError::Generic(format!("{provider}: {e}")))
                        }
                    }
                }
            }
        }

        if paths.is_empty() {
            return Err(SpecFetchError::Generic(format!(
                "No result from {provider} fetch"
            )));
        }

        // Get source URL from provider config
        let source_url = Self::configured_providers()
            .iter()
            .find(|(name, _)| *name == provider)
            .map(|(_, url)| url.to_string())
            .unwrap_or_else(|| String::new());

        // Convert paths to relative strings (relative to artefacts dir)
        let spec_files: Vec<String> = paths
            .iter()
            .filter_map(|p| {
                p.strip_prefix(&artefacts_dir)
                    .ok()
                    .map(|rel| rel.display().to_string())
            })
            .collect();

        Ok(DistilledSpec {
            provider: provider.to_string(),
            version: chrono::Utc::now().format("%Y%m%d").to_string(),
            fetched_at: chrono::Utc::now(),
            source_url,
            raw_spec: serde_json::Value::Null,
            endpoints: None,
            content_hash: String::new(),
            spec_files,
        })
    }

    /// List of all configured providers and their spec URLs.
    pub fn configured_providers() -> Vec<(&'static str, &'static str)> {
        vec![
            ("fly-io", "https://docs.machines.dev/spec/openapi3.json"),
            ("planetscale", "https://api.planetscale.com/v1/openapi-spec"),
            ("cloudflare", "https://github.com/cloudflare/api-schemas"),
            ("gcp", "https://discovery.googleapis.com/discovery/v1/apis"),
            ("prisma-postgres", "https://api.prisma.io/v1/doc"),
            ("supabase", "https://api.supabase.com/api/v1-json"),
            (
                "mongodb-atlas",
                "https://www.mongodb.com/docs/api/doc/atlas-admin-api-v2.json",
            ),
            ("neon", "https://neon.com/api_spec/release/v2.json"),
            (
                "stripe",
                "https://raw.githubusercontent.com/stripe/openapi/master/openapi/spec3.json",
            ),
        ]
    }
}
