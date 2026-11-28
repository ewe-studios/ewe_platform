//! The tests of the Cloudflare drivers.

use std::sync::Arc;

use foundation_core::valtron::Stream;
use foundation_sandbox::shared::cloudflare::driver::{
    CloudflareContainerDriver, CloudflareDoDriver,
};
use foundation_sandbox::shared::cloudflare::transport::PortConnector;
use foundation_sandbox::shared::cloudflare::{
    port_error_from_text, CloudflareConfig, ContainerApi, ModelBackend, ModelConfig, PortError,
};
use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::control::SandboxState;
use foundation_sandbox::shared::driver::{
    CreateProgress, SandboxDriver, SlaveEndpoint, StoredRecord, Substrate,
};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::ids::DriverId;
use foundation_sandbox::shared::rpc::messages::ExecEventKind;
use foundation_sandbox::shared::spec::{HostMount, NetworkMode, RootFs, RootMode, RootSource};

use super::support::{self, block_on, spec, Harness};

/// The connector of a harness.
fn connector(h: &Harness) -> Arc<PortConnector> {
    Arc::new(PortConnector {
        container: h.container.clone(),
        config: Arc::clone(&h.host.config),
        do_slave: None,
    })
}

/// One stored ready record.
fn stored_ready(h: &Harness, state: SandboxState) -> StoredRecord {
    StoredRecord {
        id: h.id(),
        driver: DriverId::CloudflareContainer,
        spec: spec(),
        state,
        created_unix_ms: 0,
        expires_at_unix_ms: None,
        manifest: None,
        endpoint: Some(SlaveEndpoint::CloudflarePort { port: 1024 }),
    }
}

/// Runs one launch stream to its one result.
fn launch_result(
    driver: &dyn SandboxDriver,
    h: &Harness,
    sandbox_spec: foundation_sandbox::shared::spec::SandboxSpec,
) -> (
    Vec<foundation_sandbox::shared::driver::CreateProgress>,
    Option<foundation_sandbox::shared::error::SandboxResult<Substrate>>,
) {
    let deadline = h.ctx.clock().now() + std::time::Duration::from_secs(60);
    let mut stream = driver.launch(h.ctx.clone(), h.id(), sandbox_spec, deadline);
    let mut progresses = Vec::new();
    loop {
        match stream.next() {
            None => return (progresses, None),
            Some(Stream::Pending(progress)) => progresses.push(progress),
            Some(Stream::Next(result)) => return (progresses, Some(result)),
            Some(
                Stream::Ignore
                | Stream::Init
                | Stream::Wait
                | Stream::Delayed(_)
                | Stream::Spread(_),
            ) => {}
        }
    }
}

