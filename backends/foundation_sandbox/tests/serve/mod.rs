//! The `SandboxService` server and client on a real socket (feature 06).
//!
//! These tests use the simulation driver and the scripted slave of feature 02,
//! so they need no microVM. They prove invariants I1, I2, I3 and I4 and the two
//! fault rows of a Unix domain socket.
#![allow(
    clippy::assert_is_empty,
    reason = "the test reads clearer as an empty check on the listed sandboxes"
)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use foundation_context::Clock;
use foundation_context::Ctx;
use foundation_core::env;
use foundation_core::valtron::SimulatedClock;
use foundation_http::ShutdownSignal;

use foundation_sandbox::native::connect::ConnectAddr;
use foundation_sandbox::native::serve::{bind_control, serve_bound, ServeConfig, ServeError};
use foundation_sandbox::shared::command::Command;
use foundation_sandbox::shared::control::testing::{
    in_memory_connector, spec, ScriptedSlave, SimDriver,
};
use foundation_sandbox::shared::control::{
    ManagerConfig, MemoryRecordStore, SandboxManager, SandboxProvider, SandboxRpcClient,
};
use foundation_sandbox::shared::driver::SandboxDriver;
use foundation_sandbox::shared::error::SandboxError;
use foundation_sandbox::shared::ids::{DriverId, SandboxId};
use foundation_sandbox::shared::limits::SandboxLimits;
use foundation_sandbox::shared::listen::ListenAddress;
use foundation_sandbox::shared::rpc::messages::ExecEventKind;
use foundation_sandbox::shared::Secret;

/// A socket file that this test removes on drop.
struct SocketPath(PathBuf);

impl SocketPath {
    /// A short path below `/tmp`, because the kernel caps the path length.
    fn new(tag: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self(PathBuf::from(format!(
            "/tmp/ewe-f06-{tag}-{}-{id}.sock",
            std::process::id()
        )))
    }
}

impl Drop for SocketPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// A context with a simulated clock and environment.
fn ctx(seed: u64) -> Ctx {
    crate::pool::pool();
    let clock = Arc::new(SimulatedClock::new(std::time::Duration::ZERO));
    let ctx = Ctx::with_seed(Arc::clone(&clock) as Arc<dyn Clock>, seed);
    let env = env::simulated(&ctx, seed).expect("the simulated Env builds");
    ctx.store(env);
    ctx
}

/// A manager over one simulation driver and a scripted slave.
fn manager(ctx: &Ctx, driver: SimDriver, slave: Arc<ScriptedSlave>) -> Arc<SandboxManager> {
    Arc::new(SandboxManager::new(
        ctx.clone(),
        vec![Arc::new(driver) as Arc<dyn SandboxDriver>],
        in_memory_connector(slave),
        Arc::new(MemoryRecordStore::default()),
        ManagerConfig::default(),
    ))
}

/// Starts the server on `bound` and returns its thread.
fn serve(
    ctx: Ctx,
    control: Arc<dyn SandboxProvider>,
    bound: foundation_sandbox::native::serve::Bound,
    auth: Option<Secret>,
) -> std::thread::JoinHandle<()> {
    let limits = SandboxLimits::default();
    std::thread::spawn(move || serve_bound(ctx, control, &bound, auth, &limits))
}

