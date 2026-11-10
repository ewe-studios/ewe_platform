//! An idle QUIC listener costs no CPU (spec 70 feature 75, E2).
//!
//! WHY: a proxy with an H3 listener and no client used about 85 ms of CPU per
//! second. A listener that waits for a packet must park, not poll.
//!
//! WHAT: start the proxy with `h3_bind`, connect nobody, hold still for one
//! second, and measure the CPU time of the whole process. The limit is 20 ms.

#![cfg(feature = "quic")]

use std::time::Duration;

use foundation_core::valtron::{initialize_pool, BlockingFutureExt};
use foundation_proxy::config::{ServiceConfig, SslConfig};
use foundation_proxy::{ProxyConfig, ProxyServer};

const CERT: &[u8] = include_bytes!("fixtures/quic_cert.pem");
const KEY: &[u8] = include_bytes!("fixtures/quic_key.pem");

/// The most CPU an idle listener may use in one second.
const CPU_LIMIT: Duration = Duration::from_millis(20);

/// The CPU time this process has used, all threads.
fn process_cpu() -> Duration {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: `ts` is a valid out pointer for the call.
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut ts) };
    assert_eq!(rc, 0, "clock_gettime");
    Duration::new(ts.tv_sec as u64, ts.tv_nsec as u32)
}

#[test]
fn idle_quic_listener_uses_no_cpu() {
    let _guard = initialize_pool(97, Some(8));
    let dir = std::env::temp_dir().join(format!("ewe_h3_idle_{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("temp dir");
    let (cert, key) = (dir.join("cert.pem"), dir.join("key.pem"));
    std::fs::write(&cert, CERT).expect("cert");
    std::fs::write(&key, KEY).expect("key");
    let config = ProxyConfig::new("localhost", "127.0.0.1")
        .bind("127.0.0.1:0")
        .ssl(SslConfig::static_cert(
            &cert.display().to_string(),
            &key.display().to_string(),
        ))
        .h3_bind("127.0.0.1:0")
        .service(ServiceConfig::new("idle", "localhost").backend("http://127.0.0.1:9"));
    let proxy = ProxyServer::start(config, foundation_context::Ctx::host())
        .block()
        .expect("start proxy");
    assert!(proxy.h3_local_addr().is_some(), "the H3 listener is bound");

    // Let start-up work finish, then hold still.
    std::thread::sleep(Duration::from_millis(500));
    let before = process_cpu();
    std::thread::sleep(Duration::from_secs(1));
    let used = process_cpu() - before;
    proxy.shutdown();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(
        used < CPU_LIMIT,
        "an idle QUIC listener used {used:?} of CPU in one second; the limit is {CPU_LIMIT:?}"
    );
}
