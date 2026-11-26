//! The request bodies, methods and paths.

use std::time::Duration;

use foundation_sandbox::native::firecracker::generated::actions::InstanceActionInfo;
use foundation_sandbox::native::firecracker::generated::boot_source::BootSource;
use foundation_sandbox::native::firecracker::generated::default::InstanceInfoState;
use foundation_sandbox::native::firecracker::generated::drives::Drive;
use foundation_sandbox::native::firecracker::generated::machine_config::MachineConfiguration;
use foundation_sandbox::native::firecracker::generated::vsock::Vsock;
use serde_json::{json, Value};

use super::mock_socket::Request;
use super::support::{self, json_ok, no_content, MockApi};

/// The request body as JSON.
fn body_of(request: &Request) -> Value {
    serde_json::from_slice(&request.body).expect("the request body is JSON")
}

/// The request body must never carry a `null`.
fn assert_no_null(request: &Request) {
    assert!(
        !String::from_utf8_lossy(&request.body).contains("null"),
        "the body has a null: {}",
        String::from_utf8_lossy(&request.body)
    );
}

/// The client for one mock server.
fn client(
    ctx: &foundation_context::Ctx,
    server: &MockApi,
) -> foundation_sandbox::native::firecracker::client::FirecrackerClient {
    support::client(ctx, &server.socket, Duration::from_secs(5), 64 * 1024)
}

#[test]
fn machine_config_body() {
    let (ctx, _clock) = support::simulated_ctx(1);
    let server = MockApi::start("machine-config", |_| no_content());
    let client = client(&ctx, &server);
    let body = MachineConfiguration {
        vcpu_count: 1,
        mem_size_mib: 512,
        smt: Some(false),
        ..Default::default()
    };
    support::block_on(client.put_machine_config(&body)).expect("put machine config");

    let requests = server.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PUT");
    assert_eq!(requests[0].target, "/machine-config");
    assert_eq!(
        body_of(&requests[0]),
        json!({"vcpu_count": 1, "mem_size_mib": 512, "smt": false})
    );
    assert_no_null(&requests[0]);
}

#[test]
fn boot_source_body() {
    let (ctx, _clock) = support::simulated_ctx(2);
    let server = MockApi::start("boot-source", |_| no_content());
    let client = client(&ctx, &server);
    let body = BootSource {
        kernel_image_path: "/kernel/vmlinux".to_string(),
        initrd_path: Some("/kernel/initrd".to_string()),
        boot_args: Some("console=ttyS0 reboot=k panic=1".to_string()),
    };
    support::block_on(client.put_boot_source(&body)).expect("put boot source");

    let requests = server.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PUT");
    assert_eq!(requests[0].target, "/boot-source");
    assert_eq!(
        body_of(&requests[0]),
        json!({
            "kernel_image_path": "/kernel/vmlinux",
            "initrd_path": "/kernel/initrd",
            "boot_args": "console=ttyS0 reboot=k panic=1",
        })
    );
    assert_no_null(&requests[0]);
}

#[test]
fn drive_body_and_path() {
    let (ctx, _clock) = support::simulated_ctx(3);
    let server = MockApi::start("drive", |_| no_content());
    let client = client(&ctx, &server);
    let body = Drive {
        drive_id: "rootfs".to_string(),
        is_root_device: false,
        path_on_host: Some("/images/rootfs.ext4".to_string()),
        is_read_only: Some(true),
        ..Default::default()
    };
    support::block_on(client.put_drive(&body)).expect("put drive");

    let requests = server.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PUT");
    assert_eq!(requests[0].target, "/drives/rootfs");
    assert_eq!(
        body_of(&requests[0]),
        json!({
            "drive_id": "rootfs",
            "path_on_host": "/images/rootfs.ext4",
            "is_root_device": false,
            "is_read_only": true,
        })
    );
    assert_no_null(&requests[0]);
}

#[test]
fn vsock_body() {
    let (ctx, _clock) = support::simulated_ctx(4);
    let server = MockApi::start("vsock", |_| no_content());
    let client = client(&ctx, &server);
    let body = Vsock {
        guest_cid: 4,
        uds_path: "/run/sandbox/vsock.sock".to_string(),
        ..Default::default()
    };
    support::block_on(client.put_vsock(&body)).expect("put vsock");

    let requests = server.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PUT");
    assert_eq!(requests[0].target, "/vsock");
    assert_eq!(
        body_of(&requests[0]),
        json!({"guest_cid": 4, "uds_path": "/run/sandbox/vsock.sock"})
    );
    assert_no_null(&requests[0]);
}

#[test]
fn instance_start_body() {
    let (ctx, _clock) = support::simulated_ctx(5);
    let server = MockApi::start("start", |_| no_content());
    let client = client(&ctx, &server);
    support::block_on(client.instance_start()).expect("instance start");

    let requests = server.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "PUT");
    assert_eq!(requests[0].target, "/actions");
    assert_eq!(
        body_of(&requests[0]),
        json!({"action_type": "InstanceStart"})
    );
}

#[test]
fn describe_instance_parses_state() {
    let (ctx, _clock) = support::simulated_ctx(6);
    let server = MockApi::start("describe", |_| {
        json_ok(
            r#"{"app_name":"Firecracker","id":"x","state":"Not started","vmm_version":"1.19.0"}"#,
        )
    });
    let client = client(&ctx, &server);
    let info = support::block_on(client.describe_instance()).expect("describe instance");

    assert_eq!(info.app_name, "Firecracker");
    assert_eq!(info.id, "x");
    assert_eq!(info.vmm_version, "1.19.0");
    assert!(
        matches!(info.state, InstanceInfoState::NotStarted),
        "state is {:?}",
        info.state
    );

    let requests = server.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].target, "/");
}

#[test]
fn version_parses() {
    let (ctx, _clock) = support::simulated_ctx(7);
    let server = MockApi::start("version", |_| {
        json_ok(r#"{"firecracker_version":"1.19.0"}"#)
    });
    // `FirecrackerClient::new` takes the shared sandbox limits; this test
    // exercises that entry point.
    let client = support::sandbox_client(&ctx, &server.socket);
    let version = support::block_on(client.version()).expect("version");

    assert_eq!(version.firecracker_version, "1.19.0");
    let requests = server.take_requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].method, "GET");
    assert_eq!(requests[0].target, "/version");
}

// The typed action keeps the JSON string the API expects.
#[test]
fn instance_start_action_is_typed() {
    let info = InstanceActionInfo {
        action_type:
            foundation_sandbox::native::firecracker::generated::actions::InstanceActionInfoActionType::InstanceStart,
    };
    assert_eq!(
        serde_json::to_value(&info).expect("serialize"),
        json!({"action_type": "InstanceStart"})
    );
}

// The enum keeps a value with a leading digit, through its `rename`.
#[test]
fn huge_pages_value_with_digit_round_trips() {
    let value = serde_json::to_value(
        foundation_sandbox::native::firecracker::generated::machine_config::MachineConfigurationHugePages::Variant2M,
    )
    .expect("serialize");
    assert_eq!(value, json!("2M"));
    let parsed: foundation_sandbox::native::firecracker::generated::machine_config::MachineConfigurationHugePages =
        serde_json::from_value(value).expect("deserialize");
    assert!(matches!(
        parsed,
        foundation_sandbox::native::firecracker::generated::machine_config::MachineConfigurationHugePages::Variant2M
    ));
}
