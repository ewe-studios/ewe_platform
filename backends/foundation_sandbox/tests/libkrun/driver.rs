//! The libkrun driver simulation tests (feature 09, section 7).
//!
//! The driver runs against a fake process spawner and an in-memory host file
//! system. No virtual machine starts, and no test waits on real time.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_sandbox::native::libkrun::driver::{
    capabilities, krun_exit_error, KrunHandle, LibkrunDriver, USER_ROOT_TAG,
};
use foundation_sandbox::shared::control::testing::{FakeHostFs, FakeProcessSpawner};
use foundation_sandbox::shared::driver::{
    Capabilities, CreateProgress, HostReport, SandboxDriver, SlaveEndpoint, SubstrateHandle,
};
use foundation_sandbox::shared::error::{SandboxError, SandboxResult};
use foundation_sandbox::shared::host::{ProcessSpawner, ProcessSpec, SlaveConnector};
use foundation_sandbox::shared::krun_config::{KrunMount, KrunRunnerConfig, KRUN_LOG_LEVEL_WARN};
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::spec::{RootMode, RootSource};

use super::support::{
    boot_root, host_config, install_host, install_host_with, progress_of, result_of, run_dir,
    run_launch, sandbox_id, seeded_fs, serial, sim_ctx, sim_spec,
};

/// A connector that fails every call. The driver must not use it.
struct FailingConnector;

impl SlaveConnector for FailingConnector {
    fn connect(
        &self,
        _ctx: &Ctx,
        _endpoint: &SlaveEndpoint,
        _limits: &SandboxLimits,
    ) -> SandboxResult<Arc<dyn foundation_connectrpc::shared::transport::Transport>> {
        Err(SandboxError::Io {
            op: "connect".to_string(),
            message: "the driver must not call the slave".to_string(),
        }
        .into())
    }
}

#[test]
fn krun_config_roundtrip() {
    let config = KrunRunnerConfig {
        libkrun_path: Some(PathBuf::from("/opt/homebrew/lib/libkrun.1.dylib")),
        vcpus: 2,
        memory_mib: 1024,
        boot_root: PathBuf::from("/state/000000000001/root"),
        user_root: Some(KrunMount {
            tag: "ewe-root".to_string(),
            host_path: PathBuf::from("/root"),
            read_only: true,
        }),
        slave_path: PathBuf::from("/bin/agent-slave"),
        slave_socket: PathBuf::from("/state/000000000001/slave.sock"),
        slave_port: 1024,
        tsi_inet: true,
        mounts: vec![KrunMount {
            tag: "ewe-m0".to_string(),
            host_path: PathBuf::from("/data"),
            read_only: false,
        }],
        disks: Vec::new(),
        log_level: KRUN_LOG_LEVEL_WARN,
        lifeline_fd: Some(3),
    };
    let json = serde_json::to_string(&config).expect("the config encodes");
    let back: KrunRunnerConfig = serde_json::from_str(&json).expect("the config decodes");
    assert_eq!(back, config);
}

