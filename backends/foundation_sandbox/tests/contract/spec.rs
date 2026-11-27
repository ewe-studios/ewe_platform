//! I4: `validate` returns the first failed rule in the fixed order.

use std::path::PathBuf;

use foundation_sandbox::shared::driver::Capabilities;
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_sandbox::shared::ids::DriverId;
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::spec::{
    HostMount, ImageRef, NetworkMode, RootFs, RootMode, SandboxSpec,
};

/// Capabilities that allow every rule.
fn capabilities() -> Capabilities {
    Capabilities {
        rootfs_directory: true,
        rootfs_ext4: true,
        rootfs_squashfs: true,
        rootfs_container_image: true,
        root_modes: vec![RootMode::Overlay, RootMode::ReadOnly],
        host_mounts: true,
        network_none: true,
        network_host: true,
        max_vcpus: 8,
    }
}

/// A spec that passes every rule.
fn spec() -> SandboxSpec {
    SandboxSpec::new(RootFs::Directory(PathBuf::from("/srv/root")))
}

/// Checks that `result` is `InvalidSpec` for `field`.
fn assert_invalid(result: SandboxResult<()>, field: &str) {
    let error = result.expect_err("the spec must fail");
    match error.current_context() {
        SandboxError::InvalidSpec { field: actual, .. } => assert_eq!(actual, field),
        other => panic!("expected InvalidSpec, got {other:?}"),
    }
}

/// Checks that `result` is `UnsupportedRootFs`.
fn assert_unsupported_rootfs(result: SandboxResult<()>) {
    assert!(matches!(
        result.expect_err("the spec must fail").current_context(),
        SandboxError::UnsupportedRootFs
    ));
}

/// Checks that `result` is `UnsupportedNetwork`.
fn assert_unsupported_network(result: SandboxResult<()>) {
    assert!(matches!(
        result.expect_err("the spec must fail").current_context(),
        SandboxError::UnsupportedNetwork
    ));
}

/// I4 and the two validate scenarios: one case for each rule.
#[allow(
    clippy::too_many_lines,
    reason = "one case for each of the nine ordered rules"
)]
#[test]
fn spec_validate() {
    let caps = capabilities();
    assert!(spec().validate(&caps).is_ok());

    // Rule 1: vcpus and memory.
    let mut wrong = spec();
    wrong.vcpus = 0;
    assert_invalid(wrong.validate(&caps), "vcpus");

    let mut wrong = spec();
    wrong.memory_mib = 63;
    assert_invalid(wrong.validate(&caps), "memory_mib");

    // Rule 2: absolute paths without `..`.
    let mut wrong = spec();
    wrong.workdir = "relative".to_string();
    assert_invalid(wrong.validate(&caps), "workdir");

    let mut wrong = spec();
    wrong.workdir = "/a/../b".to_string();
    assert_invalid(wrong.validate(&caps), "workdir");

    let mut wrong = spec();
    wrong.mounts = vec![HostMount {
        host_path: PathBuf::from("/tmp/x"),
        guest_path: "relative".to_string(),
        read_only: false,
    }];
    assert_invalid(wrong.validate(&caps), "mounts");

    let mut wrong = spec();
    wrong.mounts = vec![HostMount {
        host_path: PathBuf::from("/tmp/x"),
        guest_path: "/a/../b".to_string(),
        read_only: false,
    }];
    assert_invalid(wrong.validate(&caps), "mounts");

    // Rule 3: environment keys.
    for key in ["", "A=B", "A\0"] {
        let mut wrong = spec();
        wrong.env = vec![(key.to_string(), "value".to_string())];
        assert_invalid(wrong.validate(&caps), "env");
    }

    // Rule 4: DNS entries parse as an IP address.
    let mut wrong = spec();
    wrong.dns = vec!["not-an-ip".to_string()];
    assert_invalid(wrong.validate(&caps), "dns");

    // Rule 5: the root file system kind is in the capabilities.
    let mut caps_without_squashfs = capabilities();
    caps_without_squashfs.rootfs_squashfs = false;
    let mut wrong = spec();
    wrong.rootfs = RootFs::SquashfsImage(PathBuf::from("/srv/root.squashfs"));
    assert_unsupported_rootfs(wrong.validate(&caps_without_squashfs));

    // `RootFs::Image` never reaches validate; if it does, it is unsupported.
    let mut wrong = spec();
    wrong.rootfs = RootFs::Image(ImageRef::Tag("image".to_string()));
    assert_unsupported_rootfs(wrong.validate(&caps));

    // Rule 6: the root mode is in the capabilities.
    let mut caps_without_read_only = capabilities();
    caps_without_read_only.root_modes = vec![RootMode::Overlay];
    let mut wrong = spec();
    wrong.root_mode = RootMode::ReadOnly;
    assert_invalid(wrong.validate(&caps_without_read_only), "root_mode");

    // Rule 7: the network mode is in the capabilities.
    let mut caps_without_none = capabilities();
    caps_without_none.network_none = false;
    let mut wrong = spec();
    wrong.network = NetworkMode::None;
    assert_unsupported_network(wrong.validate(&caps_without_none));

    let mut caps_without_host = capabilities();
    caps_without_host.network_host = false;
    let mut wrong = spec();
    wrong.network = NetworkMode::Host;
    assert_unsupported_network(wrong.validate(&caps_without_host));

    // Rule 8: host mounts are in the capabilities.
    let mut caps_without_mounts = capabilities();
    caps_without_mounts.host_mounts = false;
    let mut wrong = spec();
    wrong.mounts = vec![HostMount {
        host_path: PathBuf::from("/tmp/x"),
        guest_path: "/mnt/x".to_string(),
        read_only: false,
    }];
    assert!(matches!(
        wrong
            .validate(&caps_without_mounts)
            .expect_err("the mount must fail")
            .current_context(),
        SandboxError::UnsupportedMount
    ));

    // Rule 9: every limit is more than zero, and a chunk is at most half a
    // message.
    let mut wrong = spec();
    wrong.limits.max_message_bytes = 0;
    assert_invalid(wrong.validate(&caps), "limits");

    let mut wrong = spec();
    let half = wrong.limits.max_message_bytes / 2;
    wrong.limits.chunk_bytes = half + 1;
    assert_invalid(wrong.validate(&caps), "limits");
}

