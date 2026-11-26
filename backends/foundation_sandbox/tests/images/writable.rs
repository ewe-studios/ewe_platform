//! The writable root rows of section 7 (feature 12): the validation rules and
//! the exclusive image lock.

use foundation_sandbox::shared::control::testing::{full_capabilities, FakeHostFs};
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::host::HostFs as _;
use foundation_sandbox::shared::spec::{RootFs, RootMode, SandboxSpec};

#[test]
fn writable_needs_an_ext4_image() {
    let mut spec = SandboxSpec::new(RootFs::Directory("/root".to_string().into()));
    spec.root_mode = RootMode::Writable;
    let error = spec
        .validate(&full_capabilities())
        .expect_err("a directory root cannot be writable");
    assert!(matches!(
        error.current_context(),
        SandboxError::InvalidSpec { field, .. } if field == "root_mode"
    ));
}

#[test]
fn writable_needs_a_capability() {
    let mut spec = SandboxSpec::new(RootFs::Ext4Image("/image.ext4".to_string().into()));
    spec.root_mode = RootMode::Writable;
    let mut caps = full_capabilities();
    caps.root_modes = vec![RootMode::Overlay, RootMode::ReadOnly];
    let error = spec
        .validate(&caps)
        .expect_err("a driver without the writable mode refuses it");
    assert!(matches!(
        error.current_context(),
        SandboxError::InvalidSpec { field, .. } if field == "root_mode"
    ));
    spec.validate(&full_capabilities())
        .expect("a writable ext4 image is valid");
}

#[test]
fn writable_lock() {
    let fs = FakeHostFs::new();
    fs.insert_file("/image.ext4", b"image");
    let first = fs.lock_exclusive(std::path::Path::new("/image.ext4"));
    let first = first.expect("the first lock succeeds");
    let second = fs.lock_exclusive(std::path::Path::new("/image.ext4"));
    assert!(second.is_err(), "a second writer is refused");
    drop(first);
    fs.lock_exclusive(std::path::Path::new("/image.ext4"))
        .expect("the lock is free after the first holder drops");
}

#[cfg(all(feature = "firecracker", target_os = "linux"))]
#[test]
fn firecracker_writable_drive() {
    use foundation_sandbox::native::firecracker::driver::{drive, FcPlan};
    use foundation_sandbox::shared::limits::SandboxLimits;
    use foundation_sandbox::shared::spec::FsType;

    let plan = |mode| FcPlan {
        id: "000000000001".parse().expect("the identifier"),
        run_dir: "/state/000000000001".into(),
        firecracker: "/bin/firecracker".into(),
        kernel: "/kernel".into(),
        initramfs: "/initramfs".into(),
        rootfs: "/image.ext4".into(),
        rootfs_type: FsType::Ext4,
        root_mode: mode,
        vcpus: 1,
        memory_mib: 512,
        guest_cid: 3,
        boot_args: String::new(),
        limits: SandboxLimits::default(),
        lock: None,
    };
    assert_eq!(
        drive(&plan(RootMode::Writable)).is_read_only,
        Some(false),
        "the writable drive is read-write"
    );
    assert_eq!(
        drive(&plan(RootMode::Overlay)).is_read_only,
        Some(true),
        "every other mode keeps the image read-only"
    );
}
