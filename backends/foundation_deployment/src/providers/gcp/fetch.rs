//! GCP Discovery Service spec fetcher.
//!
//! WHY: GCP uses a two-stage fetch - first the Discovery directory,
//! then individual API discovery documents.
//!
//! WHAT: Fetches the API directory, then fetches ALL API specs in parallel.
//!
//! HOW: Uses combinators to chain: directory fetch → create API tasks →
//! `collect_all_streams` → write output. No blocking, no `from_future`.

use crate::error::DeploymentError;
use foundation_core::valtron::{
    collect_next_from_streams, execute, one_shot, Stream, StreamIterator, StreamIteratorExt,
    TaskIteratorExt, TaskShortCircuit, TaskStatus,
};
use foundation_core::wire::simple_http::client::{
    body_reader, ClientConfig, HttpConnectionPool, RequestIntro, SendRequestTask, SimpleHttpClient,
    SystemDnsResolver,
};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// GCP Discovery Service URL.
pub const GCP_DISCOVERY_URL: &str = "https://discovery.googleapis.com/discovery/v1/apis";

/// GCP API entry from the Discovery directory.
#[derive(Debug, serde::Deserialize, Clone)]
pub struct GcpApiEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub title: String,
    #[serde(rename = "discoveryRestUrl")]
    pub discovery_rest_url: String,
    #[serde(default)]
    pub preferred: bool,
}

/// Directory response structure.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GcpDirectoryResponse {
    pub items: Vec<GcpApiEntry>,
}

/// Progress states for GCP fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GcpFetchPending {
    FetchingDirectory,
    FetchingApiSpecs { remaining: usize },
    WritingFiles,
}

type ApiFetchStream = Box<
    dyn StreamIterator<
            D = Result<PathBuf, DeploymentError>,
            P = GcpFetchPending,
            Item = Stream<Result<PathBuf, DeploymentError>, GcpFetchPending>,
        > + Send,
>;