/// Issue #1031: an empty workdir means the default; a bad value still fails.
#[test]
fn spec_workdir_empty_is_the_default() {
    let caps = capabilities();

    let mut empty = spec();
    empty.workdir = String::new();
    assert!(empty.validate(&caps).is_ok());
    assert_eq!(empty.effective_workdir(), "/workspace");

    // A path that leaves the root still fails.
    let mut escape = spec();
    escape.workdir = "../x".to_string();
    assert_invalid(escape.validate(&caps), "workdir");

    let mut relative = spec();
    relative.workdir = "tmp".to_string();
    assert_invalid(relative.validate(&caps), "workdir");

    // A value that the caller gives is used as it is.
    let mut custom = spec();
    custom.workdir = "/tmp".to_string();
    assert!(custom.validate(&caps).is_ok());
    assert_eq!(custom.effective_workdir(), "/tmp");
}

/// I4: the first failed rule wins.
#[test]
fn spec_validate_first_rule_wins() {
    let mut wrong = spec();
    wrong.vcpus = 0;
    wrong.workdir = "relative".to_string();
    assert_invalid(wrong.validate(&capabilities()), "vcpus");
}

/// I8: every limit has its documented default, and every default is positive.
#[test]
fn limits_defaults() {
    let limits = SandboxLimits::default();
    assert_eq!(limits.boot_timeout, std::time::Duration::from_secs(10));
    assert_eq!(limits.shutdown_grace, std::time::Duration::from_secs(5));
    assert_eq!(limits.rpc_timeout, std::time::Duration::from_secs(5));
    assert_eq!(limits.max_message_bytes, 4 * 1024 * 1024);
    assert_eq!(limits.chunk_bytes, 64 * 1024);
    assert_eq!(limits.max_exec_sessions, 128);
    assert_eq!(limits.max_captured_output, 1024 * 1024);
    assert_eq!(limits.max_error_message, 64 * 1024);
    assert_eq!(
        limits.output_drain_timeout,
        std::time::Duration::from_secs(2)
    );
    assert_eq!(
        limits.upload_idle_timeout,
        std::time::Duration::from_secs(60)
    );
    assert_eq!(
        limits.input_retry_timeout,
        std::time::Duration::from_secs(30)
    );
    assert_eq!(limits.max_unpack_entries, 1_000_000);
    assert_eq!(limits.max_unpack_bytes, 16 * 1024 * 1024 * 1024);
    assert!(limits.all_positive());

    let changed = SandboxLimits::new()
        .with_boot_timeout(std::time::Duration::from_secs(1))
        .with_chunk_bytes(1024)
        .with_max_unpack_bytes(1);
    assert_eq!(changed.boot_timeout, std::time::Duration::from_secs(1));
    assert_eq!(changed.chunk_bytes, 1024);
    assert_eq!(changed.max_unpack_bytes, 1);
}

/// A new spec has the documented default values.
#[test]
fn spec_new_defaults() {
    let spec = SandboxSpec::new(RootFs::Directory(PathBuf::from("/srv/root")));
    assert_eq!(spec.driver, None);
    assert_eq!(spec.root_mode, RootMode::Overlay);
    assert_eq!(spec.vcpus, 1);
    assert_eq!(spec.memory_mib, 512);
    assert_eq!(spec.network, NetworkMode::None);
    assert_eq!(spec.dns, vec!["1.1.1.1".to_string(), "8.8.8.8".to_string()]);
    assert_eq!(spec.env, Vec::new());
    assert_eq!(spec.workdir, "/workspace");
    assert_eq!(spec.mounts, Vec::new());
    assert_eq!(spec.kernel, None);
    assert_eq!(spec.limits, SandboxLimits::default());
    assert_eq!(spec.driver, None::<DriverId>);
}
