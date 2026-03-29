//! WHY: Fetches OpenAPI specifications from multiple deployment providers
//! and distills them into versioned JSON snapshots.
//!
//! WHAT: A CLI subcommand that pulls specs from providers like Fly.io, GCP,
//! Cloudflare, Neon, Stripe, etc., normalizes responses, and writes them
//! to git-tracked `distilled-spec-*` repositories.
//!
//! HOW: Uses `foundation_core::wire::simple_http::client::SimpleHttpClient` for HTTP requests,
//! Valtron's `execute_collect_all` for parallel fetch execution, `serde_json` for parsing,
//! and blocking `std::fs` for file I/O at sync boundaries (after Valtron execution).

pub mod core;
pub mod errors;
pub mod fetcher;
pub mod providers;

use clap::{ArgMatches, Command};
use foundation_core::valtron;
use foundation_core::wire::simple_http::client::SimpleHttpClient;
use std::path::PathBuf;
use std::time::Duration;

pub use core::DistilledSpec;
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

/// Run the `gen_provider_specs` command.
///
/// # Panics
///
/// Panics if Valtron pool initialization fails.
pub fn run(matches: &ArgMatches) -> Result<(), SpecFetchError> {
    let specs_base: PathBuf = matches
        .get_one::<String>("output")
        .map(PathBuf::from)
        .unwrap();

    // Initialize Valtron pool - keep guard alive for duration of function
    let _guard = valtron::initialize_pool(100, None);

    let fetcher = ProviderSpecFetcher::new(specs_base);

    // Create HTTP client with system defaults
    let mut client = SimpleHttpClient::from_system()
        .max_body_size(None)
        .batch_size(8192 * 2)
        .read_timeout(Duration::from_secs(30))
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
    let force = matches.get_flag("force");

    let specs = fetcher.fetch_all(client)?;

    for (provider, spec) in specs {
        let repo_name = format!("distilled-spec-{provider}");

        if !dry_run {
            // Check for changes
            if force || fetcher.has_changed(&repo_name, &spec.content_hash)? {
                fetcher.write_spec(&spec, &repo_name)?;
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

fn fetch_single_provider(
    fetcher: &ProviderSpecFetcher,
    client: &mut SimpleHttpClient,
    provider: &str,
    matches: &ArgMatches,
) -> Result<(), SpecFetchError> {
    let dry_run = matches.get_flag("dry-run");

    let spec = fetcher.fetch_single(client, provider)?;
    let repo_name = format!("distilled-spec-{provider}");

    if !dry_run {
        fetcher.write_spec(&spec, &repo_name)?;
        println!("Updated: {provider} -> {repo_name}/specs/");
    } else {
        println!("Would update: {provider} (dry-run)");
    }

    Ok(())
}
