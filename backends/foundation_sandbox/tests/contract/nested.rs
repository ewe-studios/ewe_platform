//! The `parent` rule of a nested sandbox (feature 11).
//!
//! `parent` is `Some` exactly when the driver is `Nested`.

use std::path::PathBuf;

use foundation_sandbox::shared::driver::Capabilities;
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::spec::{RootFs, RootMode, SandboxSpec};

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

/// A spec that passes every other rule.
fn spec() -> SandboxSpec {
    SandboxSpec::new(RootFs::Directory(PathBuf::from("/srv/root")))
}

/// A valid parent identifier.
fn parent() -> SandboxId {
    "000000000001".parse().expect("a valid sandbox id")
}

/// Checks that `result` is `InvalidSpec` for `field`.
fn assert_invalid(result: SandboxResult<()>, field: &str) {
    let error = result.expect_err("the spec must fail");
    match error.current_context() {
        SandboxError::InvalidSpec { field: actual, .. } => assert_eq!(actual, field),
        other => panic!("expected InvalidSpec, got {other:?}"),
    }
}

/// The rule of feature 11, section 6.2.
#[test]
fn spec_parent_rule() {
    let caps = capabilities();

    // A nested child with a parent is valid.
    let mut nested = spec();
    nested.driver = Some(DriverId::Nested);
    nested.parent = Some(parent());
    assert!(nested.validate(&caps).is_ok());

    // A nested child with no parent is not.
    let mut wrong = spec();
    wrong.driver = Some(DriverId::Nested);
    assert_invalid(wrong.validate(&caps), "parent");

    // A parent on another driver is not.
    let mut wrong = spec();
    wrong.driver = Some(DriverId::Firecracker);
    wrong.parent = Some(parent());
    assert_invalid(wrong.validate(&caps), "parent");

    // A parent with no driver named is not.
    let mut wrong = spec();
    wrong.parent = Some(parent());
    assert_invalid(wrong.validate(&caps), "parent");
}

/// The two new fields have their documented defaults.
#[test]
fn spec_nested_defaults() {
    let spec = spec();
    assert_eq!(spec.parent, None);
    assert!(!spec.nested_host);
}
