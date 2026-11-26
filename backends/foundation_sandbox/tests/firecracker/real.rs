//! The test against the real `firecracker` program.
//!
//! It needs the program and, on Linux, `/dev/kvm`. When either is missing it
//! prints why and returns: a machine without a virtual machine monitor cannot
//! run this test.

use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use foundation_context::{Ctx, MonotonicClock};
use foundation_core::env;
use foundation_sandbox::native::firecracker::client::FirecrackerClient;
use foundation_sandbox::native::firecracker::generated::machine_config::MachineConfiguration;
use foundation_sandbox::shared::limits::SandboxLimits;

use super::support;

/// A context with the real clock and the real effects.
fn real_ctx() -> Ctx {
    let ctx = Ctx::fixed(Arc::new(MonotonicClock::new()));
    env::install(&ctx, env::system).expect("the real environment builds");
    ctx
}

/// The `firecracker` program, when it is installed.
fn find_firecracker() -> Option<String> {
    let output = Command::new("firecracker").arg("--version").output().ok()?;
    output.status.success().then(|| "firecracker".to_string())
}

#[test]
fn types_match_real_firecracker() {
    let Some(program) = find_firecracker() else {
        eprintln!("skipped: the firecracker program is not installed");
        return;
    };
    let dir = support::socket_path("real")
        .parent()
        .expect("the socket has a directory")
        .to_path_buf();
    let socket = dir.join("api.sock");
    let mut child = match Command::new(&program)
        .arg("--api-sock")
        .arg(&socket)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            eprintln!("skipped: cannot start firecracker: {error}");
            return;
        }
    };

    // Wait up to five seconds for the API socket. A host without `/dev/kvm`
    // exits instead. A real wait is correct here: this test drives a real
    // program, not the injected clock.
    for _ in 0..250 {
        if socket.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    if !socket.exists() {
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
        eprintln!("skipped: firecracker made no API socket (is /dev/kvm present?)");
        return;
    }

    let ctx = real_ctx();
    let client = FirecrackerClient::new(&ctx, &socket, &SandboxLimits::default())
        .expect("the client builds");

    let version = support::block_on(client.version()).expect("version");
    assert_ne!(version.firecracker_version, "");
    let info = support::block_on(client.describe_instance()).expect("describe instance");
    assert_eq!(info.app_name, "Firecracker");

    let config = MachineConfiguration {
        vcpu_count: 1,
        mem_size_mib: 128,
        ..Default::default()
    };
    support::block_on(client.put_machine_config(&config)).expect("put machine config");

    let _ = child.kill();
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(&dir);
}
