//! The container create body, and where published ports are reached.
//!
//! WHY: A container's ports are published on the Docker host. A process on
//! the host reaches them on loopback, but one inside a job container (a CI
//! `container:` job with the daemon's socket mounted) must use its network's
//! gateway — and a container sharing the caller's network needs that
//! network's mode and no endpoint config of its own. Getting either wrong
//! leaves the ports unreachable.
//!
//! WHAT: `ContainerHandle::build_body`, `ContainerHandle::split_image_tag`
//! and the gateway / network-mode helpers, with no daemon.

use std::net::Ipv4Addr;

use foundation_deployment_platform::docker::container::{
    caller_network_mode, gateway_of, published_port_host, CallerContainer, ContainerHandle,
};
use foundation_deployment_platform::docker::{ContainerConfig, DockerFileConfig};

#[test]
fn an_image_tag_is_split_off_but_a_registry_port_is_not() {
    let split = ContainerHandle::split_image_tag;
    assert_eq!(
        split("alpine:latest"),
        ("alpine".to_string(), Some("latest".to_string()))
    );
    assert_eq!(split("alpine"), ("alpine".to_string(), None));
    assert_eq!(
        split("localhost:5000/img"),
        ("localhost:5000/img".to_string(), None)
    );
    assert_eq!(
        split("localhost:5000/team/img:v2"),
        (
            "localhost:5000/team/img".to_string(),
            Some("v2".to_string())
        )
    );
}

#[test]
fn gateway_of_a_bridge_network_job_container() {
    // What `GET /containers/<self>/json` reports for a GitHub Actions job
    // container: one user-defined bridge network with a gateway.
    let networks = serde_json::json!({
        "github_network_4b74fb24": { "Gateway": "172.18.0.1", "IPAddress": "172.18.0.2" }
    });
    assert_eq!(
        gateway_of(Some(&networks)),
        Some(Ipv4Addr::new(172, 18, 0, 1))
    );
}

#[test]
fn gateway_of_picks_the_first_network_by_name() {
    let networks = serde_json::json!({
        "zeta": { "Gateway": "10.0.9.1" },
        "alpha": { "Gateway": "10.0.1.1" },
    });
    assert_eq!(
        gateway_of(Some(&networks)),
        Some(Ipv4Addr::new(10, 0, 1, 1))
    );
}

#[test]
fn host_network_has_no_gateway() {
    // `--network host` shares the host's loopback: stay on 127.0.0.1.
    let networks = serde_json::json!({ "host": { "Gateway": "", "IPAddress": "" } });
    assert_eq!(gateway_of(Some(&networks)), None);
    assert_eq!(gateway_of(None), None);
    assert_eq!(gateway_of(Some(&serde_json::json!({}))), None);
}

#[test]
fn ports_are_published_on_the_given_host_ip() {
    let config = ContainerConfig::new("redis:7-alpine").port(6379);
    let body = ContainerHandle::build_body(&config, None, Ipv4Addr::new(172, 18, 0, 1), None);
    let json = serde_json::to_value(&body).expect("serialize body");
    assert_eq!(
        json["HostConfig"]["PortBindings"]["6379/tcp"][0]["HostIp"],
        "172.18.0.1"
    );
    assert_eq!(
        json["HostConfig"]["PortBindings"]["6379/tcp"][0]["HostPort"],
        ""
    );
}

#[test]
fn the_caller_network_is_the_callers_container_or_the_host() {
    assert_eq!(caller_network_mode(None), "host");
    let me = CallerContainer {
        id: "0123abcd".into(),
        networks: None,
    };
    assert_eq!(caller_network_mode(Some(&me)), "container:0123abcd");
    // On the host, or in a container on the host network: loopback.
    assert_eq!(published_port_host(None), Ipv4Addr::LOCALHOST);
    assert_eq!(published_port_host(Some(&me)), Ipv4Addr::LOCALHOST);
    let job = CallerContainer {
        id: "job".into(),
        networks: Some(serde_json::json!({ "github_network_x": { "Gateway": "172.18.0.1" } })),
    };
    assert_eq!(
        published_port_host(Some(&job)),
        Ipv4Addr::new(172, 18, 0, 1)
    );
}

#[test]
fn a_container_sharing_the_callers_network_gets_that_mode_and_no_endpoint() {
    let config = ContainerConfig::new("bw")
        .network("ignored")
        .network_alias("also-ignored")
        .share_caller_network();
    let body = ContainerHandle::build_body(
        &config,
        None,
        Ipv4Addr::LOCALHOST,
        Some("container:0123abcd".into()),
    );
    let json = serde_json::to_value(&body).expect("serialize body");
    assert_eq!(json["HostConfig"]["NetworkMode"], "container:0123abcd");
    assert!(json.get("NetworkingConfig").is_none(), "{json}");
}

#[test]
fn building_tags_the_dockerfile_with_the_image() {
    let config =
        ContainerConfig::new("my-e2e-image").build(DockerFileConfig::new("FROM alpine:3.20"));
    assert_eq!(config.build.expect("build set").tag, "my-e2e-image");
}
