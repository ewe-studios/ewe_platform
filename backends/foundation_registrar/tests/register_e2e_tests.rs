//! End-to-end test behind the `docker-tests` feature (spec 82 feature 01, T10).
//!
//! WHY: the parts must work together with a real proxy, the real installer, a
//! real Docker daemon and real probes.
//!
//! WHAT: install an HTTP app with the installer's Docker deploy target, find it
//! through Docker discovery, probe and register it through a real registration
//! authority, fetch it through the proxy, then upgrade it to a new container
//! address while a client keeps sending requests.
//!
//! HOW: the test skips itself (with a printed line) when no Docker daemon
//! answers. It runs on Linux, where the host reaches a container IP.

#![cfg(feature = "docker-tests")]

mod support;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use foundation_core::valtron::valtron_test;
use foundation_deployment_docker::DockerClient;
use foundation_installer::manifest::{DeploySpec, SourceSpec};
use foundation_installer::{
    DockerEngine, EntryName, EntryOutcome, InstallContext, Installer, InstallerConfig, Manifest,
    SystemCommandRunner as InstallerCommandRunner,
};
use foundation_proxy::config::{ServiceConfig, SslConfig};
use foundation_proxy::registration::RegistrationConfig;
use foundation_proxy::{ProxyConfig, ProxyServer};
use foundation_registrar::{
    DockerContainerApi, DockerDiscovery, DockerMetrics, Prober, Reconciler, RegistrarConfig,
    RegistrarContext, RegistrarMode, RegistrationClient,
    SystemCommandRunner as RegistrarCommandRunner,
};

const HOST: &str = "agents.example.com";
const SERVICE: &str = "agents";

fn unique_name() -> String {
    format!("ewe-reg-{}", std::process::id())
}

fn http_get(addr: SocketAddr, host: &str) -> Result<String, String> {
    let mut stream = match TcpStream::connect(addr) {
        Ok(stream) => stream,
        // A busy host can return a transient `EAGAIN` from `connect`. Retry
        // once so the flood does not report a kernel queue limit as a failed
        // request. Any other error stays a failure.
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
            std::thread::sleep(Duration::from_millis(10));
            TcpStream::connect(addr).map_err(|error| error.to_string())?
        }
        Err(error) => return Err(error.to_string()),
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    write!(
        stream,
        "GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n"
    )
    .map_err(|error| error.to_string())?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|error| error.to_string())?;
    let body = response
        .split_once("\r\n\r\n")
        .map_or(response.as_str(), |(_, body)| body);
    // The echo image ends its text with a newline.
    Ok(body.trim_end().to_string())
}

fn installer_entry(name: &str, version: &str, text: &str) -> foundation_installer::Entry {
    foundation_installer::Entry {
        name: EntryName::parse_at(name, "/entries/0/name").expect("entry name"),
        version: version.to_string(),
        source: SourceSpec::DockerImage {
            repository: "hashicorp/http-echo".to_string(),
            tag: "1.0".to_string(),
            digest: None,
        },
        deploy: DeploySpec::Docker {
            ports: Vec::new(),
            env: BTreeMap::new(),
            volumes: Vec::new(),
            restart: None,
            command: Some(vec![format!("-text={text}"), "-listen=:5678".to_string()]),
        },
        proxy: None,
        probes: None,
    }
}

fn installer_manifest(name: &str, version: &str, text: &str) -> Manifest {
    Manifest {
        manifest_version: 1,
        entries: vec![installer_entry(name, version, text)],
    }
}

fn write_sidecar_manifest(path: &std::path::Path, name: &str, version: &str) {
    let value = serde_json::json!({
        "manifest_version": 1,
        "entries": [{
            "name": name,
            "version": version,
            "source": { "kind": "docker-image" },
            "deploy": { "kind": "docker" },
            "proxy": {
                "service": SERVICE,
                "route": { "host": HOST, "path_prefix": "/" },
                "port": 5678,
                "protocol": "http"
            },
            "probes": {
                "readiness": {
                    "kind": "http",
                    "port": 5678,
                    "path": "/",
                    "period_secs": 1,
                    "failure_threshold": 1
                }
            }
        }]
    });
    std::fs::write(
        path,
        serde_json::to_string_pretty(&value).expect("manifest JSON"),
    )
    .expect("write sidecar manifest");
}

