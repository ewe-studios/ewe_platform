//! End-to-end tests behind the `docker-tests` feature.
//!
//! The container case runs against the real Docker daemon and skips when no
//! daemon answers. The systemd case runs only on a Linux host with systemd and
//! skips elsewhere, because systemd is the Linux service manager.

#![cfg(feature = "docker-tests")]

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;

#[cfg(target_os = "linux")]
use async_trait::async_trait;
use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;
use foundation_installer::{
    context::DockerApi,
    manifest::{DeploySpec, SourceSpec},
    DockerEngine, EntryOutcome, InstallContext, Installer, Manifest, SystemCommandRunner,
};
#[cfg(target_os = "linux")]
use foundation_installer::{Artifact, SourceAdapter};
use tempfile::TempDir;

/// A unique entry name for this test process.
fn unique_name(prefix: &str) -> String {
    format!("{prefix}-{}", std::process::id())
}

fn container_body() -> DeploySpec {
    DeploySpec::Docker {
        ports: Vec::new(),
        env: BTreeMap::new(),
        volumes: Vec::new(),
        restart: None,
        command: Some(vec!["sleep".to_string(), "300".to_string()]),
    }
}

async fn clean_container(docker: &Arc<dyn DockerApi>, name: &str) {
    if docker.inspect_container(name).await.is_ok() {
        let _ = docker.stop_container(name, Some(1)).await;
        let _ = docker.remove_container(name, true).await;
    }
}

#[valtron_test]
async fn a_container_entry_installs_is_unchanged_and_upgrades() {
    let client = match DockerClient::connect_with_defaults() {
        Ok(client) => client,
        Err(err) => {
            eprintln!("skip: no Docker client: {err}");
            return;
        }
    };
    if let Err(err) = client.system_ping().await {
        eprintln!("skip: no Docker daemon: {err}");
        return;
    }

    let directory = TempDir::new().expect("a temporary directory");
    let mut config = support::test_config(directory.path());
    config.start_timeout = std::time::Duration::from_secs(60);
    config.start_poll_interval = std::time::Duration::from_millis(500);

    let docker: Arc<dyn DockerApi> = Arc::new(DockerEngine::new(client));
    let name = unique_name("ewe-e2e-container");
    clean_container(&docker, &name).await;

    let ctx = InstallContext::new(config.clone(), support::test_ctx())
        .with_docker(docker.clone())
        .with_commands(Arc::new(SystemCommandRunner))
        .with_http(foundation_netio::default_http_client());

    let entry = support::entry(
        &name,
        "1.0.0",
        SourceSpec::DockerImage {
            repository: "alpine".to_string(),
            tag: "3.20".to_string(),
            digest: None,
        },
        container_body(),
    );
    let manifest = Manifest {
        manifest_version: 1,
        entries: vec![entry.clone()],
    };
    let installer = Installer::with_defaults(config.clone());

    let first = installer.run(&manifest, &ctx).await.expect("the first run");
    assert!(
        matches!(first.entries[0], EntryOutcome::Applied { .. }),
        "expected an applied container, got {:?}",
        first.entries[0]
    );

    let second = installer
        .run(&manifest, &ctx)
        .await
        .expect("the second run");
    assert!(
        matches!(second.entries[0], EntryOutcome::Unchanged { .. }),
        "expected an unchanged container, got {:?}",
        second.entries[0]
    );

    let mut upgraded = entry;
    upgraded.version = "2.0.0".to_string();
    let upgrade_manifest = Manifest {
        manifest_version: 1,
        entries: vec![upgraded],
    };
    let third = installer
        .run(&upgrade_manifest, &ctx)
        .await
        .expect("the upgrade run");
    assert!(
        matches!(third.entries[0], EntryOutcome::Applied { .. }),
        "expected an upgraded container, got {:?}",
        third.entries[0]
    );

    clean_container(&docker, &name).await;
}

/// A source that hands back a local file, so the systemd case needs no network.
#[cfg(target_os = "linux")]
struct LocalFileSource {
    path: std::path::PathBuf,
}

#[cfg(target_os = "linux")]
#[async_trait]
impl SourceAdapter for LocalFileSource {
    fn kind(&self) -> &'static str {
        "url"
    }

    async fn resolve(
        &self,
        _entry: &foundation_installer::Entry,
        _ctx: &InstallContext,
    ) -> Result<Artifact, foundation_installer::InstallError> {
        Ok(Artifact::File {
            path: self.path.clone(),
            sha256: "unused".to_string(),
        })
    }
}

#[cfg(target_os = "linux")]
fn is_root() -> bool {
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim() == "0")
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn systemd_is_up() -> bool {
    std::process::Command::new("systemctl")
        .arg("is-system-running")
        .output()
        .map(|output| {
            let state = String::from_utf8_lossy(&output.stdout).trim().to_string();
            state == "running" || state == "degraded"
        })
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
#[valtron_test]
async fn a_file_entry_runs_as_a_systemd_unit() {
    if !is_root() || !systemd_is_up() {
        eprintln!("skip: the systemd case needs root and a running systemd");
        return;
    }

    let directory = TempDir::new().expect("a temporary directory");
    let mut config = support::test_config(directory.path());
    config.install_root = std::path::PathBuf::from("/opt/ewe");
    config.systemd_unit_dir = std::path::PathBuf::from("/etc/systemd/system");
    config.start_timeout = std::time::Duration::from_secs(30);

    let binary = directory.path().join("sleep-binary");
    std::fs::copy("/bin/sleep", &binary).expect("copy sleep");

    let name = unique_name("ewe-e2e-systemd");
    let ctx = InstallContext::new(config.clone(), support::test_ctx())
        .with_commands(Arc::new(SystemCommandRunner));
    let entry = support::entry(
        &name,
        "1.0.0",
        SourceSpec::Url {
            url: "https://example.com/unused".to_string(),
            sha256: "0".repeat(64),
        },
        DeploySpec::Systemd {
            args: vec!["300".to_string()],
            env: BTreeMap::new(),
            user: None,
            restart: None,
        },
    );
    let manifest = Manifest {
        manifest_version: 1,
        entries: vec![entry],
    };
    let installer = Installer::with_defaults(config.clone())
        .with_source(Arc::new(LocalFileSource { path: binary }));

    let first = installer.run(&manifest, &ctx).await.expect("the first run");
    assert!(
        matches!(first.entries[0], EntryOutcome::Applied { .. }),
        "expected an applied unit, got {:?}",
        first.entries[0]
    );
    let second = installer
        .run(&manifest, &ctx)
        .await
        .expect("the second run");
    assert!(
        matches!(second.entries[0], EntryOutcome::Unchanged { .. }),
        "expected an unchanged unit, got {:?}",
        second.entries[0]
    );

    let unit = format!("ewe-{name}.service");
    let _ = std::process::Command::new("systemctl")
        .args(["disable", "--now", &unit])
        .output();
    let _ = std::fs::remove_file(format!("/etc/systemd/system/{unit}"));
    let _ = std::fs::remove_dir_all(format!("/opt/ewe/{name}"));
}
