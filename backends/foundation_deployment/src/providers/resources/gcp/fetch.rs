//! GCP Discovery Service spec fetcher.
//!
//! WHY: GCP uses a two-stage fetch - first the Discovery directory,
//! then individual API discovery documents.
//!
//! WHAT: Fetches the API directory, then fetches ALL API specs in parallel.
//!
//! HOW: Uses combinators to chain: directory fetch → create API tasks →
//! `collect_all_streams` → write output. No blocking, no from_future.

use crate::error::DeploymentError;
use foundation_core::valtron::{
    collect_all_streams, execute, one_shot, Stream, StreamIterator, StreamIteratorExt,
    TaskIteratorExt, TaskShortCircuit, TaskStatus,
};
use foundation_core::wire::simple_http::client::{
    body_reader, ClientConfig, HttpConnectionPool, RequestIntro, SendRequestTask, SimpleHttpClient,
    SystemDnsResolver,
};
use serde_json::Value;
use std::path::{Path, PathBuf};
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

/// List of relevant GCP API names to fetch (filters down from 510+ APIs).
/// Add more APIs here as needed for your use case.
const RELEVANT_GCP_APIS: &[&str] = &[
    // Core infrastructure - most commonly used
    "compute", // Compute Engine
              // "container", // GKE
              // "iam",        // IAM
              // "logging",    // Cloud Logging
              // "monitoring", // Cloud Monitoring
              // "storage",    // Cloud Storage
];

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
/// Fetch ALL GCP specs using two-stage approach with combinators.
///
/// # Arguments
///
/// * `client` - The HTTP client to use for fetching API specs
/// * `output_dir` - Directory where fetched specs will be written
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
) -> Result<
    impl StreamIterator<D = Result<PathBuf, DeploymentError>, P = GcpFetchPending> + Send + 'static,
    DeploymentError,
> {
    let pool = client.client_pool().expect("should have pool");
    let config = client.client_config();

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
                debug!("gcp: Found {} APIs in directory", total_apis);

                // Filter to only relevant APIs
                let filtered_items: Vec<GcpApiEntry> = directory
                    .items
                    .into_iter()
                    .filter(|item| RELEVANT_GCP_APIS.contains(&item.name.as_str()))
                    .collect();

                info!(
                    "Found {} GCP APIs in directory, filtered to {} relevant APIs",
                    total_apis,
                    filtered_items.len()
                );

                // Create fetch streams for all APIs - execute each task immediately
                let mut streams: Vec<Box<dyn StreamIterator<D = _, P = _, Item = _> + Send>> =
                    Vec::new();

                for entry in filtered_items {
                    match create_api_fetch_task(entry, pool.clone(), config.clone()) {
                        Ok(task) => match execute(task, None) {
                            Ok(stream) => streams.push(Box::new(stream)),
                            Err(e) => warn!("Failed to execute fetch task: {}", e),
                        },
                        Err(e) => warn!("Failed to create fetch task: {}", e),
                    }
                }

                let total_tasks = streams.len();
                let output_dir = output_dir.clone();

                // Collect all API fetches and write output
                Box::new(
                    collect_all_streams(streams)
                        .map_done(move |results| {
                            let mut specs = Vec::new();
                            for result in results {
                                if let Some(Ok((entry, spec))) = result {
                                    info!("  Loaded: {} ({})", entry.name, entry.version);
                                    specs.push((entry, spec));
                                } else if let Some(Err(e)) = result {
                                    warn!("Spec fetch failed for all streams: {}", e);
                                }
                            }

                            info!("Fetched {} API specs, writing output", specs.len());
                            write_output(&output_dir, total_tasks, &specs)
                        })
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
fn create_api_fetch_task(
    entry: GcpApiEntry,
    pool: Arc<HttpConnectionPool<SystemDnsResolver>>,
    config: ClientConfig,
) -> Result<
    impl foundation_core::valtron::TaskIterator<
            Ready = Option<Result<(GcpApiEntry, Value), DeploymentError>>,
            Pending = usize,
            Spawner = foundation_core::valtron::BoxedSendExecutionAction,
        > + Send
        + 'static,
    DeploymentError,
> {
    let request = SimpleHttpClient::new(config.clone(), pool.clone())
        .get(&entry.discovery_rest_url)
        .map_err(|e| DeploymentError::Generic(format!("Failed to build request: {e}")))?
        .build()
        .map_err(|e| DeploymentError::Generic(format!("Failed to build request: {e}")))?;

    let name = entry.name.clone();
    let task = SendRequestTask::new(request, 5, pool, config)
        .map_ready(move |intro| match intro {
            RequestIntro::Success { stream, .. } => {
                debug!("gcp/{}: Response received", name);
                let body = body_reader::collect_string(stream);
                debug!("gcp/{}: Body length: {}", name, body.len());
                debug!("gcp/{}: Body: {}", name, &body);

                let output_path = Path::new("response_api.json");
                std::fs::write(output_path, &body).expect("write out to disk");

                match serde_json::from_str::<Value>(body.trim()) {
                    Ok(spec) => {
                        debug!("gcp/{}/{}: JSON parsed", name, entry.discovery_rest_url);
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

    Ok(task)
}

fn write_output(
    output_dir: &PathBuf,
    total_apis: usize,
    specs: &[(GcpApiEntry, Value)],
) -> Result<PathBuf, DeploymentError> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| DeploymentError::Generic(format!("Failed to create output directory: {e}")))?;

    let mut consolidated = serde_json::Map::new();
    let mut spec_names = Vec::new();

    for (entry, spec) in specs {
        consolidated.insert(entry.id.clone(), spec.clone());
        spec_names.push(format!("{}-{}.json", entry.name, entry.version));
    }

    let output_path = output_dir.join("openapi.json");
    let json = serde_json::to_string_pretty(&Value::Object(consolidated))
        .map_err(|e| DeploymentError::Generic(format!("Failed to serialize JSON: {e}")))?;

    std::fs::write(&output_path, json)
        .map_err(|e| DeploymentError::Generic(format!("Failed to write output file: {e}")))?;

    let manifest = serde_json::json!({
        "source": GCP_DISCOVERY_URL,
        "fetched_at": chrono::Utc::now().to_rfc3339(),
        "total_apis": total_apis,
        "fetched": specs.len(),
        "apis": specs.iter().zip(spec_names.iter()).map(|((entry, _), filename)| serde_json::json!({
            "filename": filename,
            "id": &entry.id,
            "name": &entry.name,
            "version": &entry.version,
            "title": &entry.title,
            "preferred": entry.preferred,
        })).collect::<Vec<_>>(),
    });

    let manifest_path = output_dir.join("_manifest.json");
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)
        .map_err(|e| DeploymentError::Generic(format!("Failed to write manifest: {e}")))?;

    info!("GCP spec saved to: {}", output_path.display());
    Ok(output_path)
}
