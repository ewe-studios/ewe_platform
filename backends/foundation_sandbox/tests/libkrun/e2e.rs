//! The real libkrun tests (feature 09, section 7).
//!
//! Each test calls `probe()` first. A host without libkrun, the runner program
//! or the slave program prints the report and returns. The tests need Linux
//! with KVM, or macOS on Apple silicon with Hypervisor.framework (HVF).

#![cfg(all(
    any(target_os = "linux", target_os = "macos"),
    feature = "control",
    feature = "artifacts"
))]
#![allow(
    clippy::disallowed_methods,
    reason = "an end-to-end test watches real processes on real time at the test boundary"
)]

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use foundation_context::Ctx;
use foundation_core::env;
use foundation_sandbox::native::artifacts::{sha256_hex, ArtifactStore};
use foundation_sandbox::native::libkrun::driver::LibkrunDriver;
use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::control::{
    ManagerConfig, MemoryRecordStore, SandboxHandle, SandboxManager, SandboxProvider,
};
use foundation_sandbox::shared::driver::SandboxDriver;
use foundation_sandbox::shared::host::{HostConfig, SandboxHost};
use foundation_sandbox::shared::spec::{GuestArch, HostMount, NetworkMode, RootFs, SandboxSpec};

/// The environment name that makes a test process the lifeline helper.
const HELPER_ENV: &str = "EWE_KRUN_LIFELINE_CHILD";
/// The environment name of the file that the helper writes the runner path to.
const PIDFILE_ENV: &str = "EWE_KRUN_LIFELINE_PIDFILE";

/// A host that can run a libkrun machine, or `None` with the reason.
fn usable_host() -> Option<(Ctx, SandboxHost)> {
    crate::pool::pool();
    let ctx = Ctx::host();
    let effects = env::system(&ctx).ok()?;
    ctx.store(effects);
    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let config = HostConfig::discover(|name| std::env::var(name).ok(), &exe_dir);
    let host = SandboxHost::native(config);
    ctx.store(host.clone());
    let report = crate::pool::block_on(LibkrunDriver::new().probe(ctx.clone()));
    if !report.usable {
        eprintln!("skip: the host cannot run libkrun: {report:?}");
        return None;
    }
    Some((ctx, host))
}

/// The Alpine root directory, fetched before the create deadline starts.
fn alpine_rootfs(ctx: &Ctx, host: &SandboxHost) -> PathBuf {
    let store = ArtifactStore::new(ctx.clone(), host.fs.clone(), host.config.cache_dir.clone());
    crate::pool::block_on(store.alpine_rootfs_dir(GuestArch::host()))
        .expect("the Alpine root directory")
}

/// A manager with this driver.
fn manager(ctx: &Ctx, host: &SandboxHost) -> Arc<SandboxManager> {
    Arc::new(SandboxManager::new(
        ctx.clone(),
        vec![Arc::new(LibkrunDriver::new())],
        host.connector.clone(),
        Arc::new(MemoryRecordStore::default()),
        ManagerConfig::default(),
    ))
}

/// Creates one sandbox with the Alpine root directory.
fn create(manager: &SandboxManager, rootfs: &Path) -> SandboxHandle {
    crate::pool::block_on(manager.create(SandboxSpec::new(RootFs::Directory(rootfs.to_path_buf()))))
        .expect("the sandbox starts")
}

#[test]
fn e2e_command() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = alpine_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let sandbox = create(&manager, &rootfs);
    let output = crate::pool::block_on(sandbox.run(Command::new(["echo", "hello"])))
        .expect("the command runs");
    assert_eq!(output.stdout, b"hello\n");
    assert_eq!(output.status.exit_code(), Some(0));
    let uname = crate::pool::block_on(sandbox.run(Command::new(["uname", "-s"])))
        .expect("the command runs");
    assert_eq!(String::from_utf8_lossy(&uname.stdout).trim(), "Linux");
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_rootfs_not_changed() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = alpine_rootfs(&ctx, &host);
    let before = file_hashes(&rootfs);
    let manager = manager(&ctx, &host);
    let sandbox = create(&manager, &rootfs);
    let _ = crate::pool::block_on(sandbox.run(Command::new([
        "sh",
        "-c",
        "touch /marker; rm -f /etc/hostname",
    ])));
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
    let after = file_hashes(&rootfs);
    assert_eq!(before, after, "the caller's root directory never changes");
}

#[test]
fn e2e_host_env_not_leaked() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = alpine_rootfs(&ctx, &host);
    std::env::set_var("EWE_SECRET_TEST", "1");
    let manager = manager(&ctx, &host);
    let sandbox = create(&manager, &rootfs);
    let output =
        crate::pool::block_on(sandbox.run(Command::new(["env"]))).expect("the command runs");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        !text.contains("EWE_SECRET_TEST"),
        "the guest environment is empty: {text}"
    );
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_no_network() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = alpine_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let sandbox = create(&manager, &rootfs);
    let output =
        crate::pool::block_on(sandbox.run(Command::new(["sh", "-c", "ls /sys/class/net"])))
            .expect("the command runs");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "lo",
        "the only interface is lo"
    );
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_host_network() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = alpine_rootfs(&ctx, &host);
    let (port, server) = start_http_server();
    let manager = manager(&ctx, &host);
    let mut spec = SandboxSpec::new(RootFs::Directory(rootfs));
    spec.network = NetworkMode::Host;
    let sandbox = crate::pool::block_on(manager.create(spec)).expect("the sandbox starts");
    let output = crate::pool::block_on(sandbox.run(Command::new([
        "sh",
        "-c",
        &format!("timeout 10 wget -qO- http://127.0.0.1:{port}/"),
    ])))
    .expect("the command runs");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("krunok"),
        "the guest reaches the host network"
    );
    let _ = server.join();
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_host_mount() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = alpine_rootfs(&ctx, &host);
    let share = temp_dir("mount");
    std::fs::write(share.join("f"), b"mount-data").expect("write the shared file");
    let manager = manager(&ctx, &host);
    let mut spec = SandboxSpec::new(RootFs::Directory(rootfs));
    spec.mounts = vec![HostMount {
        host_path: share.clone(),
        guest_path: "/data".to_string(),
        read_only: true,
    }];
    let sandbox = crate::pool::block_on(manager.create(spec)).expect("the sandbox starts");
    let output = crate::pool::block_on(sandbox.run(Command::new([
        "sh",
        "-c",
        "cat /data/f && touch /data/g",
    ])))
    .expect("the command runs");
    assert_eq!(output.stdout, b"mount-data");
    assert_ne!(
        output.status.exit_code(),
        Some(0),
        "the read-only share refuses a write"
    );
    assert!(!share.join("g").exists(), "the host directory has no g");
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
    let _ = std::fs::remove_dir_all(&share);
}

