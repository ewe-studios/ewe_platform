//! The control-plane rows of section 7 (feature 12): the manifest defaults, the
//! spec precedence, the architecture check and the start command.

use std::path::PathBuf;
use std::sync::Arc;

use foundation_sandbox::shared::control::testing::{ScriptedSlave, SimDriver};
use foundation_sandbox::shared::control::SandboxProvider;
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::images::ImageManifest;
use foundation_sandbox::shared::rpc::messages::ExecEventKind;
use foundation_sandbox::shared::spec::{ImageRef, RootFs, SandboxSpec};

use super::support;

const ROOTFS: &str = "/images/tool.ext4";
const TAG: &str = "tool";

fn image_spec() -> SandboxSpec {
    SandboxSpec::new(RootFs::Image(ImageRef::Tag(TAG.to_string())))
}

fn resolver_with(
    manifest: ImageManifest,
) -> Arc<dyn foundation_sandbox::shared::images::ImageResolver> {
    Arc::new(support::FixedResolver {
        rootfs: PathBuf::from(ROOTFS),
        manifest,
    })
}

struct CreatedImage {
    handle: foundation_sandbox::shared::control::SandboxHandle,
    driver: Arc<SimDriver>,
    slave: Arc<ScriptedSlave>,
    // The manager kills its sandboxes when it drops, so the test keeps it.
    _manager: foundation_sandbox::shared::control::SandboxManager,
}

fn create_image(
    seed: u64,
    resolver: Arc<dyn foundation_sandbox::shared::images::ImageResolver>,
) -> CreatedImage {
    let (ctx, _clock) = crate::control::support::ctx(seed);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let slave = Arc::new(ScriptedSlave::new());
    let manager = support::image_manager(ctx, Arc::clone(&driver), Arc::clone(&slave), resolver);
    let handle =
        crate::pool::block_on(manager.create(image_spec())).expect("the image sandbox is created");
    CreatedImage {
        handle,
        driver,
        slave,
        _manager: manager,
    }
}

#[test]
fn image_defaults_apply() {
    let _serial = crate::control::support::serial();
    let manifest = support::manifest(foundation_sandbox::shared::spec::GuestArch::host());
    let created = create_image(93, resolver_with(manifest));
    let launched = created
        .driver
        .launch_spec()
        .expect("the driver received a spec");
    assert_eq!(launched.rootfs, RootFs::Ext4Image(PathBuf::from(ROOTFS)));
    assert_eq!(
        launched.workdir, "/srv",
        "the manifest work directory applies"
    );
    assert!(
        launched.env.contains(&("A".to_string(), "1".to_string())),
        "the manifest environment applies"
    );
}

#[test]
fn spec_wins() {
    let _serial = crate::control::support::serial();
    let manifest = support::manifest(foundation_sandbox::shared::spec::GuestArch::host());
    let (ctx, _clock) = crate::control::support::ctx(94);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let slave = Arc::new(ScriptedSlave::new());
    let manager = support::image_manager(ctx, Arc::clone(&driver), slave, resolver_with(manifest));
    let mut spec = image_spec();
    spec.workdir = "/tmp".to_string();
    crate::pool::block_on(manager.create(spec)).expect("the sandbox is created");
    let launched = driver.launch_spec().expect("the driver received a spec");
    assert_eq!(launched.workdir, "/tmp", "the spec work directory wins");
}

#[test]
fn image_arch_mismatch() {
    let _serial = crate::control::support::serial();
    let mut manifest = support::manifest(foundation_sandbox::shared::spec::GuestArch::host());
    manifest.arch = support::other_arch();
    let (ctx, _clock) = crate::control::support::ctx(95);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let slave = Arc::new(ScriptedSlave::new());
    let manager = support::image_manager(ctx, Arc::clone(&driver), slave, resolver_with(manifest));
    let error = crate::control::support::expect_error(
        crate::pool::block_on(manager.create(image_spec())),
        "an image of another architecture",
    );
    assert!(matches!(error, SandboxError::ImageArchMismatch { .. }));
    assert_eq!(driver.launch_calls(), 0, "no driver starts");
}

#[test]
fn start_cmd_runs_the_manifest_command() {
    let _serial = crate::control::support::serial();
    let manifest = support::manifest(foundation_sandbox::shared::spec::GuestArch::host());
    let expected = manifest.cmd.clone().expect("the manifest has a command");
    let created = create_image(96, resolver_with(manifest));
    created.slave.with_exec_events(vec![
        ExecEventKind::Started { pid: 7 },
        ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    ]);
    let _exec = crate::pool::block_on(created.handle.start_cmd()).expect("the start command runs");
    let argv = created
        .slave
        .calls()
        .into_iter()
        .find_map(|call| match call {
            foundation_sandbox::shared::control::testing::SlaveCall::Exec { argv, .. } => {
                Some(argv)
            }
            _ => None,
        });
    assert_eq!(argv, Some(expected));
}

#[test]
fn start_cmd_without_a_manifest_command() {
    let _serial = crate::control::support::serial();
    let mut manifest = support::manifest(foundation_sandbox::shared::spec::GuestArch::host());
    manifest.cmd = None;
    let created = create_image(97, resolver_with(manifest));
    let error = crate::control::support::expect_error(
        crate::pool::block_on(created.handle.start_cmd()),
        "a manifest without a command",
    );
    assert!(matches!(
        error,
        SandboxError::InvalidSpec { ref field, .. } if field == "cmd"
    ));
}

#[test]
fn unpack_limit_becomes_artifact_too_large() {
    let _serial = crate::control::support::serial();
    let manifest = support::manifest(foundation_sandbox::shared::spec::GuestArch::host());
    let created = create_image(99, resolver_with(manifest));
    created.slave.with_unpack_error(SandboxError::SlaveError {
        code: foundation_sandbox::shared::error::SlaveErrorCode::Io,
        errno: Some(libc::ENOSPC),
        message: format!(
            "{}: more than 4 bytes",
            foundation_sandbox::shared::unpack::UNPACK_LIMIT_MESSAGE
        ),
    });
    let tar = futures::stream::iter(vec![Ok(vec![0_u8; 8])]);
    let error = crate::control::support::expect_error(
        crate::pool::block_on(created.handle.unpack(
            "/",
            tar,
            foundation_sandbox::shared::unpack::UnpackOptions::default(),
        )),
        "an unpack over the limit",
    );
    assert!(
        matches!(error, SandboxError::ArtifactTooLarge { .. }),
        "the control plane maps the slave limit to ArtifactTooLarge: {error:?}"
    );
}

#[test]
fn image_without_a_resolver() {
    let _serial = crate::control::support::serial();
    let (ctx, _clock) = crate::control::support::ctx(98);
    let driver = Arc::new(SimDriver::new(support::driver_id()));
    let slave = Arc::new(ScriptedSlave::new());
    let store = Arc::new(foundation_sandbox::shared::control::MemoryRecordStore::new());
    let manager = foundation_sandbox::shared::control::SandboxManager::new(
        ctx,
        vec![driver as Arc<dyn foundation_sandbox::shared::driver::SandboxDriver>],
        foundation_sandbox::shared::control::testing::in_memory_connector(slave),
        store as Arc<dyn foundation_sandbox::shared::control::RecordStore>,
        foundation_sandbox::shared::control::ManagerConfig::default(),
    );
    let error = crate::control::support::expect_error(
        crate::pool::block_on(manager.create(image_spec())),
        "a manager without an image resolver",
    );
    assert!(matches!(
        error,
        SandboxError::InvalidSpec { ref field, .. } if field == "rootfs"
    ));
}
