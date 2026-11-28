//! Tests for the container create body (issue #966).
//!
//! WHY: The Docker spec models `HostConfig` as an `allOf` of `Resources` and
//! the networking fields. The generated `HostConfig` merges both, so the
//! hand-written wrapper must not flatten the generated `Resources` a second
//! time. A second flatten shadowed `Devices` with its default `None`.
//!
//! WHAT: A wrapper around a `HostConfig` with a device serializes that device
//! and the resource fields.

#![cfg(feature = "docker")]

use foundation_deployment_docker::client::ContainerHostConfig;
use foundation_deployment_docker::generated::json::{DeviceMapping, HostConfig};
use serde_json::json;

/// One device mapping for `/dev/kvm`.
fn kvm_device() -> DeviceMapping {
    DeviceMapping {
        cgroup_permissions: Some("rwm".to_string()),
        path_in_container: Some("/dev/kvm".to_string()),
        path_on_host: Some("/dev/kvm".to_string()),
    }
}

/// The resource and device fields of one `HostConfig` reach the create body.
#[test]
fn host_config_keeps_its_devices_and_resources() {
    let config = ContainerHostConfig {
        base: HostConfig {
            devices: Some(vec![kvm_device()]),
            memory: Some(256 * 1_048_576),
            nano_cpus: Some(2_000_000_000),
            pids_limit: Some(4096),
            ..HostConfig::default()
        },
    };
    let value = serde_json::to_value(&config).expect("the host config serializes");
    assert_eq!(
        value["Devices"],
        json!([{
            "CgroupPermissions": "rwm",
            "PathInContainer": "/dev/kvm",
            "PathOnHost": "/dev/kvm"
        }]),
        "the one Devices field is the value that the caller set"
    );
    assert_eq!(value["Memory"], 256 * 1_048_576_i64);
    assert_eq!(value["NanoCpus"], 2_000_000_000_i64);
    assert_eq!(value["PidsLimit"], 4096);
}
