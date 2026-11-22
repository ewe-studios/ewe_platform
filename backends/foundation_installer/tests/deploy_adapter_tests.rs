//! Deploy-target adapters: the compatibility table, the unit text, the
//! container body and rollback.

mod support;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::valtron_test;
use foundation_installer::{
    deploy::{docker::DockerDeploy, none::NoneDeploy, systemd::SystemdDeploy, DeployTargetAdapter},
    manifest::{DeploySpec, DockerRestart, PortProtocol, PortSpec, SystemdRestart, VolumeSpec},
    source::Artifact,
    Deployed, ImageRef,
};
use support::{
    deploy_none, entry, fake_context, test_config, FakeCommands, FakeDocker, FakeHttpClient,
};

fn systemd_deploy() -> DeploySpec {
    let mut env = BTreeMap::new();
    env.insert("RUST_LOG".to_string(), "info".to_string());
    DeploySpec::Systemd {
        args: vec![
            "serve".to_string(),
            "--bind".to_string(),
            "0.0.0.0:7443".to_string(),
        ],
        env,
        user: Some("ewe".to_string()),
        restart: Some(SystemdRestart::Always),
    }
}

fn docker_deploy() -> DeploySpec {
    let mut env = BTreeMap::new();
    env.insert("LOG_LEVEL".to_string(), "info".to_string());
    DeploySpec::Docker {
        ports: vec![PortSpec {
            host: 6333,
            container: 6333,
            protocol: PortProtocol::Tcp,
        }],
        env,
        volumes: vec![VolumeSpec {
            host: std::path::PathBuf::from("/var/lib/vector-db"),
            container: std::path::PathBuf::from("/data"),
        }],
        restart: Some(DockerRestart::UnlessStopped),
        command: Some(vec![
            "--config".to_string(),
            "/data/config.json".to_string(),
        ]),
    }
}

#[test]
fn the_compatibility_table_holds() {
    let systemd = systemd_deploy();
    let docker = docker_deploy();
    let none = deploy_none();
    for (deploy, class, expected) in [
        (&systemd, "file", true),
        (&systemd, "tool", true),
        (&systemd, "image", false),
        (&docker, "file", false),
        (&docker, "tool", false),
        (&docker, "image", true),
        (&none, "file", true),
        (&none, "tool", true),
        (&none, "image", true),
    ] {
        assert_eq!(deploy.accepts(class), expected, "{deploy:?} + {class}");
    }
}

#[valtron_test]
async fn systemd_writes_the_expected_unit() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let source_path = directory.path().join("downloaded");
    std::fs::write(&source_path, b"binary").expect("the downloaded file");
    let artifact = Artifact::File {
        path: source_path,
        sha256: "unused".to_string(),
    };
    let commands = FakeCommands::new();
    commands.respond("systemctl", &["is-active"], true, "active");
    let ctx = fake_context(
        config.clone(),
        FakeHttpClient::ok(Vec::new()),
        commands,
        FakeDocker::new("sha256:x"),
    );
    let item = entry("agent-rpc", "0.4.0", source_for_none(), systemd_deploy());

    let deployed = SystemdDeploy
        .apply(&item, &artifact, None, &ctx)
        .await
        .expect("the unit is applied");

    let unit_path = config.systemd_unit_dir.join("ewe-agent-rpc.service");
    let unit = std::fs::read_to_string(&unit_path).expect("the unit file exists");
    let program = config
        .install_root
        .join("agent-rpc")
        .join("current")
        .join("agent-rpc");
    assert!(
        unit.contains(&format!(
            "ExecStart=\"{}\" \"serve\" \"--bind\" \"0.0.0.0:7443\"",
            program.display()
        )),
        "unexpected unit:\n{unit}"
    );
    assert!(unit.contains("User=ewe"), "unexpected unit:\n{unit}");
    assert!(unit.contains("Restart=always"), "unexpected unit:\n{unit}");
    assert!(
        unit.contains("Environment=\"RUST_LOG=info\""),
        "unexpected unit:\n{unit}"
    );
    assert!(
        unit.contains("Environment=\"EWE_ENTRY=agent-rpc\""),
        "unexpected unit:\n{unit}"
    );
    assert!(
        unit.contains("Environment=\"EWE_VERSION=0.4.0\""),
        "unexpected unit:\n{unit}"
    );

    match &deployed {
        Deployed::Systemd {
            unit,
            exec_path,
            version,
        } => {
            assert_eq!(unit, "ewe-agent-rpc.service");
            assert_eq!(
                exec_path,
                &config
                    .install_root
                    .join("agent-rpc")
                    .join("0.4.0")
                    .join("agent-rpc")
            );
            assert_eq!(version, "0.4.0");
        }
        other => panic!("expected a systemd deployment, got {other:?}"),
    }
    assert!(SystemdDeploy
        .exists(&item, &deployed, &ctx)
        .await
        .expect("exists"));
}

