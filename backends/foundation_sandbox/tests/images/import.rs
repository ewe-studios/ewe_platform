//! The container image source tests (feature 13, section 7).
//!
//! The pure rows (`config_to_manifest`, `key_is_stable`) need no sandbox. The
//! simulation rows use the fake Docker Engine of the Docker driver tests, the
//! simulated driver and the scripted slave, so no daemon and no virtual
//! machine runs.
//!
//! The end-to-end rows of section 7 (`e2e_import_*`) need a Linux host with
//! KVM and Docker; they belong to `tests/e2e_images.rs`. This module covers the
//! simulation and the fault table, which run everywhere.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use foundation_core::valtron::{execute, StreamIteratorExt as _};
use foundation_deployment_docker::generated::json::ImageConfig;
use foundation_sandbox::native::images::import::{
    config_to_manifest as manifest_from_config, export_unpack_options, image_key, ImageImporter,
    ImageSource, ImportEvent, ImportOptions, ImportProgress,
};
use foundation_sandbox::native::images::ImageStore;
use foundation_sandbox::shared::control::testing::{
    in_memory_connector, FakeHostFs, FakeProcessSpawner, ScriptedSlave, SimDriver,
};
use foundation_sandbox::shared::control::{MemoryRecordStore, SandboxManager, SandboxProvider};
use foundation_sandbox::shared::driver::SandboxDriver;
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::host::{HostConfig, HostFs as _, SandboxHost};
use foundation_sandbox::shared::ids::DriverId;
use foundation_sandbox::shared::limits::{ArtifactLimits, ImageLimits};
use foundation_sandbox::shared::spec::{GuestArch, ImageRef};
use serde_json::json;

use crate::docker::support::{Call, ExportBody, Failure, FakeDockerApi, Op};

/// The image that the fake daemon has.
const IMAGE: &str = "python:3.12-slim";

/// One import harness: a fake host, a fake Engine and a simulated control plane.
struct Harness {
    /// The clock that the killed-stop case advances.
    clock: Arc<foundation_core::valtron::SimulatedClock>,
    /// The importer under test.
    importer: ImageImporter,
    /// The fake Engine.
    docker: FakeDockerApi,
    /// The simulated driver.
    driver: Arc<SimDriver>,
    /// The scripted slave.
    slave: Arc<ScriptedSlave>,
    /// The fake host file system.
    fs: FakeHostFs,
    /// The image store.
    store: ImageStore,
}

impl Harness {
    /// A harness with the fake daemon that has `IMAGE`.
    fn new(seed: u64) -> Self {
        Self::build(
            seed,
            Arc::new(SimDriver::new(DriverId::Firecracker)),
            Arc::new(ScriptedSlave::new()),
            FakeDockerApi::new(),
            Some("/bin/mkfs.ext4"),
            true,
        )
    }

    /// A harness over the given parts.
    fn build(
        seed: u64,
        driver: Arc<SimDriver>,
        slave: Arc<ScriptedSlave>,
        docker: FakeDockerApi,
        mkfs: Option<&str>,
        shutdown_exits: bool,
    ) -> Self {
        let (ctx, clock) = crate::control::support::ctx(seed);
        if shutdown_exits {
            slave.with_shutdown_exit(driver.substrate());
        }
        let fs = FakeHostFs::new();
        fs.insert_file("/bin/agent-slave", b"slave");
        if mkfs.is_some() {
            fs.insert_file("/bin/mkfs.ext4", b"mkfs");
        }
        let spawner = Arc::new(FakeProcessSpawner::new());
        spawner.with_program_output(fs.clone(), "mkfs.ext4", "");
        let config = HostConfig {
            state_dir: "/state".into(),
            cache_dir: "/cache".into(),
            slave_path: Some(PathBuf::from("/bin/agent-slave")),
            firecracker_path: None,
            krun_runner_path: None,
            libkrun_path: None,
            mkfs_ext4_path: mkfs.map(PathBuf::from),
            driver_order: vec![DriverId::Firecracker],
            artifact_limits: ArtifactLimits::default(),
            image_limits: ImageLimits::default(),
        };
        let host = SandboxHost {
            processes: spawner,
            fs: Arc::new(fs.clone()),
            connector: Arc::new(super::support::NoConnector),
            config: Arc::new(config),
        };
        let store = ImageStore::new(ctx.clone(), host.clone());
        let manager: Arc<dyn SandboxProvider> = Arc::new(SandboxManager::new(
            ctx.clone(),
            vec![Arc::clone(&driver) as Arc<dyn SandboxDriver>],
            in_memory_connector(Arc::clone(&slave)),
            Arc::new(MemoryRecordStore::new()),
            foundation_sandbox::shared::control::ManagerConfig::default(),
        ));
        let importer = ImageImporter::new(
            ctx.clone(),
            host,
            store.clone(),
            Arc::clone(&manager),
            Some(docker.api()),
        );
        // The fake daemon answers for this host, so the arch check passes
        // unless a test changes the answer.
        docker.set_arch(match GuestArch::host() {
            GuestArch::X86_64 => "amd64",
            GuestArch::Aarch64 => "arm64",
        });
        Self {
            clock,
            importer,
            docker,
            driver,
            slave,
            fs,
            store,
        }
    }

