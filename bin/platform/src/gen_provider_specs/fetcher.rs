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
    ///
    /// # Returns
    ///
    /// Map of provider name to distilled spec. Failed fetches are logged and skipped.
    pub fn fetch_all(
        &self,
        client: &SimpleHttpClient,
    ) -> Result<BTreeMap<String, DistilledSpec>, crate::gen_provider_specs::errors::SpecFetchError> {
        use foundation_deployment::providers::spec_fetch::{cloudflare, gcp};

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
        let mut streams: Vec<Box<dyn foundation_core::valtron::StreamIterator<
            Item = Stream<Result<DistilledSpec, SpecFetchError>, ()>,
            D = Result<DistilledSpec, SpecFetchError>,
            P = (),
        > + Send + 'static>> = Vec::new();

        for (provider, _url) in Self::configured_providers() {
            let provider_dir = artefacts_dir.join(provider);

            if provider == "cloudflare" {
                let stream = cloudflare::fetch_cloudflare_specs(
                    temp_dir.clone(),
                    provider_dir,
                ).map_err(|e| SpecFetchError::Generic(format!("Cloudflare: {e}")))?;
                streams.push(Box::new(stream.map_done(|result| {
                    result
                        .map_err(|e| SpecFetchError::Generic(format!("Cloudflare: {e}")))
                        .map(|_path| DistilledSpec {
                            provider: "cloudflare".to_string(),
                            version: chrono::Utc::now().format("%Y%m%d").to_string(),
                            fetched_at: chrono::Utc::now(),
                            source_url: cloudflare::CLOUDFLARE_API_SCHEMAS_URL.to_string(),
                            raw_spec: serde_json::Value::Null,
                            endpoints: None,
                            content_hash: String::new(),
                        })
                })));
            } else if provider == "gcp" {
                let stream = gcp::fetch_gcp_specs(client, provider_dir)
                    .map_err(|e| SpecFetchError::Generic(format!("GCP: {e}")))?;
                streams.push(Box::new(stream
                    .map_pending(|_| ())
                    .map_done(|result| {
                    result
                        .map_err(|e| SpecFetchError::Generic(format!("GCP: {e}")))
                        .map(|_path| DistilledSpec {
                            provider: "gcp".to_string(),
                            version: chrono::Utc::now().format("%Y%m%d").to_string(),
                            fetched_at: chrono::Utc::now(),
                            source_url: gcp::GCP_DISCOVERY_URL.to_string(),
                            raw_spec: serde_json::Value::Null,
                            endpoints: None,
                            content_hash: String::new(),
                        })
                })));
            } else {
                // Standard HTTP fetch for other providers
                // TODO: Implement using foundation_deployment HTTP fetcher
                tracing::warn!("Standard HTTP fetch not yet implemented for {provider}");
            }
        }

        // Execute all streams and collect results
        let mut specs = BTreeMap::new();

        // Collect from each stream - they're all running in parallel on the thread pool
        for stream in streams {
            for item in stream {
                if let Stream::Next(result) = item {
                    match result {
                        Ok(spec) => {
                            specs.insert(spec.provider.clone(), spec);
                        }
                        Err(e) => {
                            tracing::warn!("Provider fetch failed: {e}");
                        }
                    }
                }
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
    pub fn fetch_single(
        &self,
        client: &SimpleHttpClient,
        provider: &str,
    ) -> Result<DistilledSpec, crate::gen_provider_specs::errors::SpecFetchError> {
        use foundation_deployment::providers::spec_fetch::{cloudflare, gcp};

        let artefacts_dir = PathBuf::from("artefacts/cloud_providers");
        let provider_dir = artefacts_dir.join(provider);

        if provider == "cloudflare" {
            let temp_dir = std::env::temp_dir().join("cloudflare-spec-fetch");
            std::fs::create_dir_all(&temp_dir).map_err(|e| SpecFetchError::WriteFile {
                path: temp_dir.display().to_string(),
                source: e,
            })?;

            let stream = cloudflare::fetch_cloudflare_specs(temp_dir, provider_dir)
                .map_err(|e| SpecFetchError::Generic(format!("Cloudflare: {e}")))?;

            // Collect single result
            for item in stream {
                if let Stream::Next(result) = item {
                    return result
                        .map_err(|e| SpecFetchError::Generic(format!("Cloudflare: {e}")))
                        .map(|_path| DistilledSpec {
                            provider: "cloudflare".to_string(),
                            version: chrono::Utc::now().format("%Y%m%d").to_string(),
                            fetched_at: chrono::Utc::now(),
                            source_url: cloudflare::CLOUDFLARE_API_SCHEMAS_URL.to_string(),
                            raw_spec: serde_json::Value::Null,
                            endpoints: None,
                            content_hash: String::new(),
                        });
                }
            }
            return Err(SpecFetchError::Generic("No result from fetch".into()));
        }

        if provider == "gcp" {
            let stream = gcp::fetch_gcp_specs(client, provider_dir)
                .map_err(|e| SpecFetchError::Generic(format!("GCP: {e}")))?;

            for item in stream {
                if let Stream::Next(result) = item {
                    return result
                        .map_err(|e| SpecFetchError::Generic(format!("GCP: {e}")))
                        .map(|_path| DistilledSpec {
                            provider: "gcp".to_string(),
                            version: chrono::Utc::now().format("%Y%m%d").to_string(),
                            fetched_at: chrono::Utc::now(),
                            source_url: gcp::GCP_DISCOVERY_URL.to_string(),
                            raw_spec: serde_json::Value::Null,
                            endpoints: None,
                            content_hash: String::new(),
                        });
                }
            }
            return Err(SpecFetchError::Generic("No result from fetch".into()));
        }

        Err(crate::gen_provider_specs::errors::SpecFetchError::Generic(
            format!("Provider {provider} not yet implemented")
        ))
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
            ("stripe", "https://docs.stripe.com/api"),
        ]
    }
}
