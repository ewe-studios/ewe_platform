//! Unit tests for ContainerConfig builder.

use foundation_deployment_platform::docker::{ContainerConfig, WaitFor, VolumeSource};

#[test]
fn test_config_new_sets_image() {
    let cfg = ContainerConfig::new("redis:7");
    assert_eq!(cfg.image, "redis:7");
    assert!(cfg.name.is_none());
}

#[test]
fn test_config_builder_methods() {
    let cfg = ContainerConfig::new("postgres:16")
        .name("test-db")
        .port(5432)
        .port_mapped(8080, 80)
        .env("POSTGRES_PASSWORD", "secret")
        .env("POSTGRES_DB", "testdb")
        .network("testbed-net")
        .network_alias("db")
        .memory("512m")
        .cpus(2)
        .stop_timeout_secs(20)
        .wait(WaitFor::port(5432));

    assert_eq!(cfg.image, "postgres:16");
    assert_eq!(cfg.name.unwrap(), "test-db");
    assert_eq!(cfg.ports.len(), 2);
    assert_eq!(cfg.env.len(), 2);
    assert_eq!(cfg.network.unwrap(), "testbed-net");
    assert_eq!(cfg.network_aliases, vec!["db"]);
    assert_eq!(cfg.memory.unwrap(), "512m");
    assert_eq!(cfg.cpus.unwrap(), 2);
    assert_eq!(cfg.stop_timeout, 20);
    assert!(matches!(cfg.wait, WaitFor::Port { port: 5432, .. }));
}

#[test]
fn test_config_label_and_command() {
    let cfg = ContainerConfig::new("nginx:alpine")
        .label("env", "test")
        .label("team", "platform")
        .command(vec!["nginx".into(), "-g".into(), "daemon off;".into()]);

    assert_eq!(cfg.labels.get("env").unwrap(), "test");
    assert_eq!(cfg.labels.get("team").unwrap(), "platform");
    assert_eq!(cfg.command.unwrap(), vec!["nginx", "-g", "daemon off;"]);
}

#[test]
fn test_config_devices_and_caps() {
    let cfg = ContainerConfig::new("dockurr/windows:5.15")
        .device("/dev/kvm")
        .device("/dev/net/tun")
        .cap_add("NET_ADMIN")
        .cap_add("SYS_ADMIN");

    assert_eq!(cfg.devices.len(), 2);
    assert_eq!(cfg.devices[0].host_path.to_string_lossy(), "/dev/kvm");
    assert_eq!(cfg.cap_add, vec!["NET_ADMIN", "SYS_ADMIN"]);
}

#[test]
fn test_config_named_volume() {
    let cfg = ContainerConfig::new("mysql:8")
        .named_volume("mysql-data", "/var/lib/mysql");

    assert_eq!(cfg.volumes.len(), 1);
    match &cfg.volumes[0].source {
        VolumeSource::Named(name) => assert_eq!(name, "mysql-data"),
        _ => panic!("expected named volume"),
    }
}

#[test]
fn test_config_required_flag() {
    let cfg = ContainerConfig::new("redis:7").required();
    assert!(cfg.required);
}

#[test]
fn test_config_always_pull() {
    let cfg = ContainerConfig::new("redis:7").always_pull();
    assert!(cfg.always_pull);
}
