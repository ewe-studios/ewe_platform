//! The native side-effect tests (feature 05).
//!
//! The process spawner and the connector use the real operating system, with no
//! virtual machine. The tests run on Linux and macOS.

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::sync::Arc;
use std::time::Duration;

use foundation_context::Ctx;
use foundation_core::env;
use foundation_sandbox::native::host::{NativeConnector, NativeProcessSpawner};
use foundation_sandbox::shared::control::SlaveClient;
use foundation_sandbox::shared::driver::SlaveEndpoint;
use foundation_sandbox::shared::host::{HostConfig, ProcessSpawner, ProcessSpec, SlaveConnector};
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::rpc::messages::SlaveInfo;

/// A native context with the real timer.
fn native_ctx() -> Ctx {
    crate::pool::pool();
    let ctx = Ctx::host();
    let env = env::system(&ctx).expect("the system environment builds");
    ctx.store(env);
    ctx
}

/// A unique temporary directory.
///
/// The socket tests use `/tmp` when it exists: a Unix socket address is short,
/// and the default macOS temporary directory is longer than that limit.
fn temp_dir(tag: &str) -> PathBuf {
    let base = if Path::new("/tmp").is_dir() {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    };
    let path = base.join(format!("ewe-nh-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("the temporary directory");
    path
}

/// A process spec whose output goes to `dir`.
fn spec(program: &str, args: &[&str], dir: &Path) -> ProcessSpec {
    ProcessSpec {
        program: PathBuf::from(program),
        args: args.iter().map(ToString::to_string).collect(),
        env: vec![("EWE_TEST".to_string(), "1".to_string())],
        cwd: dir.to_path_buf(),
        stdout: dir.join("out.log"),
        stderr: dir.join("err.log"),
        kill_with_parent: true,
        extra_fds: Vec::new(),
    }
}

#[test]
fn spawn_env_is_empty() {
    let dir = temp_dir("env");
    let spawner = NativeProcessSpawner::new();
    let child = spawner
        .spawn(spec("/usr/bin/env", &[], &dir))
        .expect("the child starts");
    let exit = crate::pool::block_on(child.wait()).expect("the child exits");
    assert_eq!(exit.code, Some(0));
    let output = std::fs::read_to_string(dir.join("out.log")).expect("the output");
    assert!(
        output.contains("EWE_TEST=1"),
        "the given environment is there"
    );
    assert!(!output.contains("HOME="), "the host environment is cleared");
    assert!(!output.contains("PATH="), "the host environment is cleared");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn spawn_wait_and_signal() {
    let dir = temp_dir("signal");
    let spawner = NativeProcessSpawner::new();
    let child = spawner
        .spawn(spec("/bin/sh", &["-c", "exit 7"], &dir))
        .expect("the child starts");
    let exit = crate::pool::block_on(child.wait()).expect("the child exits");
    assert_eq!(exit.code, Some(7));
    assert_eq!(exit.signal, None);

    let child = spawner
        .spawn(spec("/bin/sleep", &["60"], &dir))
        .expect("the child starts");
    child.signal(libc::SIGKILL).expect("the signal");
    let exit = crate::pool::block_on(child.wait()).expect("the child exits");
    assert_eq!(exit.signal, Some(libc::SIGKILL));
    // A signal after the recorded exit is a no-op.
    child.signal(libc::SIGKILL).expect("the signal is ignored");
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(target_os = "linux")]
#[test]
fn spawn_kill_with_parent() {
    let dir = temp_dir("parent");
    let pid_file = dir.join("sleep.pid");
    let mut helper = std::process::Command::new(std::env::current_exe().expect("the test binary"))
        .args([
            "--exact",
            "native_host::helper_entry",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("EWE_TEST_PID_FILE", &pid_file)
        .spawn()
        .expect("the helper starts");
    let sleep_pid = wait_for_pid(&pid_file);
    helper.kill().expect("the helper is killed");
    let _ = helper.wait();

    let ctx = Ctx::host();
    let deadline = ctx.clock().now() + Duration::from_secs(2);
    while process_exists(sleep_pid) {
        assert!(
            ctx.clock().now() < deadline,
            "the grandchild is still alive after the parent died"
        );
        std::thread::yield_now();
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The helper of `spawn_kill_with_parent`.
///
/// It starts a `sleep` with the parent-death signal and writes its pid, then
/// parks. The test kills this process.
#[cfg(target_os = "linux")]
#[test]
fn helper_entry() {
    let Ok(pid_file) = std::env::var("EWE_TEST_PID_FILE") else {
        return;
    };
    let dir = PathBuf::from(&pid_file)
        .parent()
        .expect("the pid file has a directory")
        .to_path_buf();
    let spawner = NativeProcessSpawner::new();
    let child = spawner
        .spawn(ProcessSpec {
            program: PathBuf::from("/bin/sleep"),
            args: vec!["60".to_string()],
            env: Vec::new(),
            cwd: dir.clone(),
            stdout: dir.join("sleep.out"),
            stderr: dir.join("sleep.err"),
            kill_with_parent: true,
            extra_fds: Vec::new(),
        })
        .expect("the sleep starts");
    std::fs::write(&pid_file, child.pid().to_string()).expect("the pid file");
    loop {
        std::thread::park();
    }
}

#[cfg(target_os = "linux")]
#[test]
fn spawn_survives_pool_thread_exit() {
    let dir = temp_dir("survives");
    let spawner = Arc::new(NativeProcessSpawner::new());
    let shared = Arc::clone(&spawner);
    let child_dir = dir.clone();
    let pid = std::thread::spawn(move || {
        let child = shared
            .spawn(spec("/bin/sleep", &["60"], &child_dir))
            .expect("the child starts");
        child.pid()
    })
    .join()
    .expect("the short thread ends");
    assert!(process_exists(pid), "the child outlives the calling thread");
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        // SAFETY: `pid` names the child that this process started.
        let _ = unsafe { libc::kill(pid, libc::SIGKILL) };
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The last process identifier that a helper wrote.
#[cfg(target_os = "linux")]
fn wait_for_pid(path: &Path) -> u32 {
    let ctx = Ctx::host();
    let deadline = ctx.clock().now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(pid) = text.trim().parse() {
                return pid;
            }
        }
        assert!(ctx.clock().now() < deadline, "the helper wrote no pid");
        std::thread::yield_now();
    }
}

/// Whether a process with `pid` exists.
#[cfg(target_os = "linux")]
fn process_exists(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    // SAFETY: signal 0 checks for the process and sends nothing.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// A Unix-socket HTTP server that reads a request and answers one response.
fn serve_http(path: &Path, response: Vec<u8>) -> std::thread::JoinHandle<Vec<u8>> {
    let listener = UnixListener::bind(path).expect("bind the socket");
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return Vec::new();
        };
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    request.extend_from_slice(&buffer[..count]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
            }
        }
        let _ = stream.write_all(&response);
        let _ = stream.flush();
        request
    })
}

/// The HTTP response that carries one Connect JSON message.
fn connect_response(body: &[u8]) -> Vec<u8> {
    let mut response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
         Connection: close\r\n\r\n",
        body.len()
    )
    .into_bytes();
    response.extend_from_slice(body);
    response
}

fn info_body() -> Vec<u8> {
    serde_json::to_vec(&SlaveInfo {
        slave_version: "test".to_string(),
        arch: "x86_64".to_string(),
        kernel: "test".to_string(),
        is_pid1: false,
        prepared: false,
        can_control: false,
    })
    .expect("the info message serializes")
}

#[test]
fn connector_unix() {
    let dir = temp_dir("unix");
    let socket = dir.join("slave.sock");
    let server = serve_http(&socket, connect_response(&info_body()));
    let ctx = native_ctx();
    let connector = NativeConnector;
    let transport = connector
        .connect(
            &ctx,
            &SlaveEndpoint::Unix {
                path: socket.clone(),
            },
            &SandboxLimits::default(),
        )
        .expect("the transport builds");
    let client =
        SlaveClient::new(ctx, transport, SandboxLimits::default()).expect("the client builds");
    let info = crate::pool::block_on(client.info(Duration::from_secs(5))).expect("the info call");
    assert_eq!(info.slave_version, "test");
    let request = server.join().expect("the server ends");
    let request = String::from_utf8_lossy(&request).to_lowercase();
    assert!(request.contains("info"), "the server saw the Info call");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A server for the vsock preamble tests.
fn serve_preamble(
    path: &Path,
    reply: &'static [u8],
    then_http: bool,
) -> std::thread::JoinHandle<Vec<u8>> {
    let listener = UnixListener::bind(path).expect("bind the socket");
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return Vec::new();
        };
        let mut preamble = Vec::new();
        let mut byte = [0_u8; 1];
        while let Ok(1) = stream.read(&mut byte) {
            preamble.push(byte[0]);
            if byte[0] == b'\n' {
                break;
            }
        }
        let _ = stream.write_all(reply);
        let _ = stream.flush();
        if !then_http {
            return preamble;
        }
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        let _ = stream.read(&mut buffer);
        request.extend_from_slice(&buffer);
        let response = connect_response(&info_body());
        let _ = stream.write_all(&response);
        let _ = stream.flush();
        preamble
    })
}