#[test]
fn rpc_parity_uds() {
    let _serial = crate::control::support::serial();
    let ctx = ctx(80);
    let slave = Arc::new(ScriptedSlave::new());
    let driver = SimDriver::new(DriverId::Firecracker);
    // A stop must not wait the whole shutdown grace: the driver's substrate
    // exits at once when the slave receives `Shutdown`.
    slave.with_shutdown_exit(driver.substrate());
    slave.with_exec_events(vec![
        ExecEventKind::Started { pid: 7 },
        ExecEventKind::Stdout {
            data: b"hi\n".to_vec(),
        },
        ExecEventKind::Exit {
            code: Some(0),
            signal: None,
        },
    ]);
    let manager = manager(&ctx, driver, Arc::clone(&slave));
    let socket = SocketPath::new("parity");
    let shutdown = Arc::new(ShutdownSignal::new());
    let listen = ListenAddress::Unix {
        path: socket.0.clone(),
    };
    let bound = bind_control(&listen, Arc::clone(&shutdown), &ServeConfig::default())
        .expect("the Unix listener binds");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&socket.0)
            .expect("the socket file")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "invariant I4: the socket file has mode 0600");
    }
    let control: Arc<dyn SandboxProvider> = Arc::clone(&manager) as Arc<dyn SandboxProvider>;
    let server = serve(ctx.clone(), control, bound, None);

    let limits = SandboxLimits::default();
    let client = SandboxRpcClient::connect(
        ctx.clone(),
        ConnectAddr::Unix(socket.0.clone()),
        limits.clone(),
    )
    .expect("the client connects");
    let sandbox = crate::pool::block_on(client.create(spec())).expect("the remote create");
    let output =
        crate::pool::block_on(sandbox.run(Command::new(["echo", "hi"]))).expect("the remote run");
    assert_eq!(output.stdout, b"hi\n");
    assert_eq!(output.status.exit_code(), Some(0));
    let sandboxes = crate::pool::block_on(client.list()).expect("the remote list");
    assert_eq!(sandboxes.len(), 1);
    crate::pool::block_on(sandbox.stop()).expect("the remote stop");

    shutdown.turn_on();
    server.join().expect("the server thread ends");
    assert!(!socket.0.exists(), "the server removes its socket file");
}

#[test]
fn rpc_error_variants() {
    let _serial = crate::control::support::serial();
    let ctx = ctx(81);
    let slave = Arc::new(ScriptedSlave::new());
    let manager = manager(&ctx, SimDriver::new(DriverId::Firecracker), slave);
    let socket = SocketPath::new("error");
    let shutdown = Arc::new(ShutdownSignal::new());
    let bound = bind_control(
        &ListenAddress::Unix {
            path: socket.0.clone(),
        },
        Arc::clone(&shutdown),
        &ServeConfig::default(),
    )
    .expect("the Unix listener binds");
    let control: Arc<dyn SandboxProvider> = Arc::clone(&manager) as Arc<dyn SandboxProvider>;
    let server = serve(ctx.clone(), control, bound, None);

    let client = SandboxRpcClient::connect(
        ctx.clone(),
        ConnectAddr::Unix(socket.0.clone()),
        SandboxLimits::default(),
    )
    .expect("the client connects");
    let missing: SandboxId = "aaaaaaaaaaaa".parse().expect("id");
    let Err(error) = crate::pool::block_on(client.get(missing.clone())) else {
        panic!("get must fail");
    };
    assert!(matches!(
        error.current_context(),
        SandboxError::NotFound { .. }
    ));
    let Err(error) = crate::pool::block_on(client.stop(missing)) else {
        panic!("stop must fail");
    };
    assert!(matches!(
        error.current_context(),
        SandboxError::NotFound { .. }
    ));

    shutdown.turn_on();
    server.join().expect("the server thread ends");
}