#[valtron_test]
async fn systemd_tool_uses_the_program_path() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let program = directory.path().join("bin").join("rg");
    std::fs::create_dir_all(program.parent().expect("a parent")).expect("the bin directory");
    std::fs::write(&program, b"binary").expect("the fake tool");
    let artifact = Artifact::Tool {
        program: "rg".to_string(),
        path: program.clone(),
    };
    let commands = FakeCommands::new();
    commands.respond("systemctl", &["is-active"], true, "active");
    let ctx = fake_context(
        config,
        FakeHttpClient::ok(Vec::new()),
        commands,
        FakeDocker::new("sha256:x"),
    );
    let item = entry("ripgrep", "14.1.0", source_for_none(), systemd_deploy());

    let deployed = SystemdDeploy
        .apply(&item, &artifact, None, &ctx)
        .await
        .expect("the tool unit is applied");
    match deployed {
        Deployed::Systemd { exec_path, .. } => assert_eq!(exec_path, program),
        other => panic!("expected a systemd deployment, got {other:?}"),
    }
}

#[valtron_test]
async fn none_only_records_the_artifact() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let path = directory.path().join("rg");
    std::fs::write(&path, b"binary").expect("the fake tool");
    let artifact = Artifact::File {
        path,
        sha256: "unused".to_string(),
    };
    let ctx = fake_context(
        config,
        FakeHttpClient::ok(Vec::new()),
        FakeCommands::new(),
        FakeDocker::new("sha256:x"),
    );
    let item = entry("ripgrep", "14.1.0", source_for_none(), deploy_none());
    let deployed = NoneDeploy
        .apply(&item, &artifact, None, &ctx)
        .await
        .expect("the artifact is recorded");
    assert_eq!(
        deployed,
        Deployed::Installed {
            artifact: artifact.clone()
        }
    );
    assert!(NoneDeploy
        .exists(&item, &deployed, &ctx)
        .await
        .expect("exists"));
}

#[valtron_test]
async fn docker_creates_starts_and_swaps_the_container() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let config = test_config(directory.path());
    let docker = FakeDocker::new(&format!("sha256:{}", "ab".repeat(32)));
    let ctx = fake_context(
        config,
        FakeHttpClient::ok(Vec::new()),
        FakeCommands::new(),
        Arc::clone(&docker),
    );
    let image = ImageRef {
        repository: "ghcr.io/example/vector-db".to_string(),
        tag: "1.9.0".to_string(),
        digest: format!("sha256:{}", "ab".repeat(32)),
    };
    let artifact = Artifact::Image(image.clone());
    let item = entry("vector-db", "1.9.0", source_for_none(), docker_deploy());

    let deployed = DockerDeploy
        .apply(&item, &artifact, None, &ctx)
        .await
        .expect("the container is applied");
    assert_eq!(
        deployed,
        Deployed::Container {
            name: "vector-db".to_string(),
            id: "id-vector-db-next".to_string(),
            image,
            version: "1.9.0".to_string(),
        }
    );

    let created = docker.created();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].0, "vector-db-next");
    let body = &created[0].1;
    assert_eq!(
        body["Image"], "ghcr.io/example/vector-db:1.9.0",
        "unexpected create body: {body}"
    );
    assert_eq!(body["Labels"]["dev.ewe.entry"], "vector-db");
    assert_eq!(body["Labels"]["dev.ewe.version"], "1.9.0");
    assert_eq!(body["Cmd"][0], "--config");
    assert_eq!(body["Env"][0], "LOG_LEVEL=info");
    assert_eq!(
        body["HostConfig"]["RestartPolicy"]["Name"],
        "unless-stopped"
    );
    assert_eq!(
        body["HostConfig"]["PortBindings"]["6333/tcp"][0]["HostPort"],
        "6333"
    );
    assert_eq!(body["HostConfig"]["Binds"][0], "/var/lib/vector-db:/data");
    assert_eq!(
        docker.renamed(),
        vec![("id-vector-db-next".to_string(), "vector-db".to_string())]
    );
    assert!(DockerDeploy
        .exists(&item, &deployed, &ctx)
        .await
        .expect("exists"));
}