#[test]
fn launch_ok() {
    let _guard = serial();
    let (ctx, _clock) = sim_ctx(7, Duration::from_millis(1));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    install_host(&ctx, &fs, &spawner, Arc::new(FailingConnector));
    let driver = LibkrunDriver::new();
    let deadline = ctx.clock().now() + Duration::from_secs(10);
    let items = run_launch(&driver, &ctx, sim_spec(), deadline);

    assert_eq!(
        progress_of(&items),
        vec![CreateProgress::Preparing, CreateProgress::SubstrateStarted],
        "the progress values come in the order of invariant I10"
    );
    let substrate = result_of(items).expect("the launch succeeds");
    assert_eq!(
        substrate.endpoint,
        SlaveEndpoint::Unix {
            path: run_dir().join("slave.sock"),
        }
    );
    assert_eq!(
        substrate.root.source,
        RootSource::VirtioFs {
            tag: USER_ROOT_TAG.to_string(),
        }
    );
    assert_eq!(substrate.root.mode, RootMode::Overlay);
    assert_eq!(
        substrate.mounts,
        Vec::<foundation_sandbox::shared::spec::GuestMount>::new()
    );

    let specs = spawner.specs();
    let record = specs.first().expect("one runner process");
    assert_eq!(record.program, PathBuf::from("/bin/sandbox-krun"));
    assert_eq!(
        record.args,
        vec![run_dir().join("krun.json").display().to_string()]
    );
    assert_eq!(record.cwd, run_dir());
    assert_eq!(record.stdout, run_dir().join("console.log"));
    assert_eq!(record.stderr, run_dir().join("console.log"));
    assert!(
        record.env.is_empty(),
        "the runner gets an empty environment"
    );
    assert_eq!(record.kill_with_parent, cfg!(target_os = "linux"));

    let json = fs
        .file(&run_dir().join("krun.json"))
        .expect("the driver writes krun.json");
    let config: KrunRunnerConfig = serde_json::from_slice(&json).expect("krun.json is valid");
    assert_eq!(config.vcpus, 1);
    assert_eq!(config.memory_mib, 512);
    assert_eq!(config.boot_root, boot_root());
    assert_eq!(
        config.user_root,
        Some(KrunMount {
            tag: USER_ROOT_TAG.to_string(),
            host_path: PathBuf::from("/root"),
            read_only: true,
        })
    );
    assert_eq!(config.slave_path, PathBuf::from("/bin/agent-slave"));
    assert_eq!(config.slave_socket, run_dir().join("slave.sock"));
    assert_eq!(config.slave_port, 1024);
    assert!(!config.tsi_inet);
    assert_eq!(config.mounts, Vec::<KrunMount>::new());
    assert_eq!(config.log_level, KRUN_LOG_LEVEL_WARN);
    assert_eq!(
        config.lifeline_fd,
        if cfg!(target_os = "macos") {
            Some(3)
        } else {
            None
        }
    );

    for directory in ["", "dev", "proc", "sys", "run"] {
        let path = if directory.is_empty() {
            boot_root()
        } else {
            boot_root().join(directory)
        };
        assert!(fs.has_dir(&path), "the boot root has {}", path.display());
    }
}

#[test]
fn launch_fault_matrix() {
    let _guard = serial();
    let driver = LibkrunDriver::new();

    // The root directory does not exist.
    {
        let (ctx, _clock) = sim_ctx(8, Duration::from_millis(1));
        let fs = FakeHostFs::new();
        fs.insert_file("/bin/sandbox-krun", b"runner");
        fs.insert_file("/bin/agent-slave", b"slave");
        let spawner = FakeProcessSpawner::new();
        install_host(&ctx, &fs, &spawner, Arc::new(FailingConnector));
        let deadline = ctx.clock().now() + Duration::from_secs(10);
        let result = result_of(run_launch(&driver, &ctx, sim_spec(), deadline));
        assert!(matches!(
            result,
            Err(error)
                if matches!(error.current_context(), SandboxError::InvalidSpec { field, .. } if field == "rootfs")
        ));
        assert!(spawner.specs().is_empty(), "no process starts");
        assert!(!fs.has_dir(&run_dir()), "no run directory is left");
    }

    // The runner is missing.
    {
        let (ctx, _clock) = sim_ctx(9, Duration::from_millis(1));
        let fs = FakeHostFs::new();
        fs.insert_dir("/root");
        fs.insert_file("/bin/agent-slave", b"slave");
        let spawner = FakeProcessSpawner::new();
        install_host(&ctx, &fs, &spawner, Arc::new(FailingConnector));
        let deadline = ctx.clock().now() + Duration::from_secs(10);
        let result = result_of(run_launch(&driver, &ctx, sim_spec(), deadline));
        assert!(matches!(
            result,
            Err(error)
                if matches!(error.current_context(), SandboxError::BinaryNotFound { .. })
        ));
        assert!(!fs.has_dir(&run_dir()), "no run directory is left");
    }

    // The spawn fails.
    {
        let (ctx, _clock) = sim_ctx(10, Duration::from_millis(1));
        let fs = seeded_fs();
        let spawner = FakeProcessSpawner::new();
        spawner.fail_next("the spawner is closed");
        install_host(&ctx, &fs, &spawner, Arc::new(FailingConnector));
        let deadline = ctx.clock().now() + Duration::from_secs(10);
        let result = result_of(run_launch(&driver, &ctx, sim_spec(), deadline));
        assert!(matches!(
            result,
            Err(error) if matches!(error.current_context(), SandboxError::Io { .. })
        ));
        assert!(!fs.has_dir(&run_dir()), "no run directory is left");
    }

    // The socket path is too long.
    {
        let (ctx, _clock) = sim_ctx(11, Duration::from_millis(1));
        let fs = seeded_fs();
        let spawner = FakeProcessSpawner::new();
        let mut config = host_config();
        config.state_dir = PathBuf::from("/tmp").join("a".repeat(100));
        let run_dir = config.state_dir.join(sandbox_id().as_str());
        install_host_with(&ctx, &fs, &spawner, Arc::new(FailingConnector), config);
        let deadline = ctx.clock().now() + Duration::from_secs(10);
        let result = result_of(run_launch(&driver, &ctx, sim_spec(), deadline));
        assert!(matches!(
            result,
            Err(error)
                if matches!(error.current_context(), SandboxError::SocketPathTooLong { .. })
        ));
        assert!(!fs.has_dir(&run_dir), "no run directory is left");
    }
}

