//! The real microVM tests (feature 05).
//!
//! Each test calls `probe()` first. A host without `/dev/kvm`, the
//! `firecracker` program or the slave program prints the report and returns.
//! The continuous integration (CI) job of feature 06 sets
//! `EWE_SANDBOX_E2E_REQUIRED=1`, so a skipped run fails there.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::env;
use foundation_sandbox::native::artifacts::{sha256_hex, ArtifactStore};
use foundation_sandbox::native::firecracker::driver::FirecrackerDriver;
use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::control::{
    ManagerConfig, MemoryRecordStore, SandboxHandle, SandboxManager, SandboxProvider,
};
use foundation_sandbox::shared::driver::SandboxDriver;
use foundation_sandbox::shared::host::{HostConfig, SandboxHost};
use foundation_sandbox::shared::spec::{GuestArch, RootFs, RootMode, SandboxSpec};

/// A host that can run a microVM, or `None` with the reason.
fn usable_host() -> Option<(Ctx, SandboxHost)> {
    crate::pool::pool();
    let ctx = Ctx::host();
    let effects = env::system(&ctx).ok()?;
    ctx.store(effects);
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let config = HostConfig::discover(|name| std::env::var(name).ok(), &exe_dir);
    let host = SandboxHost::native(config);
    ctx.store(host.clone());
    let report = crate::pool::block_on(FirecrackerDriver::new().probe(ctx.clone()));
    if !report.usable {
        // In CI the environment variable turns a skip into a failure, so the
        // job cannot pass by running no microVM (feature 06, section 6.8).
        assert!(
            std::env::var("EWE_SANDBOX_E2E_REQUIRED").as_deref() != Ok("1"),
            "the CI runner cannot run Firecracker: {report:?}"
        );
        eprintln!("skip: the host cannot run Firecracker: {report:?}");
        return None;
    }
    Some((ctx, host))
}

/// The continuous integration root file system.
///
/// The kernel is fetched here too, outside the boot deadline. The driver then
/// finds both files in the cache, so a cold network does not count as a boot
/// time.
fn ci_rootfs(ctx: &Ctx, host: &SandboxHost) -> PathBuf {
    let store = ArtifactStore::new(ctx.clone(), host.fs.clone(), host.config.cache_dir.clone());
    let _ =
        crate::pool::block_on(store.firecracker_kernel(GuestArch::host())).expect("the CI kernel");
    crate::pool::block_on(store.firecracker_ci_rootfs(GuestArch::host()))
        .expect("the CI root file system")
}

/// A manager with this driver.
fn manager(ctx: &Ctx, host: &SandboxHost) -> Arc<SandboxManager> {
    Arc::new(SandboxManager::new(
        ctx.clone(),
        vec![Arc::new(FirecrackerDriver::new())],
        host.connector.clone(),
        Arc::new(MemoryRecordStore::default()),
        ManagerConfig::default(),
    ))
}

/// Creates one sandbox with the CI image.
fn create(manager: &SandboxManager, rootfs: &std::path::Path) -> SandboxHandle {
    crate::pool::block_on(manager.create(SandboxSpec::new(RootFs::SquashfsImage(
        rootfs.to_path_buf(),
    ))))
    .expect("the sandbox starts")
}