#[test]
fn e2e_boot_time() {
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = alpine_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let started = Instant::now();
    let sandbox = create(&manager, &rootfs);
    let elapsed = started.elapsed();
    eprintln!("libkrun boot time: {elapsed:?}");
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

#[test]
fn e2e_kill_parent_kills_vm() {
    let Some(_) = usable_host() else {
        return;
    };
    let state_dir = temp_dir("lifeline");
    let pidfile = state_dir.join("runner-path");
    let mut helper =
        std::process::Command::new(std::env::current_exe().expect("the test binary path"))
            .args(["--exact", "e2e_lifeline_helper", "--nocapture"])
            .env(HELPER_ENV, "1")
            .env(PIDFILE_ENV, &pidfile)
            .env("EWE_SANDBOX_STATE_DIR", state_dir.join("state"))
            .spawn()
            .expect("the helper process starts");
    let krun_json =
        wait_for_file(&pidfile, Duration::from_secs(120)).expect("the helper creates a sandbox");
    let marker = PathBuf::from(krun_json);
    let pids = runner_pids(&marker);
    assert!(!pids.is_empty(), "the runner process exists");
    helper.kill().expect("the helper gets SIGKILL");
    let _ = helper.wait();
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if runner_pids(&marker).is_empty() {
            let _ = std::fs::remove_dir_all(&state_dir);
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("the runner process died with its parent in less than 2 s");
}

/// The helper process of [`e2e_kill_parent_kills_vm`].
///
/// The test suite also runs this test directly. It returns when the parent did
/// not set [`HELPER_ENV`].
#[test]
fn e2e_lifeline_helper() {
    if std::env::var(HELPER_ENV).is_err() {
        return;
    }
    let Some((ctx, host)) = usable_host() else {
        return;
    };
    let rootfs = alpine_rootfs(&ctx, &host);
    let manager = manager(&ctx, &host);
    let sandbox = create(&manager, &rootfs);
    let krun_json = host
        .config
        .state_dir
        .join(sandbox.id().as_str())
        .join("krun.json");
    let pidfile = std::env::var(PIDFILE_ENV).expect("the pidfile path");
    std::fs::write(&pidfile, krun_json.display().to_string()).expect("write the pidfile");
    std::thread::sleep(Duration::from_secs(30));
    crate::pool::block_on(sandbox.stop()).expect("the sandbox stops");
}

/// A unique temporary directory for one test.
fn temp_dir(tag: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("ewe-krun-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("the temporary directory");
    path
}

/// Every file under `root`, with its SHA-256, in path order.
fn file_hashes(root: &Path) -> Vec<(PathBuf, String)> {
    fn walk(root: &Path, directory: &Path, found: &mut Vec<(PathBuf, String)>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(root, &path, found);
            } else if let Ok(bytes) = std::fs::read(&path) {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                found.push((relative, sha256_hex(&bytes)));
            }
        }
    }
    let mut found = Vec::new();
    walk(root, root, &mut found);
    found.sort();
    found
}

/// A server on the host loopback that answers one request.
fn start_http_server() -> (u16, std::thread::JoinHandle<()>) {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("the test server binds a loopback port");
    let port = listener
        .local_addr()
        .expect("the test server address")
        .port();
    let handle = std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 6\r\nConnection: close\r\n\r\nkrunok",
            );
        }
    });
    (port, handle)
}

/// Waits for `path` to exist and returns its text.
fn wait_for_file(path: &Path, timeout: Duration) -> Option<String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(text) = std::fs::read_to_string(path) {
            return Some(text);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// The processes whose command line names `marker`.
#[cfg(target_os = "linux")]
fn runner_pids(marker: &Path) -> Vec<u32> {
    let marker = marker.to_string_lossy().into_owned();
    let mut pids = Vec::new();
    let Ok(entries) = std::fs::read_dir("/proc") else {
        return pids;
    };
    for entry in entries.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(bytes) = std::fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let command = String::from_utf8_lossy(&bytes).replace('\0', " ");
        if command.contains(&marker) {
            pids.push(pid);
        }
    }
    pids
}

/// The processes whose command line names `marker`.
#[cfg(target_os = "macos")]
fn runner_pids(marker: &Path) -> Vec<u32> {
    let marker = marker.to_string_lossy().into_owned();
    let Ok(output) = std::process::Command::new("/bin/ps")
        .args(["-axo", "pid=,command="])
        .output()
    else {
        return Vec::new();
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains(&marker))
        .filter_map(|line| line.split_whitespace().next()?.parse::<u32>().ok())
        .collect()
}
