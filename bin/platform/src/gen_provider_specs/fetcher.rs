//! ProviderSpecFetcher - orchestrates fetching specs from multiple providers.
//!
//! WHY: Fetches OpenAPI specs from multiple providers in parallel using Valtron.
//!
//! WHAT: Central coordinator that runs all fetches in parallel using Valtron's
//! `execute_collect_all` pattern, same as `gen_model_descriptors`.
//!
//! HOW: Uses `foundation_deployment::providers` for provider-specific
//! fetches, all wrapped in Valtron's `from_future` for parallel execution.

use foundation_core::valtron::{Stream, StreamIteratorExt};
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use foundation_deployment::providers::{
    cloudflare, fly_io, gcp, mongodb_atlas, neon, openapi, planetscale, prisma_postgres, standard,
    stripe, supabase,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use super::core::{DistilledSpec, SpecEndpoint};
use super::errors::SpecFetchError;

/// WHY: Orchestrates fetching specs from multiple providers.
///
/// WHAT: Central coordinator that runs all fetches in parallel.
///
/// HOW: Uses `foundation_deployment::providers` for each provider,
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
                let stream =
                    gcp::fetch::fetch_gcp_specs(client, provider_dir, gcp_api_filter.clone())
                        .map_err(|e| SpecFetchError::Generic(format!("GCP: {e}")))?;
                streams.push(Box::new(stream.map_pending(|_| ()).map_done(|result| {
                    result.map_err(|e| SpecFetchError::Generic(format!("GCP: {e}")))
                })));
            } else {
                // Use provider-specific fetch functions from foundation_deployment
                let provider_name = provider.to_string();
                let stream = Self::create_provider_stream(provider, provider_dir.clone())?;
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
                    .unwrap_or_else(String::new);

                // Convert paths to relative strings (relative to artefacts dir)
                let spec_files: Vec<String> = paths
                    .iter()
                    .filter_map(|p| {
                        p.strip_prefix(&artefacts_dir)
                            .ok()
                            .map(|rel| rel.display().to_string())
                    })
                    .collect();

                let mut spec = DistilledSpec {
                    provider: provider.clone(),
                    version: chrono::Utc::now().format("%Y%m%d").to_string(),
                    fetched_at: chrono::Utc::now(),
                    source_url: url.clone(),
                    raw_spec: serde_json::Value::Null,
                    endpoints: None,
                    content_hash: String::new(),
                    spec_files: spec_files.clone(),
                };

                // Enrich with version, endpoints, and content hash from
                // the first spec file (standard providers write one file).
                if let Some(first_path) = paths.first() {
                    Self::enrich_spec(&provider, first_path, &mut spec);
                }

                // Write/update _manifest.json with the complete spec_files list
                let provider_dir = artefacts_dir.join(&provider);
                Self::write_manifest(&provider_dir, &provider, &url, &spec_files);

                specs.insert(provider, spec);
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

            let stream = cloudflare::fetch::fetch_cloudflare_specs(temp_dir, provider_dir.clone())
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
            let stream = gcp::fetch::fetch_gcp_specs(client, provider_dir.clone(), gcp_api_filter)
                .map_err(|e| SpecFetchError::Generic(format!("GCP: {e}")))?;

            for item in stream {
                if let Stream::Next(result) = item {
                    match result {
                        Ok(path) => paths.push(path),
                        Err(e) => {
                            tracing::error!("Failed to fetch GCP spec: {e}")
                        }
                    }
                }
            }

            tracing::info!("GCP: Wrote {} API specs", paths.len());
        } else {
            // Use provider-specific fetch from foundation_deployment
            let stream = Self::create_provider_stream(provider, provider_dir.clone())?;

            for item in stream {
                if let Stream::Next(result) = item {
                    match result {
                        Ok(path) => paths.push(path),
                        Err(e) => return Err(SpecFetchError::Generic(format!("{provider}: {e}"))),
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
            .unwrap_or_else(String::new);

        // Convert paths to relative strings (relative to artefacts dir)
        let spec_files: Vec<String> = paths
            .iter()
            .filter_map(|p| {
                p.strip_prefix(&artefacts_dir)
                    .ok()
                    .map(|rel| rel.display().to_string())
            })
            .collect();

        let mut spec = DistilledSpec {
            provider: provider.to_string(),
            version: chrono::Utc::now().format("%Y%m%d").to_string(),
            fetched_at: chrono::Utc::now(),
            source_url: source_url.clone(),
            raw_spec: serde_json::Value::Null,
            endpoints: None,
            content_hash: String::new(),
            spec_files: spec_files.clone(),
        };

        // Enrich with version, endpoints, and content hash
        if let Some(first_path) = paths.first() {
            Self::enrich_spec(provider, first_path, &mut spec);
        }

        // Write/update _manifest.json with the complete spec_files list
        Self::write_manifest(&provider_dir, provider, &source_url, &spec_files);

        Ok(spec)
    }

    /// List of all configured providers and their spec URLs.
    ///
    /// Built from the per-provider modules in `foundation_deployment`.
    pub fn configured_providers() -> Vec<(&'static str, &'static str)> {
        vec![
            (fly_io::fetch::PROVIDER_NAME, fly_io::fetch::SPEC_URL),
            (
                planetscale::fetch::PROVIDER_NAME,
                planetscale::fetch::SPEC_URL,
            ),
            (
                prisma_postgres::fetch::PROVIDER_NAME,
                prisma_postgres::fetch::SPEC_URL,
            ),
            (supabase::fetch::PROVIDER_NAME, supabase::fetch::SPEC_URL),
            (
                mongodb_atlas::fetch::PROVIDER_NAME,
                mongodb_atlas::fetch::SPEC_URL,
            ),
            (neon::fetch::PROVIDER_NAME, neon::fetch::SPEC_URL),
            (stripe::fetch::PROVIDER_NAME, stripe::fetch::SPEC_URL),
            ("cloudflare", cloudflare::fetch::CLOUDFLARE_API_SCHEMAS_URL),
            ("gcp", gcp::fetch::GCP_DISCOVERY_URL),
        ]
    }

    /// Create a fetch stream for a standard provider using its
    /// per-provider fetch function from `foundation_deployment`.
    fn create_provider_stream(
        provider: &str,
        output_dir: PathBuf,
    ) -> Result<
        Box<
            dyn foundation_core::valtron::StreamIterator<
                    Item = Stream<Result<PathBuf, SpecFetchError>, ()>,
                    D = Result<PathBuf, SpecFetchError>,
                    P = (),
                > + Send
                + 'static,
        >,
        SpecFetchError,
    > {
        let provider_name = provider.to_string();

        match provider {
            "fly-io" => {
                let stream = fly_io::fetch::fetch_fly_io_specs(output_dir)
                    .map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))?;
                Ok(Box::new(stream.map_done(move |r| {
                    r.map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))
                })))
            }
            "planetscale" => {
                let stream = planetscale::fetch::fetch_planetscale_specs(output_dir)
                    .map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))?;
                Ok(Box::new(stream.map_done(move |r| {
                    r.map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))
                })))
            }
            "prisma-postgres" => {
                let stream = prisma_postgres::fetch::fetch_prisma_postgres_specs(output_dir)
                    .map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))?;
                Ok(Box::new(stream.map_done(move |r| {
                    r.map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))
                })))
            }
            "supabase" => {
                let stream = supabase::fetch::fetch_supabase_specs(output_dir)
                    .map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))?;
                Ok(Box::new(stream.map_done(move |r| {
                    r.map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))
                })))
            }
            "mongodb-atlas" => {
                let stream = mongodb_atlas::fetch::fetch_mongodb_atlas_specs(output_dir)
                    .map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))?;
                Ok(Box::new(stream.map_done(move |r| {
                    r.map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))
                })))
            }
            "neon" => {
                let stream = neon::fetch::fetch_neon_specs(output_dir)
                    .map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))?;
                Ok(Box::new(stream.map_done(move |r| {
                    r.map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))
                })))
            }
            "stripe" => {
                let stream = stripe::fetch::fetch_stripe_specs(output_dir)
                    .map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))?;
                Ok(Box::new(stream.map_done(move |r| {
                    r.map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))
                })))
            }
            _ => {
                // Fallback to standard fetch for unknown providers
                let stream =
                    standard::fetch::fetch_standard_spec(&provider_name, "", output_dir)
                        .map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))?;
                Ok(Box::new(stream.map_done(move |r| {
                    r.map_err(|e| SpecFetchError::Generic(format!("{provider_name}: {e}")))
                })))
            }
        }
    }

    /// Read a written spec file and run extraction via `openapi::process_spec`.
    ///
    /// Populates `DistilledSpec` with version, endpoints, and content hash.
    fn enrich_spec(provider: &str, spec_path: &std::path::Path, spec: &mut DistilledSpec) {
        let Ok(content) = std::fs::read_to_string(spec_path) else {
            tracing::warn!("{provider}: could not read spec file for extraction");
            return;
        };

        let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
            tracing::warn!("{provider}: could not parse spec JSON for extraction");
            return;
        };

        let processed = openapi::process_spec(&json);

        if let Some(v) = processed.version {
            spec.version = v;
        }
        spec.endpoints = processed.endpoints.map(|eps| {
            eps.into_iter()
                .map(|e| SpecEndpoint {
                    path: e.path,
                    methods: e.methods,
                    operation_id: e.operation_id,
                    summary: e.summary,
                })
                .collect()
        });
        spec.content_hash = processed.content_hash;
    }

    /// Write a consistent `_manifest.json` for a provider.
    ///
    /// This is called after all spec files have been collected, ensuring
    /// the manifest always reflects the complete list of spec files on disk.
    fn write_manifest(
        provider_dir: &std::path::Path,
        provider: &str,
        source_url: &str,
        spec_files: &[String],
    ) {
        let manifest = serde_json::json!({
            "provider": provider,
            "source": source_url,
            "fetched_at": chrono::Utc::now().to_rfc3339(),
            "spec_files": spec_files,
        });

        let manifest_path = provider_dir.join("_manifest.json");
        match serde_json::to_string_pretty(&manifest) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&manifest_path, json) {
                    tracing::warn!("{provider}: failed to write manifest: {e}");
                }
            }
            Err(e) => {
                tracing::warn!("{provider}: failed to serialize manifest: {e}");
            }
        }
    }
}