    /// The default import options with a tag.
    fn options() -> ImportOptions {
        ImportOptions {
            tag: Some("python".to_string()),
            ..ImportOptions::default()
        }
    }

    /// Imports the Docker image.
    fn import(
        &self,
    ) -> foundation_sandbox::shared::error::SandboxResult<
        foundation_sandbox::native::images::import::ImportReport,
    > {
        crate::pool::block_on(
            self.importer
                .import(ImageSource::Docker(IMAGE.to_string()), Self::options()),
        )
    }

    /// The number of export chunks that the fake Engine served.
    fn export_calls(&self) -> usize {
        self.docker.count(|call| matches!(call, Call::Export(_)))
    }

    /// The number of temporary containers that the fake Engine made.
    fn create_calls(&self) -> usize {
        self.docker
            .count(|call| matches!(call, Call::Create { .. }))
    }
}

// ── configuration to manifest (invariant I7, section 6.3) ──────────────────

#[test]
fn config_to_manifest() {
    let options = ImportOptions::default();
    let config = ImageConfig {
        entrypoint: Some(vec!["/app".to_string()]),
        cmd: Some(vec!["--serve".to_string()]),
        env: Some(vec!["A=1".to_string()]),
        working_dir: Some("/srv".to_string()),
        user: Some("1000".to_string()),
        exposed_ports: Some(json!({"8080/tcp": {}})),
        volumes: Some(json!({"/data": {}})),
        ..ImageConfig::default()
    };
    let (manifest, dropped) =
        manifest_from_config(&config, &options, GuestArch::host(), "docker:x@y");
    assert_eq!(
        manifest.cmd,
        Some(vec!["/app".to_string(), "--serve".to_string()])
    );
    assert_eq!(manifest.env, vec![("A".to_string(), "1".to_string())]);
    assert_eq!(manifest.workdir.as_deref(), Some("/srv"));
    assert_eq!(manifest.user.as_deref(), Some("1000"));
    assert_eq!(manifest.source, "docker:x@y");
    assert_eq!(dropped, vec!["ExposedPorts", "Volumes"]);
    assert_eq!(manifest.arch, GuestArch::host());

    // Empty values give an empty manifest, and no field is dropped.
    let (empty, dropped_empty) =
        manifest_from_config(&ImageConfig::default(), &options, GuestArch::host(), "s");
    assert_eq!(empty.cmd, None);
    assert_eq!(empty.env, Vec::new());
    assert_eq!(empty.workdir, None);
    assert_eq!(empty.user, None);
    assert_eq!(dropped_empty, Vec::<String>::new());

    // An environment entry splits at the first `=`.
    let config = ImageConfig {
        env: Some(vec!["B=x=y".to_string()]),
        ..ImageConfig::default()
    };
    let (manifest, _) = manifest_from_config(&config, &options, GuestArch::host(), "s");
    assert_eq!(manifest.env, vec![("B".to_string(), "x=y".to_string())]);

    // The options replace the command and add environment entries.
    let options = ImportOptions {
        cmd: Some(vec!["/bin/sh".to_string()]),
        env: vec![("C".to_string(), "3".to_string())],
        ..ImportOptions::default()
    };
    let config = ImageConfig {
        entrypoint: Some(vec!["/app".to_string()]),
        cmd: Some(vec!["--serve".to_string()]),
        env: Some(vec!["A=1".to_string()]),
        ..ImageConfig::default()
    };
    let (manifest, _) = manifest_from_config(&config, &options, GuestArch::host(), "s");
    assert_eq!(manifest.cmd, Some(vec!["/bin/sh".to_string()]));
    assert_eq!(
        manifest.env,
        vec![
            ("A".to_string(), "1".to_string()),
            ("C".to_string(), "3".to_string())
        ]
    );
}

