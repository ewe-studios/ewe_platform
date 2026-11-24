//! Discovery tests (spec 82 feature 01, T3 and T4).

mod support;

use std::sync::Arc;

use foundation_registrar::{
    Discovered, Discovery, DockerDiscovery, RegistrarConfig, RegistrarContext, RunHandle,
    SystemdDiscovery,
};
use support::{entry, FakeCommands, FakeContainerApi};

fn context(commands: Arc<FakeCommands>) -> RegistrarContext {
    RegistrarContext::new(
        support::Sim::new().ctx,
        Arc::new(RegistrarConfig::new(
            foundation_registrar::RegistrarMode::Host,
            "host-17",
        )),
        commands,
        None,
    )
}

fn systemd_entry() -> foundation_registrar::EntryView {
    entry("agent-rpc", "0.4.0", "systemd", None, None)
}

#[test]
fn a_unit_without_a_mark_is_not_installed() {
    let commands = Arc::new(FakeCommands::answering("LoadState=not-found\n"));
    let discovery = SystemdDiscovery::with_boot_time(1_000_000);
    let ctx = context(commands.clone());
    let found = futures_lite::future::block_on(discovery.discover(&systemd_entry(), &ctx))
        .expect("discover");
    assert_eq!(found, Discovered::NotInstalled);
    let calls = commands.calls();
    assert_eq!(calls[0].0, "systemctl");
    assert_eq!(calls[0].1[0], "show");
    assert_eq!(calls[0].1[1], "ewe-agent-rpc.service");
    assert_eq!(calls[0].1[2], "-p");
    assert!(calls[0].1[3].contains("ExecMainStartTimestampMonotonic"));
}

#[test]
fn a_stopped_unit_reports_its_version() {
    let stdout = "LoadState=loaded\nActiveState=inactive\nMainPID=0\n\
                  Environment=EWE_ENTRY=agent-rpc EWE_VERSION=0.4.0\nNRestarts=0\n\
                  ExecMainStartTimestampMonotonic=0\n";
    let discovery = SystemdDiscovery::with_boot_time(1_000_000);
    let ctx = context(Arc::new(FakeCommands::answering(stdout)));
    let found = futures_lite::future::block_on(discovery.discover(&systemd_entry(), &ctx))
        .expect("discover");
    assert_eq!(
        found,
        Discovered::Stopped {
            version: "0.4.0".to_string()
        }
    );
}

#[test]
fn a_running_unit_reports_its_process_and_start_time() {
    let stdout = "LoadState=loaded\nActiveState=active\nMainPID=4242\n\
                  Environment=EWE_ENTRY=agent-rpc EWE_VERSION=0.4.0\nNRestarts=2\n\
                  ExecMainStartTimestampMonotonic=5000000\n";
    let discovery = SystemdDiscovery::with_boot_time(1_000_000);
    let ctx = context(Arc::new(FakeCommands::answering(stdout)));
    let found = futures_lite::future::block_on(discovery.discover(&systemd_entry(), &ctx))
        .expect("discover");
    assert_eq!(
        found,
        Discovered::Running {
            version: "0.4.0".to_string(),
            handle: RunHandle::Process {
                pid: 4242,
                unit: "ewe-agent-rpc.service".to_string()
            },
            started_at_unix_ms: 1_005_000,
            restarts: 2,
            container_ip: None,
        }
    );
}

#[test]
fn a_unit_for_another_entry_is_not_this_entry() {
    let stdout = "LoadState=loaded\nActiveState=active\nMainPID=1\n\
                  Environment=EWE_ENTRY=other-app EWE_VERSION=1\nNRestarts=0\n\
                  ExecMainStartTimestampMonotonic=0\n";
    let discovery = SystemdDiscovery::with_boot_time(0);
    let ctx = context(Arc::new(FakeCommands::answering(stdout)));
    let found = futures_lite::future::block_on(discovery.discover(&systemd_entry(), &ctx))
        .expect("discover");
    assert_eq!(found, Discovered::NotInstalled);
}

#[test]
fn a_missing_systemctl_is_a_discovery_error() {
    let discovery = SystemdDiscovery::with_boot_time(0);
    let ctx = context(Arc::new(FakeCommands::failing()));
    let error = futures_lite::future::block_on(discovery.discover(&systemd_entry(), &ctx))
        .expect_err("a missing program is an error");
    assert!(error.to_string().contains("systemctl") || error.to_string().contains("absent"));
}

fn docker_entry() -> foundation_registrar::EntryView {
    entry("vector-db", "1.9.0", "docker", None, None)
}

fn docker_api_with(list: serde_json::Value, inspect: serde_json::Value) -> Arc<FakeContainerApi> {
    let api = Arc::new(FakeContainerApi::new());
    api.set_list(list);
    api.set_inspect(inspect);
    api
}