async fn wait_for_reply(
    reconciler: &mut Reconciler,
    addr: SocketAddr,
    expected: &str,
    timeout: Duration,
    ctx: &foundation_context::Ctx,
) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let _ = reconciler.step().await;
        if http_get(addr, HOST).ok().as_deref() == Some(expected) {
            return true;
        }
        foundation_core::valtron::sleep_async(Duration::from_millis(250), ctx).await;
    }
    false
}

#[valtron_test]
async fn install_register_and_upgrade_without_a_failed_request() {
    let client = match DockerClient::connect_with_defaults() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("skip: no Docker client: {error}");
            return;
        }
    };
    if let Err(error) = client.system_ping().await {
        eprintln!("skip: no Docker daemon: {error}");
        return;
    }

    let name = unique_name();
    let proxy_dir = tempfile::TempDir::new().expect("proxy state");
    let installer_dir = tempfile::TempDir::new().expect("installer state");
    let registrar_dir = tempfile::TempDir::new().expect("registrar state");
    clean_container(&client, &name).await;

    // 1. The proxy with its registration authority.
    let registration = RegistrationConfig::new()
        .with_bind("127.0.0.1:0")
        .with_state_dir(proxy_dir.path())
        .with_server_names(vec!["localhost".to_string(), "127.0.0.1".to_string()]);
    let config = ProxyConfig::new(HOST, "127.0.0.1")
        .bind("127.0.0.1:0")
        .ssl(SslConfig::default())
        .service(ServiceConfig::new(SERVICE, HOST))
        .registration(Some(registration));
    let ctx = support::host_ctx();
    let proxy = futures_lite::future::block_on(ProxyServer::start(config, ctx.clone()))
        .expect("start the proxy");
    let proxy_addr = proxy.local_addr();
    let authority = proxy
        .registration_authority()
        .cloned()
        .expect("registration authority");

    // 2. The sidecar enrolls with a token.
    let token = authority
        .enrollment()
        .issue_token(&format!("e2e-{}", std::process::id()), None, None)
        .expect("issue token");
    let token_path = registrar_dir.path().join("token");
    std::fs::write(&token_path, token).expect("write token");
    let sidecar_manifest = registrar_dir.path().join("manifest.json");
    write_sidecar_manifest(&sidecar_manifest, &name, "1.0.0");
    let registrar_config = RegistrarConfig::new(
        RegistrarMode::Container,
        format!("e2e-{}", std::process::id()),
    )
    .with_proxy(format!(
        "https://{}",
        authority.registration_addr().expect("registration address")
    ))
    .with_enrollment_url(format!(
        "https://{}",
        authority.enrollment_addr().expect("enrollment address")
    ))
    .with_state_dir(registrar_dir.path().join("state"))
    .with_enrollment_token_path(&token_path)
    .with_manifest_path(&sidecar_manifest);
    let registration_client =
        futures_lite::future::block_on(RegistrationClient::connect(&registrar_config, &ctx))
            .expect("enroll the sidecar");

    // 3. The installer installs the app with the Docker deploy target.
    let installer_config = InstallerConfig {
        state_dir: installer_dir.path().join("installer"),
        artifact_dir: installer_dir.path().join("artifacts"),
        start_timeout: Duration::from_secs(60),
        start_poll_interval: Duration::from_millis(250),
        old_container_grace: Duration::from_secs(4),
        ..InstallerConfig::default()
    };
    let docker = Arc::new(DockerEngine::new(client.clone()));
    let install_context = InstallContext::new(installer_config.clone(), ctx.clone())
        .with_docker(docker)
        .with_commands(Arc::new(InstallerCommandRunner))
        .with_http(foundation_netio::default_http_client());
    let installer = Installer::with_defaults(installer_config);
    let first = installer
        .run(&installer_manifest(&name, "1.0.0", "one"), &install_context)
        .await
        .expect("the first install");
    assert!(
        matches!(first.entries[0], EntryOutcome::Applied { .. }),
        "expected an applied container, got {:?}",
        first.entries[0]
    );

    // 4. The sidecar discovers, probes, registers and reports the app.
    let api = Arc::new(DockerContainerApi::new(Arc::new(client.clone())));
    let context = RegistrarContext::new(
        ctx.clone(),
        Arc::new(registrar_config.clone()),
        Arc::new(RegistrarCommandRunner),
        Some(Arc::new(client.clone())),
    );
    let metrics = foundation_registrar::CompositeMetrics::new(
        None,
        Some(Arc::new(DockerMetrics::new(api.clone()))),
    );
    let prober = Prober::new(
        ctx.clone(),
        Arc::new(foundation_registrar::NetworkProber::new(
            ctx.clone(),
            Arc::new(RegistrarCommandRunner),
            true,
        )),
        256,
    );
    let mut reconciler = Reconciler::new(
        context,
        Arc::new(DockerDiscovery::new(
            api as Arc<dyn foundation_registrar::ContainerApi>,
            None,
        )),
        prober,
        Arc::new(metrics),
        Arc::new(registration_client),
    );

    assert!(
        wait_for_reply(
            &mut reconciler,
            proxy_addr,
            "one",
            Duration::from_secs(60),
            &ctx
        )
        .await,
        "the first version must be served through the proxy"
    );
    assert!(
        !reconciler.registered_ids().is_empty(),
        "the sidecar registered the backend"
    );

    // 5. A client keeps sending requests while the app is upgraded.
    let stop = Arc::new(AtomicBool::new(false));
    let answers = Arc::new(Mutex::new(Vec::new()));
    let request_stop = Arc::clone(&stop);
    let request_answers = Arc::clone(&answers);
    let requests = std::thread::spawn(move || {
        while !request_stop.load(Ordering::SeqCst) {
            let answer = http_get(proxy_addr, HOST);
            request_answers.lock().expect("answers").push(answer);
            std::thread::sleep(Duration::from_millis(20));
        }
    });

    // 6. The upgrade replaces the container. The installer keeps the old one
    //    alive for its grace period; the sidecar discovers the new address in
    //    that window, so the proxy moves traffic before the old one stops.
    let install_done = Arc::new(AtomicBool::new(false));
    let (second, ()) = futures_lite::future::zip(
        async {
            let result = installer
                .run(&installer_manifest(&name, "2.0.0", "two"), &install_context)
                .await;
            install_done.store(true, Ordering::SeqCst);
            result
        },
        async {
            while !install_done.load(Ordering::SeqCst) {
                let _ = reconciler.step().await;
                foundation_core::valtron::sleep_async(Duration::from_millis(100), &ctx).await;
            }
        },
    )
    .await;
    let second = second.expect("the upgrade install");
    assert!(
        matches!(second.entries[0], EntryOutcome::Applied { .. }),
        "expected an upgraded container, got {:?}",
        second.entries[0]
    );
    write_sidecar_manifest(&sidecar_manifest, &name, "2.0.0");
    reconciler.reload(true);
    assert!(
        wait_for_reply(
            &mut reconciler,
            proxy_addr,
            "two",
            Duration::from_secs(60),
            &ctx
        )
        .await,
        "the upgraded version must be served through the proxy"
    );

    stop.store(true, Ordering::SeqCst);
    requests.join().expect("request thread");
    let answers = answers.lock().expect("answers").clone();
    assert!(!answers.is_empty(), "the client sent requests");
    for answer in &answers {
        match answer {
            Ok(body) if body == "one" || body == "two" => {}
            Ok(body) => panic!("a request got a bad answer: {body:?}"),
            Err(error) => panic!("a request failed during the upgrade: {error}"),
        }
    }
    assert!(
        answers.iter().any(|answer| answer.as_deref() == Ok("one")),
        "the old version answered after the client started"
    );
    assert!(
        answers.iter().any(|answer| answer.as_deref() == Ok("two")),
        "the new version answered before the client stopped"
    );

    // 7. Stop the sidecar (it deregisters in container mode) and clean up.
    futures_lite::future::block_on(reconciler.shutdown()).expect("sidecar shutdown");
    clean_container(&client, &name).await;
    proxy.shutdown();
}

async fn clean_container(client: &DockerClient, name: &str) {
    if client.inspect_container(name).await.is_ok() {
        let _ = client.stop_container(name, Some(1)).await;
        let _ = client.remove_container(name, true).await;
    }
}