// ── the export entries (section 6.4) ───────────────────────────────────────

#[test]
fn export_unpack_entries() {
    let options = export_unpack_options();
    // A container export is a flat tree, so it has no whiteouts.
    assert!(!options.oci_whiteouts);
    // `/.dockerenv` is the one entry that a sandbox root does not need. The
    // `/etc` runtime files and `/dev` entries stay.
    assert_eq!(options.skip, vec![".dockerenv".to_string()]);
}

// ── the image key (section 4.1) ────────────────────────────────────────────

#[test]
fn key_is_stable() {
    let arch = GuestArch::host();
    let key = image_key("docker:x@y", arch, 4096);
    assert_eq!(key.len(), 32);
    assert_eq!(key, image_key("docker:x@y", arch, 4096));
    assert_ne!(key, image_key("docker:x@z", arch, 4096));
    assert_ne!(
        key,
        image_key("docker:x@y", super::support::other_arch(), 4096)
    );
    assert_ne!(key, image_key("docker:x@y", arch, 256));
}

// ── the import story (invariants I3 to I6) ────────────────────────────────

#[test]
fn import_sim_ok() {
    let _serial = crate::control::support::serial();
    let harness = Harness::new(301);
    harness.docker.add_image(IMAGE);
    harness.docker.set_image_config(
        IMAGE,
        ImageConfig {
            entrypoint: Some(vec!["/app".to_string()]),
            cmd: Some(vec!["--serve".to_string()]),
            env: Some(vec!["A=1".to_string()]),
            working_dir: Some("/srv".to_string()),
            user: Some("1000".to_string()),
            exposed_ports: Some(json!({"8080/tcp": {}})),
            ..ImageConfig::default()
        },
    );
    harness
        .docker
        .set_export(ExportBody::Bytes(vec![0_u8; 1024]));

    let report = harness.import().expect("the import succeeds");
    assert!(!report.cached);
    assert_eq!(
        report.manifest.cmd,
        Some(vec!["/app".to_string(), "--serve".to_string()])
    );
    assert_eq!(
        report.manifest.env,
        vec![("A".to_string(), "1".to_string())]
    );
    assert_eq!(report.manifest.workdir.as_deref(), Some("/srv"));
    assert_eq!(report.manifest.user.as_deref(), Some("1000"));
    assert_eq!(report.dropped_config, vec!["ExposedPorts"]);
    assert_eq!(report.image, ImageRef::Tag("python".to_string()));
    assert!(
        report
            .manifest
            .source
            .starts_with("docker:python:3.12-slim@"),
        "the source names the image identifier"
    );
    assert_eq!(
        report.manifest.rootfs_size, 0,
        "the fake work file is empty"
    );

    // The tag resolves to the committed image.
    let (rootfs, manifest) =
        crate::pool::block_on(harness.store.resolve(&report.image)).expect("the tag resolves");
    assert!(rootfs.ends_with("rootfs.ext4"));
    assert_eq!(manifest.key, report.manifest.key);

    // The temporary container was removed; the fill sandbox ran one time.
    assert_eq!(harness.create_calls(), 1);
    assert_eq!(harness.export_calls(), 1);
    assert!(
        harness
            .docker
            .count(|call| matches!(call, Call::Remove(_, true)))
            >= 1,
        "the temporary container is removed"
    );
    assert_eq!(harness.driver.launch_calls(), 1, "one fill sandbox");
}

