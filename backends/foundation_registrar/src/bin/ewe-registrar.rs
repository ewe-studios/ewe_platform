//! The `ewe-registrar` command-line interface.
//!
//! WHY: one binary runs in two modes, host and container (spec 82 feature 01,
//! section 5). This file is the only place that builds the real side effects:
//! the host `Ctx`, the process runner, the Docker client and the signal
//! handlers.
//!
//! WHAT: flags and `EWE_REGISTRAR_*` environment variables, the enrollment and
//! mTLS connection, the discovery adapters, the prober and the reconcile loop.
//!
//! HOW: `#[valtron]` supplies the executor. SIGHUP reloads the manifest;
//! SIGTERM or SIGINT stops the loop. A superseded session exits with code 3,
//! so systemd does not start two sidecars that fight (section 6.10).

use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::valtron;
use foundation_core::valtron::valtron;
use foundation_deployment_docker::DockerClient;
use foundation_nativeapis::signal::{signal_task, SignalKind};
use foundation_registrar::{
    CgroupMetrics, CommandRunner, CompositeMetrics, DiscoverySet, DockerContainerApi,
    DockerDiscovery, DockerMetrics, Reconciler, RegistrarConfig, RegistrarContext, RegistrarError,
    RegistrationClient, SystemCommandRunner, SystemdDiscovery, UnavailableDocker,
};

/// Exit code for a bad configuration or an enrollment failure that needs an
/// operator.
const EXIT_USAGE: u8 = 2;
/// Exit code for a session that a newer sidecar replaced.
const EXIT_SUPERSEDED: u8 = 3;

#[valtron]
async fn main() {
    let config = match RegistrarConfig::from_args(std::env::args_os()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("ewe-registrar: {error}");
            std::process::exit(i32::from(EXIT_USAGE));
        }
    };
    let code = run(config).await;
    std::process::exit(i32::from(code));
}

#[allow(
    clippy::too_many_lines,
    reason = "the host entry wires every part once, in start-up order"
)]
async fn run(config: RegistrarConfig) -> u8 {
    let ctx = Ctx::host();
    if let Err(error) = foundation_core::env::install(&ctx, foundation_core::env::system) {
        eprintln!("ewe-registrar: cannot install the host Env: {error}");
        return EXIT_USAGE;
    }
    let config = Arc::new(config);
    let client = match RegistrationClient::connect(&config, &ctx).await {
        Ok(client) => Arc::new(client),
        Err(error) => {
            eprintln!("ewe-registrar: {error}");
            return EXIT_USAGE;
        }
    };
    let commands: Arc<dyn CommandRunner> = Arc::new(SystemCommandRunner);
    let docker = DockerClient::connect_with_defaults().ok().map(Arc::new);
    let context = RegistrarContext::new(
        ctx.clone(),
        Arc::clone(&config),
        Arc::clone(&commands),
        docker.clone(),
    );

    let systemd = SystemdDiscovery::host().unwrap_or_else(|error| {
        tracing::warn!(%error, "systemd discovery needs /proc/stat; the host has no systemd");
        SystemdDiscovery::with_boot_time(0)
    });
    let docker_discovery: Arc<dyn foundation_registrar::discovery::docker::ContainerApi> =
        match &docker {
            Some(client) => Arc::new(DockerContainerApi::new(Arc::clone(client))),
            None => Arc::new(UnavailableDocker),
        };
    let discovery = DiscoverySet::new(
        Arc::new(systemd),
        Arc::new(DockerDiscovery::new(
            docker_discovery,
            config.docker_network.clone(),
        )),
    );
    let prober = foundation_registrar::host_prober(ctx.clone(), Arc::clone(&commands), &config);
    let metrics = CompositeMetrics::new(
        Some(Arc::new(CgroupMetrics::host())),
        docker.as_ref().map(|client| {
            Arc::new(DockerMetrics::new(Arc::new(DockerContainerApi::new(
                Arc::clone(client),
            ))))
        }),
    );
    let mut reconciler = Reconciler::new(
        context,
        Arc::new(discovery),
        prober,
        Arc::new(metrics),
        client,
    );

    let (signal_task, bus) = match signal_task(ctx.clone()) {
        Ok(pair) => pair,
        Err(error) => {
            eprintln!("ewe-registrar: cannot install the signal handlers: {error}");
            return EXIT_USAGE;
        }
    };
    if let Err(error) = valtron::send(signal_task) {
        eprintln!("ewe-registrar: cannot start the signal task: {error}");
        return EXIT_USAGE;
    }
    let signals = bus.subscribe();

    loop {
        let mut stop = false;
        let mut reload = false;
        while let Some(event) = signals.try_iter().next() {
            match event.kind {
                SignalKind::Hangup => reload = true,
                SignalKind::Interrupt | SignalKind::Terminate | SignalKind::Quit => stop = true,
            }
        }
        if reload {
            tracing::info!("SIGHUP: reading the manifest again");
            reconciler.reload(true);
        }
        if stop {
            break;
        }
        match reconciler.step().await {
            Ok(()) => {}
            Err(RegistrarError::Superseded(host_id)) => {
                eprintln!(
                    "ewe-registrar: another sidecar owns host_id {host_id}; this process stops"
                );
                return EXIT_SUPERSEDED;
            }
            Err(error) => {
                tracing::warn!(%error, "reconcile step failed; retrying with backoff");
                let delay = reconciler.retry_delay();
                wait(&ctx, delay, &|| !signals.is_empty()).await;
                continue;
            }
        }
        let interval = reconciler.report_interval();
        wait(&ctx, interval, &|| !signals.is_empty()).await;
        if let Some(event) = signals.try_iter().next() {
            match event.kind {
                SignalKind::Hangup => {
                    reconciler.reload(true);
                }
                SignalKind::Interrupt | SignalKind::Terminate | SignalKind::Quit => break,
            }
        }
    }

    if let Err(error) = reconciler.shutdown().await {
        tracing::warn!(%error, "deregister at stop failed");
    }
    0
}

/// Wait for `total`, in slices, so a signal ends the wait quickly. A signal
/// stays in the queue for the caller to read.
async fn wait(ctx: &Ctx, total: Duration, has_signal: &(dyn Fn() -> bool + Send + Sync)) {
    let slice = Duration::from_millis(100);
    let mut waited = Duration::ZERO;
    while waited < total {
        if has_signal() {
            return;
        }
        let step = slice.min(total.saturating_sub(waited));
        foundation_core::valtron::sleep_async(step, ctx).await;
        waited += step;
    }
}