#[test]
fn create_starts_container() {
    let h = Harness::new(1, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let mut stream = planner.sandboxes().create_with_progress(spec());
    let mut progresses = Vec::new();
    let handle = loop {
        match stream.next() {
            None => panic!("the create ended without a handle"),
            Some(Stream::Pending(progress)) => progresses.push(progress),
            Some(Stream::Next(Ok(handle))) => break handle,
            Some(Stream::Next(Err(error))) => panic!("the create failed: {error:?}"),
            Some(
                Stream::Ignore
                | Stream::Init
                | Stream::Wait
                | Stream::Delayed(_)
                | Stream::Spread(_),
            ) => {}
        }
    };
    assert_eq!(handle.info().state, SandboxState::Ready);
    let options = h.container.start_options().expect("the container started");
    assert_eq!(
        options.entrypoint,
        vec![
            "/usr/local/bin/agent-slave".to_string(),
            "--listen".to_string(),
            "tcp:0.0.0.0:1024".to_string(),
        ]
    );
    assert!(!options.enable_internet);
    assert_eq!(
        progresses,
        vec![
            CreateProgress::Preparing,
            CreateProgress::SubstrateStarted,
            CreateProgress::SlaveConnected,
            CreateProgress::SlaveReady,
        ]
    );
    let stored: StoredRecord = serde_json::from_slice(
        &h.storage
            .value(foundation_sandbox::shared::cloudflare::store::SANDBOX_KEY)
            .expect("the record exists"),
    )
    .expect("the record is JSON");
    assert_eq!(stored.state, SandboxState::Ready);
    assert_eq!(h.index.len(), 1);
}

#[test]
fn host_network_enables_internet() {
    let h = Harness::new(2, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let mut sandbox_spec = spec();
    sandbox_spec.network = NetworkMode::Host;
    block_on(planner.sandboxes().create(sandbox_spec)).expect("create");
    assert!(
        h.container
            .start_options()
            .expect("started")
            .enable_internet
    );
}

#[test]
fn calls_are_port_fetches() {
    let h = Harness::new(3, true, false);
    h.scripted.with_exec_events(vec![
        ExecEventKind::Started { pid: 1 },
        ExecEventKind::Stdout {
            data: b"hi\n".to_vec(),
        },
        ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    ]);
    let planner = support::planner(&h, Some(h.id()));
    let sandbox = block_on(planner.sandboxes().create(spec())).expect("create");
    let output = block_on(sandbox.run(Command::new(["echo", "hi"]))).expect("run");
    assert_eq!(output.stdout, b"hi\n");
    assert_eq!(output.status.exit_code(), Some(0));
    let fetches = h.container.fetches();
    assert_ne!(fetches.len(), 0);
    assert!(fetches.iter().all(|port| *port == 1024));
}

#[test]
fn not_listening_retries() {
    let h = Harness::new(4, true, false);
    // The typed `Info` client retries one transport error, so a readiness wait
    // that gives up twice needs four failed fetches for one back-off.
    h.container.set_failures(support::not_listening(6));
    let planner = support::planner(&h, Some(h.id()));
    block_on(planner.sandboxes().create(spec())).expect("create");
    let waits = h.timer.waits();
    assert!(
        waits.contains(&std::time::Duration::from_millis(5)),
        "{waits:?}"
    );
    assert!(
        waits.contains(&std::time::Duration::from_millis(10)),
        "{waits:?}"
    );
    assert!(
        waits.contains(&std::time::Duration::from_millis(20)),
        "{waits:?}"
    );
}

#[test]
fn other_error_times_out() {
    let h = Harness::new(5, true, true);
    h.container
        .set_failures(vec![PortError::Other("internal error".to_string()); 1024]);
    let planner = support::planner(&h, Some(h.id()));
    let error = support::error_of(
        block_on(planner.sandboxes().create(spec())),
        "the create fails",
    );
    assert!(matches!(
        error,
        SandboxError::BootTimeout { stage, .. } if stage == "slave"
    ));
    assert!(h.container.destroy_count() > 0);
    assert_eq!(h.storage.value("sandbox"), None);
    assert!(h.index.is_empty());
}

#[test]
fn not_listening_text_match() {
    assert_eq!(
        port_error_from_text("the container is not listening", 64 * 1024),
        PortError::NotListening
    );
    assert!(matches!(
        port_error_from_text("internal error", 64 * 1024),
        PortError::Other(_)
    ));
}

#[test]
fn driver_rejects() {
    let h = Harness::new(6, true, false);
    let driver = CloudflareContainerDriver::new(
        h.host.clone(),
        connector(&h) as Arc<dyn foundation_sandbox::shared::host::SlaveConnector>,
    );
    let mut other_image = spec();
    other_image.rootfs = RootFs::ContainerImage("other".to_string());
    let (_, result) = launch_result(&driver, &h, other_image);
    assert!(matches!(
        support::error_of(result.expect("a result"), "refused"),
        SandboxError::UnsupportedRootFs
    ));

    let mut mounted = spec();
    mounted.mounts = vec![HostMount {
        host_path: std::path::PathBuf::from("/tmp"),
        guest_path: "/mnt".to_string(),
        read_only: true,
    }];
    let (_, result) = launch_result(&driver, &h, mounted);
    assert!(matches!(
        support::error_of(result.expect("a result"), "refused"),
        SandboxError::UnsupportedMount
    ));

    let mut read_only = spec();
    read_only.root_mode = RootMode::ReadOnly;
    let (_, result) = launch_result(&driver, &h, read_only);
    assert!(matches!(
        support::error_of(result.expect("a result"), "refused"),
        SandboxError::InvalidSpec { .. }
    ));
    assert_eq!(h.container.start_count(), 0);
}

#[test]
fn driver_ignores_cpu_and_memory() {
    let h = Harness::new(7, true, false);
    let planner = support::planner(&h, Some(h.id()));
    let mut sandbox_spec = spec();
    sandbox_spec.vcpus = 8;
    sandbox_spec.memory_mib = 4096;
    block_on(planner.sandboxes().create(sandbox_spec)).expect("create");
    assert_eq!(h.container.start_count(), 1);
}

#[test]
fn attach_running() {
    let h = Harness::new(8, true, false);
    let driver = CloudflareContainerDriver::new(
        h.host.clone(),
        connector(&h) as Arc<dyn foundation_sandbox::shared::host::SlaveConnector>,
    );
    h.container
        .start(foundation_sandbox::shared::cloudflare::ContainerStart {
            entrypoint: h.host.config.slave_entrypoint(),
            enable_internet: false,
            env: Vec::new(),
        })
        .expect("start");
    let attached = block_on(driver.attach(h.ctx.clone(), &stored_ready(&h, SandboxState::Ready)))
        .expect("attach");
    assert!(attached.is_some());
}

#[test]
fn attach_not_running() {
    let h = Harness::new(9, true, false);
    let driver = CloudflareContainerDriver::new(
        h.host.clone(),
        connector(&h) as Arc<dyn foundation_sandbox::shared::host::SlaveConnector>,
    );
    let attached = block_on(driver.attach(h.ctx.clone(), &stored_ready(&h, SandboxState::Ready)))
        .expect("attach");
    assert!(attached.is_none());
}

#[test]
fn do_driver_launch() {
    let h = Harness::new(10, true, false);
    let config = CloudflareConfig::new().with_model(Some(ModelConfig::new().with_backend(Some(
        ModelBackend::CloudflareAi {
            model: "test".to_string(),
        },
    ))));
    let driver = CloudflareDoDriver::new(&config);
    let (progresses, result) = launch_result(&driver, &h, spec());
    let substrate = result.expect("a result").expect("the launch works");
    assert_eq!(
        substrate.endpoint,
        SlaveEndpoint::CloudflareDo {
            binding: "SLAVE".to_string(),
            name: h.id(),
        }
    );
    assert_eq!(substrate.root.source, RootSource::Plain);
    assert_eq!(substrate.root.mode, RootMode::Overlay);
    assert_eq!(h.container.start_count(), 0);
    assert_ne!(progresses.len(), 0);
}

#[test]
fn do_driver_attach() {
    let h = Harness::new(11, true, false);
    let config = CloudflareConfig::new().with_model(Some(ModelConfig::new().with_backend(Some(
        ModelBackend::CloudflareAi {
            model: "test".to_string(),
        },
    ))));
    let driver = CloudflareDoDriver::new(&config);
    let mut record = stored_ready(&h, SandboxState::Ready);
    record.driver = DriverId::CloudflareDo;
    record.endpoint = Some(SlaveEndpoint::CloudflareDo {
        binding: "SLAVE".to_string(),
        name: h.id(),
    });
    let attached = block_on(driver.attach(h.ctx.clone(), &record))
        .expect("attach")
        .expect("the record is attachable");
    assert_eq!(attached.endpoint, record.endpoint.expect("endpoint"));
    assert!(attached.handle.container().is_none());
    assert!(driver.containers().is_none());
}

#[test]
fn do_driver_no_container_control() {
    let h = Harness::new(12, true, false);
    let config = CloudflareConfig::new();
    let driver = CloudflareDoDriver::new(&config);
    let (_, result) = launch_result(&driver, &h, spec());
    let error = support::error_of(result.expect("a result"), "there is no backend");
    assert!(matches!(error, SandboxError::UnsupportedCapability { .. }));
    assert!(driver.containers().is_none());
}

/// The configuration of one sandbox Durable Object carries the model values of
/// the Worker, so a model spec reaches the model driver (issue #1142).
#[test]
fn sandbox_config_carries_the_model() {
    let model = ModelConfig::new().with_backend(Some(ModelBackend::CloudflareAi {
        model: "test".to_string(),
    }));
    let config = CloudflareConfig::for_sandbox("abc", model.clone());
    assert_eq!(config.name, "abc");
    assert_eq!(config.model, Some(model));
}

/// A cleanup must not ask a container that is already gone whether it runs.
///
/// The Cloudflare host throws `Critical error` from `running` after the
/// container stopped, which took the whole Durable Object down (issue #1125).
/// The handle knows that it destroyed the container, so it destroys at most
/// once and never reads the host state.
#[test]
fn cleanup_does_not_read_a_gone_container() {
    let h = Harness::new(30, true, false);
    let driver = CloudflareContainerDriver::new(
        h.host.clone(),
        connector(&h) as Arc<dyn foundation_sandbox::shared::host::SlaveConnector>,
    );
    let (_, result) = launch_result(&driver, &h, spec());
    let substrate = result.expect("a result").expect("the launch works");
    let handle = substrate.handle;
    h.container.panic_on_running();
    block_on(handle.cleanup()).expect("the cleanup succeeds");
    assert!(
        h.container.destroy_count() > 0,
        "the cleanup destroys the container"
    );
}

/// A cleanup retries a destroy that the kill could not finish.
///
/// A failed destroy must not mark the container as destroyed. The cleanup is
/// the second chance; if it skips the destroy, the container leaks (issue
/// #1125).
#[test]
fn cleanup_retries_a_failed_destroy() {
    let h = Harness::new(31, true, false);
    let driver = CloudflareContainerDriver::new(
        h.host.clone(),
        connector(&h) as Arc<dyn foundation_sandbox::shared::host::SlaveConnector>,
    );
    let (_, result) = launch_result(&driver, &h, spec());
    let substrate = result.expect("a result").expect("the launch works");
    let handle = substrate.handle;
    h.container.fail_next_destroy("the runtime refused the destroy");
    block_on(handle.kill()).expect("the kill answers");
    assert_eq!(h.container.destroy_count(), 1, "the kill tries once");
    block_on(handle.cleanup()).expect("the cleanup succeeds");
    assert_eq!(
        h.container.destroy_count(),
        2,
        "the cleanup tries the destroy again"
    );
}

/// A cleanup does not destroy a container that a stop already destroyed.
#[test]
fn cleanup_does_not_destroy_a_container_twice() {
    let h = Harness::new(32, true, false);
    let driver = CloudflareContainerDriver::new(
        h.host.clone(),
        connector(&h) as Arc<dyn foundation_sandbox::shared::host::SlaveConnector>,
    );
    let (_, result) = launch_result(&driver, &h, spec());
    let substrate = result.expect("a result").expect("the launch works");
    let handle = substrate.handle;
    block_on(handle.kill()).expect("the kill answers");
    block_on(handle.cleanup()).expect("the cleanup succeeds");
    assert_eq!(
        h.container.destroy_count(),
        1,
        "the cleanup leaves the destroyed container alone"
    );
}

/// The container call deadline has a documented default and a builder.
#[test]
fn call_timeout_configuration() {
    let config = CloudflareConfig::new();
    assert_eq!(config.call_timeout, std::time::Duration::from_secs(30));
    assert_eq!(
        config
            .with_call_timeout(std::time::Duration::from_secs(7))
            .call_timeout,
        std::time::Duration::from_secs(7)
    );
}