#[test]
fn exit_code_mapping() {
    let tail = b"sandbox-krun: console line\n".to_vec();

    let error = krun_exit_error(Some(64), tail.clone());
    assert!(matches!(
        error,
        SandboxError::InvalidSpec { ref field, .. } if field == "spec"
    ));

    let error = krun_exit_error(Some(70), tail.clone());
    assert!(matches!(
        error,
        SandboxError::SubstrateExited { code: Some(70), .. }
    ));

    let error = krun_exit_error(
        Some(71),
        b"sandbox-krun: cannot load libkrun: no file".to_vec(),
    );
    assert!(matches!(
        error,
        SandboxError::LibraryNotFound { ref name } if name == "libkrun"
    ));

    let error = krun_exit_error(
        Some(71),
        b"sandbox-krun: libkrun has no symbol krun_add_vsock\n".to_vec(),
    );
    assert!(matches!(
        error,
        SandboxError::LibraryTooOld { ref symbol } if symbol == "krun_add_vsock"
    ));

    for code in [0, 125, 126, 127, 137, 42] {
        let error = krun_exit_error(Some(code), tail.clone());
        assert!(
            matches!(
                error,
                SandboxError::SubstrateExited {
                    code: Some(found),
                    ..
                } if found == code
            ),
            "code {code} maps to SubstrateExited"
        );
    }

    let error = krun_exit_error(None, tail.clone());
    assert!(matches!(
        error,
        SandboxError::SubstrateExited { code: None, .. }
    ));
}

#[cfg(feature = "control")]
#[test]
fn create_runner_exit_71() {
    use foundation_sandbox::shared::control::testing::{in_memory_connector, ScriptedSlave};
    use foundation_sandbox::shared::control::{
        ManagerConfig, MemoryRecordStore, SandboxManager, SandboxProvider,
    };

    let _guard = serial();
    // The timer answers the probe waits and the create deadline at once.
    let (ctx, _clock) = sim_ctx(12, Duration::from_secs(60));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    spawner.exit_next_with(71);
    script_probe(&spawner, &fs);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_info_pending();
    install_host(&ctx, &fs, &spawner, in_memory_connector(Arc::clone(&slave)));
    let manager = SandboxManager::new(
        ctx.clone(),
        vec![Arc::new(LibkrunDriver::new())],
        in_memory_connector(slave),
        Arc::new(MemoryRecordStore::new()),
        ManagerConfig::default(),
    );
    let result = crate::pool::block_on(manager.create(sim_spec()));
    match result {
        Err(error) => assert_eq!(
            error.current_context(),
            &SandboxError::LibraryNotFound {
                name: "libkrun".to_string(),
            }
        ),
        Ok(_) => panic!("a runner that exits with 71 must fail the create"),
    }
    assert!(!fs.has_dir(&run_dir()), "no run directory is left");
}

