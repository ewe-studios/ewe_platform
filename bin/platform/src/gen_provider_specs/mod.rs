//! WHY: Fetches OpenAPI specifications from multiple deployment providers
//! and stores them as raw JSON snapshots.
//!
//! WHAT: A CLI subcommand that pulls specs from providers like Fly.io, GCP,
//! Cloudflare, Neon, Stripe, etc., and saves them to `artefacts/cloud_providers`.
//!
//! HOW: Uses `foundation_core::wire::simple_http::client::SimpleHttpClient` for HTTP requests,
//! Valtron's `execute` for parallel fetch execution, `serde_json` for parsing,
//! and blocking `std::fs` for file I/O at sync boundaries (after Valtron execution).

pub mod core;
pub mod errors;
pub mod fetcher;
pub mod providers;

use clap::{ArgMatches, Command};
use foundation_core::valtron;
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use std::time::Duration;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

pub use errors::SpecFetchError;
pub use fetcher::ProviderSpecFetcher;

/// Register the `gen_provider_specs` subcommand.
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
                clap::Arg::new("gcp-apis")
                    .long("gcp-apis")
                    .help("Comma-separated list of GCP API names to fetch (default: all APIs)")
                    .value_name("APIS"),
            )
            .arg(
                clap::Arg::new("dry-run")
                    .long("dry-run")
                    .help("Fetch specs but don't write to disk")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("force")
                    .long("force")
                    .help("Write specs even if content hasn't changed")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("debug")
                    .long("debug")
                    .default_value("false")
                    .action(clap::ArgAction::SetTrue)
                    .help("Enables debug logs (default: false)"),
            ),
    )
}

/// Run the `gen_provider_specs` command.
///
/// # Panics
///
/// Panics if Valtron pool initialization fails.
pub fn run(matches: &ArgMatches) -> Result<(), SpecFetchError> {
    let logging_level = if matches.get_flag("debug") {
        Level::DEBUG
    } else {
        Level::INFO
    };

    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_max_level(logging_level)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    // Initialize Valtron pool - keep guard alive for duration of function
    let _guard = valtron::initialize_pool(100, None);

    let fetcher = ProviderSpecFetcher::new();

    // Create HTTP client with system defaults
    // Enable non-blocking TLS handshake for GCP compatibility
    let mut client = SimpleHttpClient::from_system()
        .max_body_size(None)
        .batch_size(8192 * 2)
        .read_timeout(Duration::from_secs(5))
        .max_retries(3)
        .enable_pool(10);

    if let Some(provider) = matches.get_one::<String>("provider") {
        // Fetch single provider
        fetch_single_provider(&fetcher, &mut client, provider, matches)
    } else {
        // Fetch all providers
        fetch_all_providers(&fetcher, &mut client, matches)
    }
}

fn fetch_all_providers(
    fetcher: &ProviderSpecFetcher,
    client: &mut SimpleHttpClient,
    matches: &ArgMatches,
) -> Result<(), SpecFetchError> {
    let dry_run = matches.get_flag("dry-run");

    // Parse GCP API filter if provided
    let gcp_api_filter = matches
        .get_one::<String>("gcp-apis")
        .map(|s| s.split(',').map(|api| api.trim().to_string()).collect());

    let specs = fetcher.fetch_all(client, gcp_api_filter)?;

    for (provider, _spec) in specs {
        if !dry_run {
            tracing::info!(
                "Fetched: {provider} -> artefacts/cloud_providers/{provider}/openapi.json"
            );
        } else {
            tracing::info!("Would fetch: {provider} (dry-run)");
        }
    }

    Ok(())
}

fn fetch_single_provider(
    fetcher: &ProviderSpecFetcher,
    client: &mut SimpleHttpClient,
    provider: &str,
    matches: &ArgMatches,
) -> Result<(), SpecFetchError> {
    let dry_run = matches.get_flag("dry-run");

    // Parse GCP API filter if provided (only used when fetching GCP)
    let gcp_api_filter = matches
        .get_one::<String>("gcp-apis")
        .map(|s| s.split(',').map(|api| api.trim().to_string()).collect());

    let _spec = fetcher.fetch_single(client, provider, gcp_api_filter)?;

    if !dry_run {
        tracing::info!("Fetched: {provider} -> artefacts/cloud_providers/{provider}/openapi.json");
    } else {
        tracing::info!("Would fetch: {provider} (dry-run)");
    }

    Ok(())
}