#[valtron_test]
async fn docker_removes_a_container_that_does_not_start() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let mut config = test_config(directory.path());
    config.start_timeout = Duration::ZERO;
    let docker = FakeDocker::with_start_failure(&format!("sha256:{}", "ab".repeat(32)));
    let ctx = fake_context(
        config,
        FakeHttpClient::ok(Vec::new()),
        FakeCommands::new(),
        Arc::clone(&docker),
    );
    let image = ImageRef {
        repository: "ghcr.io/example/vector-db".to_string(),
        tag: "1.9.0".to_string(),
        digest: format!("sha256:{}", "ab".repeat(32)),
    };
    let item = entry("vector-db", "1.9.0", source_for_none(), docker_deploy());

    let error = DockerDeploy
        .apply(&item, &Artifact::Image(image), None, &ctx)
        .await
        .expect_err("the container does not run");
    assert!(matches!(
        error,
        foundation_installer::InstallError::StartTimeout { .. }
    ));
    assert!(docker.removed().contains(&"id-vector-db-next".to_string()));
}

#[cfg(unix)]
#[valtron_test]
async fn systemd_points_current_back_after_a_failed_start() {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let mut config = test_config(directory.path());
    config.start_timeout = Duration::ZERO;
    let source_path = directory.path().join("downloaded");
    std::fs::write(&source_path, b"binary").expect("the downloaded file");
    let artifact = Artifact::File {
        path: source_path,
        sha256: "unused".to_string(),
    };
    let commands = FakeCommands::new();
    commands.respond("systemctl", &["is-active"], false, "inactive");
    let ctx = fake_context(
        config.clone(),
        FakeHttpClient::ok(Vec::new()),
        commands,
        FakeDocker::new("sha256:x"),
    );
    let item = entry("agent-rpc", "0.4.0", source_for_none(), systemd_deploy());
    let previous = Deployed::Systemd {
        unit: "ewe-agent-rpc.service".to_string(),
        exec_path: config
            .install_root
            .join("agent-rpc")
            .join("0.3.0")
            .join("agent-rpc"),
        version: "0.3.0".to_string(),
    };
    std::fs::create_dir_all(config.install_root.join("agent-rpc").join("0.3.0"))
        .expect("the old version directory");

    let error = SystemdDeploy
        .apply(&item, &artifact, Some(&previous), &ctx)
        .await
        .expect_err("the new version does not start");
    assert!(matches!(
        error,
        foundation_installer::InstallError::StartTimeout { .. }
    ));
    let current = config.install_root.join("agent-rpc").join("current");
    assert_eq!(
        std::fs::read_link(&current).expect("the current link"),
        config.install_root.join("agent-rpc").join("0.3.0")
    );
    let unit = std::fs::read_to_string(config.systemd_unit_dir.join("ewe-agent-rpc.service"))
        .expect("the unit file");
    assert!(unit.contains("Environment=\"EWE_VERSION=0.3.0\""));
}

fn source_for_none() -> foundation_installer::SourceSpec {
    foundation_installer::SourceSpec::Url {
        url: "https://example.com/unused".to_string(),
        sha256: "0".repeat(64),
    }
}