#[test]
fn import_cache_hit() {
    let _serial = crate::control::support::serial();
    let harness = Harness::new(302);
    harness.docker.add_image(IMAGE);
    harness.docker.set_export(ExportBody::Bytes(vec![0_u8; 64]));

    let first = harness.import().expect("the first import");
    let second = harness.import().expect("the second import");
    assert!(!first.cached);
    assert!(second.cached);
    assert_eq!(first.manifest.key, second.manifest.key);
    assert_eq!(first.image, second.image);
    assert_eq!(harness.create_calls(), 1, "a cache hit makes no container");
    assert_eq!(harness.export_calls(), 1, "a cache hit exports nothing");
    assert_eq!(
        harness.driver.launch_calls(),
        1,
        "a cache hit starts nothing"
    );
}

#[test]
fn import_arch_mismatch() {
    let _serial = crate::control::support::serial();
    let harness = Harness::new(303);
    harness.docker.add_image(IMAGE);
    harness
        .docker
        .set_arch(if GuestArch::host() == GuestArch::X86_64 {
            "arm64"
        } else {
            "amd64"
        });
    let error = crate::control::support::expect_error(harness.import(), "another architecture");
    assert!(matches!(error, SandboxError::ImageArchMismatch { .. }));
    assert_eq!(harness.create_calls(), 0, "no container is made");
    assert_eq!(harness.export_calls(), 0, "nothing is exported");
}

#[test]
fn import_streams() {
    let _serial = crate::control::support::serial();
    let harness = Harness::new(304);
    harness.docker.add_image(IMAGE);
    // 200 MiB in 64 KiB chunks: the fake makes each chunk as it is read, so the
    // export is never held whole.
    let chunk = 64 * 1024;
    let count = 200 * 1024 * 1024 / chunk;
    harness
        .docker
        .set_export(ExportBody::Chunks { count, size: chunk });

    let mut stream = harness
        .importer
        .import_with_progress(ImageSource::Docker(IMAGE.to_string()), Harness::options());
    let mut filling = Vec::new();
    let report = crate::pool::block_on(async {
        while let Some(event) = stream.next().await {
            match event {
                ImportEvent::Progress(ImportProgress::Filling { bytes }) => filling.push(bytes),
                ImportEvent::Progress(_) => {}
                ImportEvent::Done(result) => return *result,
            }
        }
        panic!("the import stream ended without a result");
    })
    .expect("the import succeeds");

    assert!(!report.cached);
    assert_eq!(harness.export_calls(), 1);
    // The importer reports one step per MiB, and one step can carry at most one
    // extra chunk. Its peak buffer stays below 1 MiB plus one 64 KiB chunk.
    assert!(!filling.is_empty(), "the fill sends progress");
    for (index, bytes) in filling.iter().enumerate() {
        let previous = if index == 0 { 0 } else { filling[index - 1] };
        assert!(
            bytes - previous <= 1024 * 1024 + chunk as u64,
            "a progress step stays within one MiB and one chunk: {bytes} - {previous}"
        );
    }
    assert_eq!(
        *filling.last().expect("the last step"),
        200 * 1024 * 1024,
        "the export streamed 200 MiB"
    );
}

// ── the fault table (section 6.2) ──────────────────────────────────────────

#[test]
fn import_fault_matrix() {
    let _serial = crate::control::support::serial();
    fault_identify_missing();
    fault_identify_missing_tar();
    fault_export();
    fault_mkfs();
    fault_start();
    fault_full_image();
}

/// Identify: the daemon does not have the image, and the pull is off.
fn fault_identify_missing() {
    let harness = Harness::new(311);
    let options = ImportOptions {
        pull: false,
        ..Harness::options()
    };
    let error = crate::control::support::expect_error(
        crate::pool::block_on(
            harness
                .importer
                .import(ImageSource::Docker(IMAGE.to_string()), options),
        ),
        "a missing image",
    );
    assert!(matches!(
        error,
        SandboxError::SubstrateApi { status: 404, .. }
    ));
    assert_eq!(harness.create_calls(), 0);
}