#[test]
fn tcp_needs_token() {
    let _serial = crate::control::support::serial();
    let ctx = ctx(82);
    let slave = Arc::new(ScriptedSlave::new());
    let manager = manager(&ctx, SimDriver::new(DriverId::Firecracker), slave);
    let shutdown = Arc::new(ShutdownSignal::new());
    let bound = bind_control(
        &ListenAddress::Tcp {
            addr: "127.0.0.1:0".parse().expect("the address"),
        },
        Arc::clone(&shutdown),
        &ServeConfig::default(),
    )
    .expect("the TCP listener binds");
    let port = bound.local_addr().expect("the TCP address").port();
    let control: Arc<dyn SandboxProvider> = Arc::clone(&manager) as Arc<dyn SandboxProvider>;
    let server = serve(
        ctx.clone(),
        control,
        bound,
        Some(Secret::new("correct-token")),
    );

    let client = SandboxRpcClient::connect(
        ctx.clone(),
        ConnectAddr::Tcp {
            host: "127.0.0.1".to_string(),
            port,
        },
        SandboxLimits::default(),
    )
    .expect("the client connects");
    let error = crate::pool::block_on(client.list()).expect_err("no token must fail");
    assert!(matches!(
        error.current_context(),
        SandboxError::Unauthorized
    ));
    let wrong = client.with_token(Secret::new("wrong-token"));
    let error = crate::pool::block_on(wrong.list()).expect_err("a wrong token must fail");
    assert!(matches!(
        error.current_context(),
        SandboxError::Unauthorized
    ));

    shutdown.turn_on();
    server.join().expect("the server thread ends");
}

#[test]
fn tcp_accepts_the_token() {
    let _serial = crate::control::support::serial();
    let ctx = ctx(83);
    let slave = Arc::new(ScriptedSlave::new());
    let manager = manager(&ctx, SimDriver::new(DriverId::Firecracker), slave);
    let shutdown = Arc::new(ShutdownSignal::new());
    let bound = bind_control(
        &ListenAddress::Tcp {
            addr: "127.0.0.1:0".parse().expect("the address"),
        },
        Arc::clone(&shutdown),
        &ServeConfig::default(),
    )
    .expect("the TCP listener binds");
    let port = bound.local_addr().expect("the TCP address").port();
    let control: Arc<dyn SandboxProvider> = Arc::clone(&manager) as Arc<dyn SandboxProvider>;
    let server = serve(
        ctx.clone(),
        control,
        bound,
        Some(Secret::new("correct-token")),
    );

    let client = SandboxRpcClient::connect(
        ctx.clone(),
        ConnectAddr::Tcp {
            host: "127.0.0.1".to_string(),
            port,
        },
        SandboxLimits::default(),
    )
    .expect("the client connects")
    .with_token(Secret::new("correct-token"));
    let sandboxes = crate::pool::block_on(client.list()).expect("a correct token passes");
    assert!(sandboxes.is_empty());

    shutdown.turn_on();
    server.join().expect("the server thread ends");
}

#[test]
fn unix_stale_socket_removed() {
    let socket = SocketPath::new("stale");
    let listener = std::os::unix::net::UnixListener::bind(&socket.0).expect("the stale listener");
    drop(listener);
    assert!(
        socket.0.exists(),
        "the stale socket file stays after the drop"
    );
    let shutdown = Arc::new(ShutdownSignal::new());
    let bound = bind_control(
        &ListenAddress::Unix {
            path: socket.0.clone(),
        },
        shutdown,
        &ServeConfig::default(),
    );
    assert!(bound.is_ok(), "a stale socket file is removed");
}

#[test]
fn unix_live_socket_refused() {
    let socket = SocketPath::new("live");
    let listener = std::os::unix::net::UnixListener::bind(&socket.0).expect("the live listener");
    let shutdown = Arc::new(ShutdownSignal::new());
    let bound = bind_control(
        &ListenAddress::Unix {
            path: socket.0.clone(),
        },
        shutdown,
        &ServeConfig::default(),
    );
    assert!(matches!(bound, Err(ServeError::AddressInUse)));
    assert!(socket.0.exists(), "a live socket file is not removed");
    drop(listener);
}

#[test]
fn unix_path_too_long() {
    let path = format!("/tmp/{}", "a".repeat(200));
    let shutdown = Arc::new(ShutdownSignal::new());
    let bound = bind_control(
        &ListenAddress::Unix {
            path: PathBuf::from(path),
        },
        shutdown,
        &ServeConfig::default(),
    );
    assert!(matches!(bound, Err(ServeError::SocketPathTooLong { .. })));
}