#[test]
fn e2e_command() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = ci_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let sandbox = create(&manager, &rootfs);
    let output = crate::pool::block_on(sandbox.run(Command::new(["echo", "hello"])))
        .expect("the command runs");
    assert_eq!(output.stdout, b"hello\n");
    assert_eq!(output.status.exit_code(), Some(0));
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_slave_is_pid1() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = ci_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let sandbox = create(&manager, &rootfs);
    let info = sandbox.info();
    let slave = info.slave.expect("the slave answered");
    assert!(slave.is_pid1, "the slave is the first process");
    let comm = crate::pool::block_on(sandbox.run(Command::new(["cat", "/proc/1/comm"])))
        .expect("the command runs");
    assert!(
        String::from_utf8_lossy(&comm.stdout).contains("agent-slave"),
        "the first process is the slave"
    );
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_image_not_changed() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = ci_rootfs(&ctx, &host);
    let before = sha256_hex(&std::fs::read(&rootfs).expect("the image"));
    let manager = manager(&ctx, &host);
    let sandbox = create(&manager, &rootfs);
    crate::pool::block_on(sandbox.run(Command::new(["sh", "-c", "echo x > /root/f"])))
        .expect("the command runs");
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
    let after = sha256_hex(&std::fs::read(&rootfs).expect("the image"));
    assert_eq!(before, after, "the image never changes");
}

#[test]
fn e2e_two_sandboxes_one_image() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = ci_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let first = create(&manager, &rootfs);
    let second = create(&manager, &rootfs);
    crate::pool::block_on(first.run(Command::new(["sh", "-c", "touch /tmp/a"])))
        .expect("the first writes");
    let output = crate::pool::block_on(second.run(Command::new(["sh", "-c", "test ! -e /tmp/a"])))
        .expect("the second runs");
    assert_eq!(
        output.status.exit_code(),
        Some(0),
        "the second does not see the write"
    );
    crate::pool::block_on(first.stop()).expect("the first stops");
    crate::pool::block_on(second.stop()).expect("the second stops");
}

#[test]
fn e2e_readonly_mode() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = ci_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let mut spec = SandboxSpec::new(RootFs::SquashfsImage(rootfs));
    spec.root_mode = RootMode::ReadOnly;
    let sandbox = crate::pool::block_on(manager.create(spec)).expect("the sandbox starts");
    let denied = crate::pool::block_on(sandbox.run(Command::new(["sh", "-c", "touch /etc/x"])))
        .expect("the command runs");
    assert_ne!(denied.status.exit_code(), Some(0), "/etc is read-only");
    let allowed =
        crate::pool::block_on(sandbox.run(Command::new(["sh", "-c", "touch /tmp/x && touch x"])))
            .expect("the command runs");
    assert_eq!(
        allowed.status.exit_code(),
        Some(0),
        "/tmp and the work directory write"
    );
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_no_network() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = ci_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let sandbox = create(&manager, &rootfs);
    let output =
        crate::pool::block_on(sandbox.run(Command::new(["sh", "-c", "ls /sys/class/net"])))
            .expect("the command runs");
    assert_eq!(output.stdout, b"lo\n", "the only interface is lo");
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_bad_listen_arg() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = ci_rootfs(&ctx, &host);
    let driver =
        FirecrackerDriver::new().with_boot_args("console=ttyS0 reboot=k panic=1 ewe.listen=bad");
    let manager = Arc::new(SandboxManager::new(
        ctx.clone(),
        vec![Arc::new(driver)],
        host.connector.clone(),
        Arc::new(MemoryRecordStore::default()),
        ManagerConfig::default(),
    ));
    let result =
        crate::pool::block_on(manager.create(SandboxSpec::new(RootFs::SquashfsImage(rootfs))));
    match result {
        Ok(_) => panic!("a guest with a bad listen address must fail"),
        Err(error) => {
            let text = format!("{error}");
            let log_tail = match error.current_context() {
                foundation_sandbox::shared::error::SandboxError::SubstrateExited {
                    log_tail,
                    ..
                }
                | foundation_sandbox::shared::error::SandboxError::BootTimeout {
                    log_tail, ..
                } => log_tail.clone(),
                other => panic!("expected a boot error, got {other:?}"),
            };
            assert!(
                String::from_utf8_lossy(&log_tail).contains("agent-slave"),
                "the console tail names the slave: {text}"
            );
        }
    }
}