/// Identify: a tar file that does not exist.
fn fault_identify_missing_tar() {
    let harness = Harness::new(312);
    let error = crate::control::support::expect_error(
        crate::pool::block_on(harness.importer.import(
            ImageSource::Tar(PathBuf::from("/missing.tar")),
            Harness::options(),
        )),
        "a missing tar",
    );
    assert!(matches!(error, SandboxError::Io { .. }));
}

/// The export fails: the container is removed and no image is committed.
fn fault_export() {
    let harness = Harness::new(313);
    harness.docker.add_image(IMAGE);
    harness.docker.fail(
        Op::Export,
        Failure::Status(500, "the export failed".to_string()),
    );
    let error = crate::control::support::expect_error(harness.import(), "a failed export");
    assert!(matches!(
        error,
        SandboxError::SubstrateApi { status: 500, .. }
    ));
    assert_eq!(harness.create_calls(), 1);
    assert!(
        harness
            .docker
            .count(|call| matches!(call, Call::Remove(_, true)))
            >= 1,
        "the temporary container is removed"
    );
    assert!(
        crate::pool::block_on(harness.store.list())
            .expect("the store lists")
            .is_empty(),
        "no image is committed"
    );
}

/// `create_blank` fails: no `mkfs.ext4` and no container left.
fn fault_mkfs() {
    let driver = Arc::new(SimDriver::new(DriverId::Firecracker));
    let slave = Arc::new(ScriptedSlave::new());
    let docker = FakeDockerApi::new();
    docker.add_image(IMAGE);
    let harness = Harness::build(314, driver, slave, docker, None, true);
    let error = crate::control::support::expect_error(harness.import(), "no mkfs");
    assert!(matches!(error, SandboxError::BinaryNotFound { .. }));
    assert!(
        harness
            .docker
            .count(|call| matches!(call, Call::Remove(_, true)))
            >= 1,
        "the temporary container is removed"
    );
}

/// The fill sandbox does not start: the work file is removed.
fn fault_start() {
    let driver = Arc::new(SimDriver::new(DriverId::Firecracker).with_launch_error(
        SandboxError::Io {
            op: "start".to_string(),
            message: "the substrate did not start".to_string(),
        },
    ));
    let slave = Arc::new(ScriptedSlave::new());
    let docker = FakeDockerApi::new();
    docker.add_image(IMAGE);
    let harness = Harness::build(315, driver, slave, docker, Some("/bin/mkfs.ext4"), true);
    let error = crate::control::support::expect_error(harness.import(), "a start failure");
    assert!(matches!(error, SandboxError::Io { .. }));
    assert!(
        harness
            .fs
            .read_dir(&PathBuf::from("/cache/images/work"))
            .expect("the work directory")
            .is_empty(),
        "the work file is removed"
    );
    assert!(
        harness
            .docker
            .count(|call| matches!(call, Call::Remove(_, true)))
            >= 1,
        "the temporary container is removed"
    );
}

/// An unpack that fills the image: the error names the size limit.
fn fault_full_image() {
    let harness = Harness::new(316);
    harness.docker.add_image(IMAGE);
    harness.slave.with_unpack_error(SandboxError::SlaveError {
        code: foundation_sandbox::shared::error::SlaveErrorCode::Io,
        errno: Some(libc::ENOSPC),
        message: "the unpack of big failed: no space left on device".to_string(),
    });
    let error = crate::control::support::expect_error(harness.import(), "a full image");
    match error {
        SandboxError::ArtifactTooLarge { message, .. } => {
            assert!(
                message.contains("size_mib"),
                "the error names the size limit: {message}"
            );
        }
        other => panic!("the error is ArtifactTooLarge: {other:?}"),
    }
    assert!(
        crate::pool::block_on(harness.store.list())
            .expect("the store lists")
            .is_empty(),
        "no image is committed"
    );
}

