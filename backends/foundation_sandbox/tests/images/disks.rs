//! The libkrun disk rows of section 7 (feature 12): the runner configuration
//! and the probe's block support. No libkrun library runs here.

#![cfg(all(unix, feature = "libkrun"))]

use foundation_sandbox::native::libkrun::capabilities;
use foundation_sandbox::shared::krun_config::{KrunDisk, KrunProbe, KrunRunnerConfig};
use foundation_sandbox::shared::spec::{FsType, RootFs, RootSource};

use crate::libkrun::support;

#[test]
fn krun_config_disks() {
    let _guard = support::serial();
    let (ctx, _clock) = support::sim_ctx(21, std::time::Duration::from_millis(1));
    let fs = support::seeded_fs();
    fs.insert_file("/image.ext4", b"image");
    let spawner = foundation_sandbox::shared::control::testing::FakeProcessSpawner::new();
    support::install_host(
        &ctx,
        &fs,
        &spawner,
        std::sync::Arc::new(foundation_sandbox::native::host::NativeConnector),
    );
    let driver = foundation_sandbox::native::libkrun::LibkrunDriver::new();
    let spec = foundation_sandbox::shared::spec::SandboxSpec::new(RootFs::Ext4Image(
        std::path::PathBuf::from("/image.ext4"),
    ));
    let items = support::run_launch(
        &driver,
        &ctx,
        spec,
        ctx.clock().now() + std::time::Duration::from_secs(5),
    );
    let substrate = support::result_of(items).expect("the launch succeeds");
    assert_eq!(
        substrate.root.source,
        RootSource::BlockDevice {
            device: "/dev/vda".to_string(),
            fstype: FsType::Ext4,
        }
    );
    let json = fs
        .file(&support::run_dir().join("krun.json"))
        .expect("the driver writes krun.json");
    let config: KrunRunnerConfig = serde_json::from_slice(&json).expect("krun.json is valid");
    assert_eq!(config.user_root, None, "an ext4 root has no virtio-fs root");
    assert_eq!(
        config.disks,
        vec![KrunDisk {
            id: "rootfs".to_string(),
            path: std::path::PathBuf::from("/image.ext4"),
            read_only: true,
        }]
    );
}

#[test]
fn probe_reports_blk() {
    // The probe reads `krun_has_feature(1)` and maps it to the capability. A
    // host without the library still reports a shape that a caller can read.
    let without = capabilities(4, false);
    assert!(!without.rootfs_ext4);
    assert!(!without
        .root_modes
        .contains(&foundation_sandbox::shared::spec::RootMode::Writable));
    let with = capabilities(4, true);
    assert!(with.rootfs_ext4);
    assert!(with
        .root_modes
        .contains(&foundation_sandbox::shared::spec::RootMode::Writable));

    let line = r#"{"library":"libkrun.so.1","max_vcpus":4,"blk":true}"#;
    let probe: KrunProbe = serde_json::from_str(line).expect("the probe line decodes");
    assert!(probe.blk);
    assert_eq!(probe.max_vcpus, 4);
}