fn info_call(
    ctx: Ctx,
    socket: &Path,
) -> foundation_sandbox::shared::error::SandboxResult<SlaveInfo> {
    let connector = NativeConnector;
    let transport = connector.connect(
        &ctx,
        &SlaveEndpoint::FirecrackerVsock {
            uds: socket.to_path_buf(),
            port: 1024,
        },
        &SandboxLimits::default(),
    )?;
    let client = SlaveClient::new(ctx, transport, SandboxLimits::default())?;
    crate::pool::block_on(client.info(Duration::from_secs(5)))
}

#[test]
fn connector_preamble_ok() {
    let dir = temp_dir("preamble-ok");
    let socket = dir.join("v.sock");
    let server = serve_preamble(&socket, b"OK 1073741824\n", true);
    let ctx = native_ctx();
    let info = info_call(ctx, &socket).expect("the info call");
    assert_eq!(info.slave_version, "test");
    let preamble = server.join().expect("the server ends");
    assert_eq!(preamble, b"CONNECT 1024\n".to_vec());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn connector_preamble_closed() {
    let dir = temp_dir("preamble-closed");
    let socket = dir.join("v.sock");
    let server = serve_preamble(&socket, b"", false);
    let ctx = native_ctx();
    let result = info_call(ctx, &socket);
    assert!(result.is_err(), "a closed connection fails the call");
    let _ = server.join();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn connector_preamble_bad_reply() {
    let dir = temp_dir("preamble-bad");
    let socket = dir.join("v.sock");
    let server = serve_preamble(&socket, b"NO\n", false);
    let ctx = native_ctx();
    let result = info_call(ctx, &socket);
    assert!(result.is_err(), "a wrong reply fails the call");
    let _ = server.join();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn discover_order() {
    let dir = temp_dir("discover");
    let firecracker = dir.join("firecracker");
    std::fs::write(&firecracker, b"binary").expect("the firecracker file");
    let environment = |name: &str| -> Option<String> {
        match name {
            "EWE_SANDBOX_STATE_DIR" => Some("/state/env".to_string()),
            "XDG_RUNTIME_DIR" => Some("/run/user".to_string()),
            "EWE_SANDBOX_CACHE_DIR" => Some("/cache/env".to_string()),
            "XDG_CACHE_HOME" => Some("/cache/xdg".to_string()),
            "HOME" => Some("/home/tester".to_string()),
            "EWE_SANDBOX_SLAVE" => Some("/slave/env".to_string()),
            "EWE_FIRECRACKER" => Some(firecracker.display().to_string()),
            "EWE_SANDBOX_KRUN" => Some("/krun/env".to_string()),
            "EWE_LIBKRUN" => Some("/libkrun/env".to_string()),
            "EWE_SANDBOX_DRIVERS" => Some("docker, libkrun".to_string()),
            "UID" => Some("1000".to_string()),
            _ => None,
        }
    };
    let config = HostConfig::discover(environment, Path::new("/exe"));
    assert_eq!(config.state_dir, PathBuf::from("/state/env"));
    assert_eq!(config.cache_dir, PathBuf::from("/cache/env"));
    assert_eq!(config.slave_path, Some(PathBuf::from("/slave/env")));
    assert_eq!(config.firecracker_path, Some(firecracker.clone()));
    assert_eq!(config.krun_runner_path, Some(PathBuf::from("/krun/env")));
    assert_eq!(config.libkrun_path, Some(PathBuf::from("/libkrun/env")));
    assert_eq!(
        config.driver_order,
        vec![
            foundation_sandbox::shared::ids::DriverId::Docker,
            foundation_sandbox::shared::ids::DriverId::Libkrun
        ]
    );

    // The second source of each field when the first is absent.
    let fallback = |name: &str| -> Option<String> {
        match name {
            "XDG_RUNTIME_DIR" => Some("/run/user".to_string()),
            "XDG_CACHE_HOME" => Some("/cache/xdg".to_string()),
            "HOME" => Some("/home/tester".to_string()),
            "UID" => Some("1000".to_string()),
            _ => None,
        }
    };
    let config = HostConfig::discover(fallback, Path::new("/exe"));
    assert_eq!(config.state_dir, PathBuf::from("/run/user/ewe-sbx"));
    assert_eq!(config.cache_dir, PathBuf::from("/cache/xdg/ewe-sandbox"));
    assert_eq!(
        config.slave_path,
        Some(PathBuf::from(format!(
            "/exe/agent-slave-{}",
            foundation_sandbox::shared::spec::GuestArch::host().as_str()
        )))
    );
    assert_eq!(config.firecracker_path, None);
    assert_eq!(
        config.krun_runner_path,
        Some(PathBuf::from("/exe/sandbox-krun"))
    );
    assert_eq!(config.libkrun_path, None);
    assert_eq!(
        config.driver_order,
        vec![
            foundation_sandbox::shared::ids::DriverId::Firecracker,
            foundation_sandbox::shared::ids::DriverId::Libkrun,
            foundation_sandbox::shared::ids::DriverId::Docker
        ]
    );

    // The last source of the state and cache directories.
    let bare = |name: &str| -> Option<String> {
        match name {
            "HOME" => Some("/home/tester".to_string()),
            "UID" => Some("1000".to_string()),
            _ => None,
        }
    };
    let config = HostConfig::discover(bare, Path::new("/exe"));
    assert_eq!(config.state_dir, PathBuf::from("/tmp/ewe-sbx-1000"));
    assert_eq!(
        config.cache_dir,
        PathBuf::from("/home/tester/.cache/ewe-sandbox")
    );
    let _ = std::fs::remove_dir_all(&dir);
}

mod record;