#[test]
fn import_stop_killed() {
    let _serial = crate::control::support::serial();
    let driver = Arc::new(SimDriver::new(DriverId::Firecracker));
    let slave = Arc::new(ScriptedSlave::new());
    // No `with_shutdown_exit`: the stop reaches the grace and forces the
    // substrate down.
    let docker = FakeDockerApi::new();
    docker.add_image(IMAGE);
    let harness = Harness::build(
        317,
        driver,
        slave.clone(),
        docker,
        Some("/bin/mkfs.ext4"),
        false,
    );

    let importer = harness.importer.clone();
    let task = execute(
        foundation_core::valtron::from_future(async move {
            importer
                .import(ImageSource::Docker(IMAGE.to_string()), Harness::options())
                .await
        }),
        None,
    )
    .expect("the import is scheduled");
    let mut attempts = 0;
    while slave.call_count("shutdown") == 0 && attempts < 100_000 {
        std::thread::yield_now();
        attempts += 1;
    }
    harness.clock.advance(Duration::from_secs(6));
    let (result, _rest) = crate::pool::block_on(task.into_ready_future()).expect("ready");
    let error = crate::control::support::expect_error(result, "a forced stop");
    assert!(
        matches!(error, SandboxError::Io { ref op, .. } if op == "unmount image"),
        "the error names the unmount: {error:?}"
    );
    assert!(
        crate::pool::block_on(harness.store.list())
            .expect("the store lists")
            .is_empty(),
        "no image is committed"
    );
}

// ── a tar source (simulation) ──────────────────────────────────────────────

#[test]
fn import_tar_source() {
    let _serial = crate::control::support::serial();
    let harness = Harness::new(318);
    // The scripted slave accepts any bytes; the tar reader is feature 12's.
    harness.fs.insert_file("/images/root.tar", b"archive");
    let options = ImportOptions {
        tag: Some("tar-image".to_string()),
        cmd: Some(vec!["/bin/sh".to_string()]),
        ..ImportOptions::default()
    };
    let report = crate::pool::block_on(
        harness
            .importer
            .import(ImageSource::Tar(PathBuf::from("/images/root.tar")), options),
    )
    .expect("the tar import succeeds");
    assert!(!report.cached);
    assert_eq!(report.manifest.cmd, Some(vec!["/bin/sh".to_string()]));
    assert!(
        report.manifest.source.starts_with("tar:sha256:"),
        "the source is the tar hash"
    );
    assert_eq!(report.manifest.arch, GuestArch::host());
    // A tar source needs no Docker call.
    assert_eq!(harness.export_calls(), 0);
}

// ── the command line (section 6.5) ─────────────────────────────────────────

/// The command line exists only in a build with the `cli` feature.
#[cfg(feature = "cli")]
#[test]
fn cli_image_parse() {
    use clap::Parser as _;
    use foundation_sandbox::cli::sandbox::{SandboxCli, SandboxCommand};

    let cli = SandboxCli::try_parse_from([
        "agent-sandbox",
        "image",
        "import",
        "docker:python:3.12-slim",
        "--tag",
        "python",
        "--size-mib",
        "256",
        "--cmd",
        "[\"/bin/sh\", \"-c\", \"echo ok\"]",
        "--env",
        "A=1",
        "--no-pull",
    ])
    .expect("the image import command parses");
    let SandboxCommand::Image(args) = cli.command else {
        panic!("the command is not image");
    };
    let foundation_sandbox::cli::sandbox::ImageCommand::Import(import) = args.command else {
        panic!("the sub-command is not import");
    };
    assert_eq!(import.source, "docker:python:3.12-slim");
    assert_eq!(import.tag.as_deref(), Some("python"));
    assert_eq!(import.size_mib, Some(256));
    assert!(import.no_pull);
    assert_eq!(import.env, vec!["A=1".to_string()]);

    for command in [
        vec!["agent-sandbox", "image", "list"],
        vec!["agent-sandbox", "image", "rm", "python"],
        vec![
            "agent-sandbox",
            "run",
            "--image",
            "python",
            "--",
            "python3",
            "-c",
            "print(1)",
        ],
    ] {
        SandboxCli::try_parse_from(command).expect("the command parses");
    }
}