fn inspect(running: bool, entry_label: &str, ip: &str) -> serde_json::Value {
    serde_json::json!({
        "Id": "container-1",
        "Config": {
            "Labels": {
                "dev.ewe.entry": entry_label,
                "dev.ewe.version": "1.9.0"
            }
        },
        "State": {
            "Running": running,
            "StartedAt": "2026-11-23T10:00:00.000000000Z"
        },
        "RestartCount": 3,
        "NetworkSettings": {
            "Networks": {
                "ewe": { "IPAddress": ip }
            }
        }
    })
}

#[test]
fn the_docker_list_uses_the_entry_label() {
    let api = docker_api_with(
        serde_json::json!([{ "Id": "container-1", "State": "running" }]),
        inspect(true, "vector-db", "172.18.0.4"),
    );
    let discovery = DockerDiscovery::new(api.clone(), Some("ewe".to_string()));
    let ctx = context(Arc::new(FakeCommands::answering("")));
    let found = futures_lite::future::block_on(discovery.discover(&docker_entry(), &ctx))
        .expect("discover");
    assert_eq!(api.labels(), vec!["dev.ewe.entry=vector-db".to_string()]);
    let Discovered::Running {
        version,
        handle,
        restarts,
        container_ip,
        ..
    } = found
    else {
        panic!("expected a running container, got {found:?}");
    };
    assert_eq!(version, "1.9.0");
    assert_eq!(
        handle,
        RunHandle::Container {
            id: "container-1".into()
        }
    );
    assert_eq!(restarts, 3);
    assert_eq!(container_ip, Some("172.18.0.4".parse().expect("ip")));
}

#[test]
fn an_empty_docker_list_is_not_installed() {
    let api = docker_api_with(
        serde_json::json!([]),
        inspect(true, "vector-db", "172.18.0.4"),
    );
    let discovery = DockerDiscovery::new(api, None);
    let ctx = context(Arc::new(FakeCommands::answering("")));
    let found = futures_lite::future::block_on(discovery.discover(&docker_entry(), &ctx))
        .expect("discover");
    assert_eq!(found, Discovered::NotInstalled);
}

#[test]
fn a_stopped_container_reports_its_version() {
    let api = docker_api_with(
        serde_json::json!([{ "Id": "container-1", "State": "exited" }]),
        inspect(false, "vector-db", ""),
    );
    let discovery = DockerDiscovery::new(api, None);
    let ctx = context(Arc::new(FakeCommands::answering("")));
    let found = futures_lite::future::block_on(discovery.discover(&docker_entry(), &ctx))
        .expect("discover");
    assert_eq!(
        found,
        Discovered::Stopped {
            version: "1.9.0".to_string()
        }
    );
}

#[test]
fn a_container_with_another_entry_label_is_not_this_entry() {
    let api = docker_api_with(
        serde_json::json!([{ "Id": "container-1", "State": "running" }]),
        inspect(true, "another-app", "172.18.0.4"),
    );
    let discovery = DockerDiscovery::new(api, None);
    let ctx = context(Arc::new(FakeCommands::answering("")));
    let found = futures_lite::future::block_on(discovery.discover(&docker_entry(), &ctx))
        .expect("discover");
    assert_eq!(found, Discovered::NotInstalled);
}

#[test]
fn an_unknown_deploy_kind_is_a_discovery_error() {
    let discovery = foundation_registrar::DiscoverySet::new(
        Arc::new(SystemdDiscovery::with_boot_time(0)),
        Arc::new(DockerDiscovery::new(
            Arc::new(FakeContainerApi::new()),
            None,
        )),
    );
    let ctx = context(Arc::new(FakeCommands::answering("")));
    let entry = entry("odd", "1", "carrier-pigeon", None, None);
    let error =
        futures_lite::future::block_on(discovery.discover(&entry, &ctx)).expect_err("unknown kind");
    assert!(error.to_string().contains("unknown deploy kind"));
}

#[test]
fn a_quoted_systemd_environment_is_parsed() {
    let stdout = "LoadState=loaded\nActiveState=active\nMainPID=7\n\
                  Environment=\"EWE_ENTRY=agent-rpc\" \"EWE_VERSION=0.4.0\"\n\
                  NRestarts=0\nExecMainStartTimestampMonotonic=0\n";
    let discovery = SystemdDiscovery::with_boot_time(0);
    let ctx = context(Arc::new(FakeCommands::answering(stdout)));
    let found = futures_lite::future::block_on(discovery.discover(&systemd_entry(), &ctx))
        .expect("discover");
    assert!(matches!(
        found,
        Discovered::Running { version, .. } if version == "0.4.0"
    ));
}