#[cfg(feature = "control")]
#[test]
fn create_slave_timeout() {
    use foundation_sandbox::shared::control::testing::{in_memory_connector, ScriptedSlave};
    use foundation_sandbox::shared::control::{
        ManagerConfig, MemoryRecordStore, SandboxManager, SandboxProvider,
    };

    let _guard = serial();
    // The timer answers the probe waits and the create deadline at once, and
    // Info never answers.
    let (ctx, _clock) = sim_ctx(13, Duration::from_secs(60));
    let fs = seeded_fs();
    let spawner = FakeProcessSpawner::new();
    script_probe(&spawner, &fs);
    let slave = Arc::new(ScriptedSlave::new());
    slave.with_info_pending();
    install_host(&ctx, &fs, &spawner, in_memory_connector(Arc::clone(&slave)));
    let manager = SandboxManager::new(
        ctx.clone(),
        vec![Arc::new(LibkrunDriver::new())],
        in_memory_connector(slave),
        Arc::new(MemoryRecordStore::new()),
        ManagerConfig::default(),
    );
    let result = crate::pool::block_on(manager.create(sim_spec()));
    match result {
        Err(error) => match error.current_context() {
            SandboxError::BootTimeout { stage, .. } => assert_eq!(stage, "slave"),
            other => panic!("expected BootTimeout, got {other:?}"),
        },
        Ok(_) => panic!("a slave that never answers must time out"),
    }
    let child = spawner.last_child().expect("one runner process");
    assert!(!child.is_live(), "the runner got SIGKILL");
    assert!(child.signals().contains(&libc::SIGKILL));
    assert!(!fs.has_dir(&run_dir()), "no run directory is left");
}

#[test]
fn probe_missing_runner() {
    let _guard = serial();
    // Every process wait resolves at once, so a probe never blocks a test.
    let (ctx, _clock) = sim_ctx(14, Duration::from_secs(60));
    let fs = FakeHostFs::new();
    fs.insert_dir("/root");
    fs.insert_file("/bin/agent-slave", b"slave");
    let spawner = FakeProcessSpawner::new();
    let mut config = host_config();
    config.krun_runner_path = Some(PathBuf::from("/bin/missing-sandbox-krun"));
    install_host_with(&ctx, &fs, &spawner, Arc::new(FailingConnector), config);
    let driver = LibkrunDriver::new();
    let report: HostReport = crate::pool::block_on(driver.probe(ctx.clone()));
    assert!(!report.usable);
    let check = report
        .checks
        .iter()
        .find(|check| check.name == "runner")
        .expect("the runner check");
    assert!(!check.ok);
}

#[test]
fn capabilities_are_the_section_4_4_values() {
    let caps: Capabilities = capabilities(8, false);
    assert!(caps.rootfs_directory);
    assert!(!caps.rootfs_ext4);
    assert!(!caps.rootfs_squashfs);
    assert!(!caps.rootfs_container_image);
    assert_eq!(caps.root_modes, vec![RootMode::Overlay, RootMode::ReadOnly]);
    assert!(caps.host_mounts);
    assert!(caps.network_none);
    assert!(caps.network_host);
    assert_eq!(caps.max_vcpus, 8);

    // Block support turns on the ext4 root and the writable mode.
    let with_blk = capabilities(8, true);
    assert!(with_blk.rootfs_ext4);
    assert_eq!(
        with_blk.root_modes,
        vec![RootMode::Overlay, RootMode::ReadOnly, RootMode::Writable]
    );
}