/// Fetch ALL GCP specs using two-stage approach with combinators.
///
/// # Arguments
///
/// * `client` - The HTTP client to use for fetching API specs
/// * `output_dir` - Directory where fetched specs will be written
/// * `api_filter` - Optional list of API names to filter. If None, fetches ALL APIs.
///
/// # Returns
///
/// A stream iterator that yields results of writing API spec files to disk.
///
/// # Errors
///
/// Returns `DeploymentError` if:
/// - Failed to build the HTTP request for the discovery directory
/// - Valtron scheduling fails during execution
/// - JSON parsing fails for the directory or API spec responses
/// - HTTP request fails during directory or spec fetch
/// - File system operations fail when writing output
///
/// # Panics
///
/// Panics if the client does not have an associated connection pool.
/// This should not occur in normal usage as the client is expected
/// to be properly initialized with a pool.
pub fn fetch_gcp_specs(
    client: &SimpleHttpClient,
    output_dir: PathBuf,
    api_filter: Option<Vec<String>>,
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = GcpFetchPending> + Send + 'static,
    DeploymentError,
> {
    let pool = client.client_pool().expect("should have pool");
    let config = client.client_config();

    // Wrap output_dir in Arc for use across closure boundaries
    let output_dir = Arc::new(output_dir);

    debug!("GCP fetch: output_dir={:?}", output_dir);

    // Stage 1: Build directory fetch request
    let request = client
        .get(GCP_DISCOVERY_URL)
        .map_err(|e| DeploymentError::Generic(format!("Failed to build request: {e}")))?
        .build()
        .map_err(|e| DeploymentError::Generic(format!("Failed to build request: {e}")))?;

    // Stage 1: Fetch directory
    let directory_stream = SendRequestTask::new(request, 5, pool.clone(), config.clone())
        .map_ready(|intro| match intro {
            RequestIntro::Success { stream, .. } => {
                info!("GCP directory response received, parsing");
                let body_text = body_reader::collect_string(stream);
                serde_json::from_str::<GcpDirectoryResponse>(&body_text)
                    .map_err(|e| DeploymentError::Generic(format!("JSON parse error: {e}")))
            }
            RequestIntro::Failed(e) => {
                error!("GCP directory fetch failed: {e}");
                Err(DeploymentError::Generic(format!(
                    "HTTP request failed: {e}"
                )))
            }
        })
        .map_pending(|_| GcpFetchPending::FetchingDirectory);

    // Execute directory fetch
    let executed = execute(directory_stream, None)
        .map_err(|e| DeploymentError::Generic(format!("Valtron scheduling failed: {e}")))?;

    // Stage 2: Transform directory result into API spec fetch stream using map_iter_done
    let api_fetch_stream = executed.map_iter_done(move |directory_result| {
        match directory_result {
            Ok(directory) => {
                let total_apis = directory.items.len();
                info!("gcp: Found {} APIs in directory", total_apis);

                // Select only the latest version of each API.
                // The directory lists multiple versions per API (e.g. compute:v1,
                // compute:beta, compute:alpha). We group by name and pick the best
                // version for each using: preferred flag > stable > beta > alpha,
                // then highest version number within each tier.
                let latest_items = select_latest_versions(directory.items);

                info!(
                    "gcp: {} total entries, {} unique APIs (latest versions only)",
                    total_apis,
                    latest_items.len()
                );

                // Apply optional name filter
                let filtered_items: Vec<GcpApiEntry> = if let Some(ref filter) = api_filter {
                    latest_items
                        .into_iter()
                        .filter(|item| filter.contains(&item.name))
                        .collect()
                } else {
                    latest_items
                };

                info!(
                    "gcp: Fetching {} APIs after filter",
                    filtered_items.len()
                );

                // Create fetch streams for all APIs - execute each task immediately
                let mut streams: Vec<Box<dyn StreamIterator<D = _, P = _, Item = _> + Send>> =
                    Vec::new();

                for entry in filtered_items {
                    let output_dir = Arc::clone(&output_dir);
                    match create_api_fetch_task(entry, pool.clone(), config.clone()) {
                        Ok(task) => match execute(task, None) {
                            Ok(stream) => streams.push(Box::new(stream.map_done(move |result| {
                                match result {
                                    Some(Ok((entry, spec))) => {
                                        info!("Fetched: {} ({})", entry.name, entry.version);
                                        // Write this single spec immediately
                                        let write_result =
                                            write_single_spec(&output_dir, &entry, &spec);
                                        match write_result {
                                            Ok(path) => {
                                                info!(
                                                    "Written to: {} ({}) - {:?}",
                                                    entry.name, entry.version, &path
                                                );
                                                Ok(path)
                                            }
                                            Err(e) => {
                                                error!("Failed to write {}: {}", entry.name, e);
                                                Err(e)
                                            }
                                        }
                                    }
                                    Some(Err(e)) => {
                                        warn!("Spec fetch failed for API: {}", e);
                                        Err(e)
                                    }
                                    None => unreachable!(
                                        "collect_next_from_streams should not produce None"
                                    ),
                                }
                            }))),
                            Err(e) => warn!("Failed to execute fetch task: {}", e),
                        },
                        Err(e) => warn!("Failed to create fetch task: {}", e),
                    }
                }

                // Write each API spec as soon as it's fetched to avoid OOM.
                // Each stream produces one (entry, spec) result, which we write immediately.
                tracing::info!("Collecting results from {} streams", streams.len());
                Box::new(
                    collect_next_from_streams(streams)
                        .map_pending(|_| GcpFetchPending::FetchingApiSpecs { remaining: 0 }),
                ) as ApiFetchStream
            }
            Err(e) => {
                error!("Failed to successfully fetch GCP API specs: {}", e);
                Box::new(
                    one_shot::<_, GcpFetchPending>(Err(e))
                        .map_pending(|_| GcpFetchPending::FetchingApiSpecs { remaining: 0 }),
                ) as ApiFetchStream
            }
        }
    });

    Ok(api_fetch_stream)
}

/// Create a task to fetch a single API spec.
type ApiFetchTask = Box<
    dyn foundation_core::valtron::TaskIterator<
            Ready = Option<Result<(GcpApiEntry, Value), DeploymentError>>,
            Pending = usize,
            Spawner = foundation_core::valtron::BoxedSendExecutionAction,
        > + Send
        + 'static,
>;

fn create_api_fetch_task(
    entry: GcpApiEntry,
    pool: Arc<HttpConnectionPool<SystemDnsResolver>>,
    config: ClientConfig,
) -> Result<ApiFetchTask, DeploymentError> {
    let request = SimpleHttpClient::new(config.clone(), pool.clone())
        .get(&entry.discovery_rest_url)
        .map_err(|e| DeploymentError::Generic(format!("Failed to build request: {e}")))?
        .build()
        .map_err(|e| DeploymentError::Generic(format!("Failed to build request: {e}")))?;

    let name = entry.name.clone();
    let task = SendRequestTask::new(request, 5, pool, config)
        .map_ready(move |intro| match intro {
            RequestIntro::Success { stream, .. } => {
                debug!(
                    "gcp/{}/{}: Response received",
                    name, entry.discovery_rest_url
                );
                let body = body_reader::collect_string(stream);
                debug!(
                    "gcp/{}/{}: Body length: {}",
                    name,
                    entry.discovery_rest_url,
                    body.len()
                );

                match serde_json::from_str::<Value>(body.trim()) {
                    Ok(spec) => {
                        info!("gcp/{}/{}: JSON parsed", name, entry.discovery_rest_url);
                        Some(Ok((entry.clone(), spec)))
                    }
                    Err(e) => {
                        error!(
                            "gcp/{}/{}: JSON error: {}",
                            name, entry.discovery_rest_url, e
                        );
                        Some(Err(DeploymentError::Generic(format!("JSON error: {e}"))))
                    }
                }
            }
            RequestIntro::Failed(e) => {
                error!("gcp/{}: Request failed: {}", name, e);
                Some(Err(DeploymentError::Generic(format!("HTTP error: {e}"))))
            }
        })
        .map_pending(|_| 0)
        .map_circuit(|item| match &item {
            TaskStatus::Ready(Some(Err(_))) => TaskShortCircuit::ReturnAndStop(item),
            _ => TaskShortCircuit::Continue(item),
        });

    Ok(Box::new(task))
}

/// Select the latest version of each API from the directory listing.
///
/// Groups entries by API name, then picks the best version for each group.
/// Selection priority:
/// 1. If an entry is marked `preferred`, it wins immediately
/// 2. Otherwise, rank by version tier: stable > beta > alpha
/// 3. Within the same tier, pick the highest version number
///
/// GCP version strings follow patterns like:
/// - `v1`, `v2`, `v3` (stable)
/// - `v1beta1`, `v2beta1` (beta)
/// - `v1alpha1`, `v1alpha2` (alpha)
/// - `datatransfer:v1` (stable, id-style)
fn select_latest_versions(items: Vec<GcpApiEntry>) -> Vec<GcpApiEntry> {
    let mut by_name: HashMap<String, Vec<GcpApiEntry>> = HashMap::new();

    for item in items {
        by_name.entry(item.name.clone()).or_default().push(item);
    }

    let mut result: Vec<GcpApiEntry> = Vec::with_capacity(by_name.len());

    for (_name, mut versions) in by_name {
        if versions.len() == 1 {
            result.push(versions.remove(0));
            continue;
        }

        // If exactly one is marked preferred, use it
        let preferred: Vec<&GcpApiEntry> = versions.iter().filter(|v| v.preferred).collect();
        if preferred.len() == 1 {
            let idx = versions.iter().position(|v| v.preferred).unwrap();
            result.push(versions.remove(idx));
            continue;
        }

        // Rank all versions and pick the best
        versions.sort_by(|a, b| version_rank(&b.version).cmp(&version_rank(&a.version)));
        result.push(versions.remove(0));
    }

    result.sort_by(|a, b| a.name.cmp(&b.name));
    result
}

/// Rank a GCP version string for comparison.
///
/// Returns `(tier, major, minor)` where:
/// - tier: 2 = stable, 1 = beta, 0 = alpha
/// - major: the leading version number (e.g. 2 from "v2beta1")
/// - minor: the trailing number in beta/alpha (e.g. 1 from "v2beta1")
fn version_rank(version: &str) -> (u32, u32, u32) {
    let v = version.trim_start_matches('v');

    if let Some(pos) = v.find("alpha") {
        let major = v[..pos].parse::<u32>().unwrap_or(0);
        let minor = v[pos + 5..].parse::<u32>().unwrap_or(0);
        (0, major, minor)
    } else if let Some(pos) = v.find("beta") {
        let major = v[..pos].parse::<u32>().unwrap_or(0);
        let minor = v[pos + 4..].parse::<u32>().unwrap_or(0);
        (1, major, minor)
    } else {
        // Stable: "v1", "v2", etc.
        let major = v.parse::<u32>().unwrap_or(0);
        (2, major, 0)
    }
}

fn write_single_spec(
    output_dir: &std::path::Path,
    entry: &GcpApiEntry,
    spec: &Value,
) -> Result<PathBuf, DeploymentError> {
    let api_dir = output_dir.join(&entry.name);
    std::fs::create_dir_all(&api_dir).map_err(|e| {
        DeploymentError::Generic(format!(
            "Failed to create directory for {}: {e}",
            entry.name
        ))
    })?;

    let api_path = api_dir.join("openapi.json");
    let json = serde_json::to_string_pretty(spec)
        .map_err(|e| DeploymentError::Generic(format!("Failed to serialize JSON: {e}")))?;

    std::fs::write(&api_path, json).map_err(|e| {
        DeploymentError::Generic(format!("Failed to write {}: {e}", api_path.display()))
    })?;

    Ok(api_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, version: &str, preferred: bool) -> GcpApiEntry {
        GcpApiEntry {
            id: format!("{name}:{version}"),
            name: name.to_string(),
            version: version.to_string(),
            title: name.to_string(),
            discovery_rest_url: format!("https://example.com/{name}/{version}"),
            preferred,
        }
    }

    #[test]
    fn version_rank_stable_over_beta_over_alpha() {
        assert!(version_rank("v1") > version_rank("v1beta1"));
        assert!(version_rank("v1beta1") > version_rank("v1alpha1"));
        assert!(version_rank("v2") > version_rank("v1"));
    }

    #[test]
    fn version_rank_higher_major_wins() {
        assert!(version_rank("v2") > version_rank("v1"));
        assert!(version_rank("v2beta1") > version_rank("v1beta1"));
        assert!(version_rank("v3alpha1") > version_rank("v2alpha1"));
    }

    #[test]
    fn version_rank_higher_minor_wins_within_tier() {
        assert!(version_rank("v1beta2") > version_rank("v1beta1"));
        assert!(version_rank("v1alpha2") > version_rank("v1alpha1"));
    }

    #[test]
    fn version_rank_stable_v1_beats_v2beta() {
        // Stable v1 should beat v2beta because stable tier (2) > beta tier (1)
        assert!(version_rank("v1") > version_rank("v2beta1"));
    }

    #[test]
    fn selects_preferred_entry() {
        let items = vec![
            entry("compute", "v1", true),
            entry("compute", "v1beta1", false),
            entry("compute", "alpha", false),
        ];
        let result = select_latest_versions(items);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].version, "v1");
    }

    #[test]
    fn selects_stable_over_beta_when_no_preferred() {
        let items = vec![
            entry("storage", "v1beta2", false),
            entry("storage", "v1", false),
        ];
        let result = select_latest_versions(items);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].version, "v1");
    }

    #[test]
    fn selects_highest_beta_when_no_stable() {
        let items = vec![
            entry("newapi", "v1beta1", false),
            entry("newapi", "v2beta1", false),
        ];
        let result = select_latest_versions(items);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].version, "v2beta1");
    }

    #[test]
    fn handles_single_version_api() {
        let items = vec![entry("simple", "v1", false)];
        let result = select_latest_versions(items);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "simple");
    }

    #[test]
    fn deduplicates_across_multiple_apis() {
        let items = vec![
            entry("compute", "v1", true),
            entry("compute", "v1beta1", false),
            entry("storage", "v1", false),
            entry("storage", "v1beta2", false),
            entry("run", "v2", true),
            entry("run", "v1", false),
            entry("run", "v1alpha1", false),
        ];
        let result = select_latest_versions(items);
        assert_eq!(result.len(), 3);

        let by_name: HashMap<String, String> = result
            .into_iter()
            .map(|e| (e.name, e.version))
            .collect();
        assert_eq!(by_name["compute"], "v1");
        assert_eq!(by_name["storage"], "v1");
        assert_eq!(by_name["run"], "v2");
    }
}