#[test]
fn e2e_stop_is_clean() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = ci_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let sandbox = create(&manager, &rootfs);
    let id = sandbox.id().clone();
    let report = crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
    assert!(!report.killed, "the guest powered itself off");
    assert!(
        !host.config.state_dir.join(id.as_str()).exists(),
        "the run directory is gone"
    );
}

#[test]
fn e2e_boot_time() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = ci_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let started = ctx.clock().now();
    let sandbox = create(&manager, &rootfs);
    let elapsed = ctx.clock().now().saturating_duration_since(started);
    eprintln!("boot time: {} ms", elapsed.as_millis());
    assert!(
        elapsed < Duration::from_secs(10),
        "the create fits the boot timeout"
    );
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_ext4_root() {
    if std::process::Command::new("mkfs.ext4")
        .arg("-V")
        .output()
        .is_err()
    {
        eprintln!("skip: mkfs.ext4 is not installed");
        return;
    }
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let store = ArtifactStore::new(ctx.clone(), host.fs.clone(), host.config.cache_dir.clone());
    let tree =
        crate::pool::block_on(store.alpine_rootfs_dir(GuestArch::host())).expect("the Alpine tree");
    let image = host.config.cache_dir.join("test-root.ext4");
    let status = std::process::Command::new("mkfs.ext4")
        .args(["-q", "-F", "-d"])
        .arg(&tree)
        .arg(&image)
        .arg("64M")
        .status()
        .expect("mkfs.ext4 runs");
    assert!(status.success(), "mkfs.ext4 makes the image");
    let manager = manager(&ctx, &host);
    let sandbox =
        crate::pool::block_on(manager.create(SandboxSpec::new(RootFs::Ext4Image(image.clone()))))
            .expect("the ext4 sandbox starts");
    let output = crate::pool::block_on(sandbox.run(Command::new(["echo", "ext4"])))
        .expect("the command runs");
    assert_eq!(output.stdout, b"ext4\n");
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
    let _ = std::fs::remove_file(&image);
}

#[test]
fn e2e_kill_parent_kills_vm() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("ewe-e2e-parent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the temporary directory");
    let pid_file = dir.join("firecracker.pid");
    let mut helper = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args([
            "--exact",
            "firecracker::e2e::kill_parent_helper",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("EWE_TEST_E2E_PID_FILE", &pid_file)
        .spawn()
        .expect("the helper starts");
    let firecracker_pid = wait_for_pid(&ctx, &pid_file);
    helper.kill().expect("the helper is killed");
    let _ = helper.wait();
    let deadline = ctx.clock().now() + Duration::from_secs(2);
    while process_exists(firecracker_pid) {
        assert!(
            ctx.clock().now() < deadline,
            "the VM is still alive after its parent died"
        );
        std::thread::yield_now();
    }
    let _ = host;
    let _ = std::fs::remove_dir_all(&dir);
}

/// The helper of `e2e_kill_parent_kills_vm`.
#[test]
fn kill_parent_helper() {
    let Ok(pid_file) = std::env::var("EWE_TEST_E2E_PID_FILE") else {
        return;
    };
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = ci_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let sandbox = create(&manager, &rootfs);
    let firecracker_pid = std::fs::read_to_string(
        host.config
            .state_dir
            .join(sandbox.id().as_str())
            .join("pid"),
    )
    .expect("the pid file");
    std::fs::write(&pid_file, firecracker_pid.trim()).expect("the pid file");
    loop {
        std::thread::park();
    }
}

fn wait_for_pid(ctx: &Ctx, path: &std::path::Path) -> u32 {
    let deadline = ctx.clock().now() + Duration::from_secs(600);
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(pid) = text.trim().parse() {
                return pid;
            }
        }
        assert!(ctx.clock().now() < deadline, "the helper wrote no pid");
        // A real substrate test: the helper downloads the image first, so poll
        // with a short wait instead of a busy loop.
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn process_exists(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 checks for the process and sends nothing.
    unsafe { libc::kill(pid, 0) == 0 }
}