#[test]
fn handle_model() {
    let _guard = serial();
    // A short grace answers at once, so terminate sends SIGTERM.
    let (ctx, _clock) = sim_ctx(15, Duration::from_millis(100));
    let fs = seeded_fs();
    fs.insert_dir(run_dir());
    let spawner = FakeProcessSpawner::new();
    let child = spawner
        .spawn(ProcessSpec {
            program: PathBuf::from("/bin/sandbox-krun"),
            args: Vec::new(),
            env: Vec::new(),
            cwd: run_dir(),
            stdout: run_dir().join("console.log"),
            stderr: run_dir().join("console.log"),
            kill_with_parent: false,
            extra_fds: Vec::new(),
        })
        .expect("the child starts");
    let control = spawner.last_child().expect("the child control");
    #[cfg(target_os = "macos")]
    let handle = KrunHandle::new(
        ctx.clone(),
        Arc::from(child),
        Arc::new(fs.clone()),
        run_dir(),
        None,
        None,
    );
    #[cfg(not(target_os = "macos"))]
    let handle = KrunHandle::new(
        ctx.clone(),
        Arc::from(child),
        Arc::new(fs.clone()),
        run_dir(),
        None,
    );

    let exit =
        crate::pool::block_on(handle.terminate(Duration::from_millis(10))).expect("terminate");
    assert!(exit.forced);
    assert_eq!(exit.signal, Some(libc::SIGTERM));
    assert_eq!(control.signals(), vec![libc::SIGTERM]);

    let exit = crate::pool::block_on(handle.kill()).expect("kill after exit");
    assert!(exit.forced);

    crate::pool::block_on(handle.cleanup()).expect("cleanup");
    crate::pool::block_on(handle.cleanup()).expect("cleanup again");
    assert!(!fs.has_dir(&run_dir()));
}

#[test]
fn no_krun_symbols_outside_runner() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let allowed = [
        root.join("bin/sandbox_krun.rs"),
        root.join("native/libkrun/ffi.rs"),
    ];
    // The C entry points of `include/libkrun.h` on `stable-1.19.x`.
    let symbols = [
        "krun_set_log_level",
        "krun_create_ctx",
        "krun_set_vm_config",
        "krun_add_virtiofs3",
        "krun_fs_add_overlay_file",
        "krun_disable_implicit_vsock",
        "krun_add_vsock",
        "krun_add_vsock_port2",
        "krun_set_port_map",
        "krun_set_exec",
        "krun_start_enter",
        "krun_get_max_vcpus",
    ];
    for path in source_files(&root) {
        if allowed.contains(&path) {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read a source file");
        // A doc comment may name a symbol. Only code must not call one.
        let code: String = text
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("//")
                    && !trimmed.starts_with("/*")
                    && !trimmed.starts_with('*')
            })
            .collect::<Vec<_>>()
            .join("\n");
        for symbol in symbols {
            assert!(
                !code.contains(symbol),
                "{} must not call the libkrun symbol {symbol}",
                path.display()
            );
        }
    }
    let ffi = std::fs::read_to_string(root.join("native/libkrun/ffi.rs"))
        .expect("read the foreign function interface");
    for symbol in symbols {
        assert!(ffi.contains(symbol), "ffi.rs names {symbol}");
    }
}

/// Scripts the programs of the host probe, so a manager create can choose the
/// driver with no real machine.
fn script_probe(spawner: &FakeProcessSpawner, fs: &FakeHostFs) {
    #[cfg(target_os = "linux")]
    fs.insert_file("/dev/kvm", b"");
    spawner.with_program_output(
        fs.clone(),
        "sandbox-krun",
        "{\"library\":\"/fake/libkrun.dylib\",\"max_vcpus\":8}\n",
    );
    #[cfg(target_os = "macos")]
    {
        spawner.with_program_output(fs.clone(), "sysctl", "1\n");
        spawner.with_program_output(fs.clone(), "codesign", "com.apple.security.hypervisor\n");
    }
}

fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files.extend(source_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
    files
}
