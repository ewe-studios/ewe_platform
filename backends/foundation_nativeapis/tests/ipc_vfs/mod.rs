#![cfg(all(feature = "vfs-ipc", feature = "vfs-native"))]

use std::sync::Arc;

use foundation_nativeapis::native::vfs::NativeFs;
use foundation_nativeapis::shared::vfs::exec_async::exec_async;
use foundation_nativeapis::shared::vfs::ipc_client::VfsTransport;
use foundation_nativeapis::shared::vfs::ipc_client::{DirectTransport, VfsClient};
use foundation_nativeapis::shared::vfs::ipc_daemon::VfsDaemon;
use foundation_nativeapis::shared::vfs::{OpenMode, SyncFs, VfsDirectory, VfsFile, VfsFileSystem};

/// Connects a client and wraps it in `SyncFs` for these sync tests.
fn connect(transport: Arc<dyn VfsTransport>) -> SyncFs<VfsClient> {
    SyncFs::new(exec_async(async move { Ok(VfsClient::new(transport).await) }).unwrap())
}
use std::fs;
use std::path::PathBuf;

struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "vfs_ipc_test_{name}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp dir");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn make_client(tmp: &TempDir) -> SyncFs<VfsClient> {
    let native_fs = NativeFs::new(tmp.path()).expect("create NativeFs");
    let daemon = VfsDaemon::new(Arc::new(native_fs));
    let transport = DirectTransport::new(daemon);
    connect(Arc::new(transport))
}

// ── Test 1: Basic file operations through IPC transport ──

#[test]
fn test_open_read_write_close() {
    let tmp = TempDir::new("read_write");
    fs::write(tmp.path().join("test.txt"), b"hello world").unwrap();

    let client = make_client(&tmp);

    // Read through IPC client
    let file = client
        .open("/test.txt", OpenMode::Read)
        .expect("open failed");
    let mut buf = vec![0u8; 64];
    let n = file.read_at(&mut buf, 0).expect("read failed");
    assert_eq!(&buf[..n], b"hello world");

    // Write through IPC client
    let wfile = client
        .open("/test.txt", OpenMode::Write)
        .expect("open write failed");
    let n = wfile.write_at(b"new content", 0).expect("write failed");
    assert_eq!(n, 11);

    // Read back
    drop(wfile);
    let file2 = client
        .open("/test.txt", OpenMode::Read)
        .expect("open after write failed");
    let mut buf2 = vec![0u8; 64];
    let n2 = file2
        .read_at(&mut buf2, 0)
        .expect("read after write failed");
    assert_eq!(&buf2[..n2], b"new content");
}

// ── Test 2: Stat through client matches direct stat ──

#[test]
fn test_stat_through_client_matches_direct() {
    let tmp = TempDir::new("stat");
    fs::write(tmp.path().join("data.txt"), b"1234567890").unwrap();

    let client = make_client(&tmp);

    // Stat through IPC client
    let client_meta = client.stat("/data.txt").expect("stat failed");

    // Direct stat on NativeFs
    let native_fs = SyncFs::new(NativeFs::new(tmp.path()).expect("create NativeFs"));
    let direct_meta = native_fs.stat("/data.txt").expect("direct stat failed");

    assert_eq!(client_meta.size, direct_meta.size);
    assert_eq!(client_meta.size, 10);
    assert_eq!(client_meta.file_type, direct_meta.file_type);
}

// ── Test 3: Multiple clients sharing one daemon ──

#[test]
fn test_multiple_clients_shared_daemon() {
    let tmp = TempDir::new("multi_client");

    // Create two independent daemon+client pairs sharing the same directory
    let native_fs_a = NativeFs::new(tmp.path()).expect("create NativeFs A");
    let daemon_a = VfsDaemon::new(Arc::new(native_fs_a));
    let transport_a = DirectTransport::new(daemon_a);
    let client_a = connect(Arc::new(transport_a));

    let native_fs_b = NativeFs::new(tmp.path()).expect("create NativeFs B");
    let daemon_b = VfsDaemon::new(Arc::new(native_fs_b));
    let transport_b = DirectTransport::new(daemon_b);
    let client_b = connect(Arc::new(transport_b));

    // Client A creates a file
    client_a
        .write_file("/shared.txt", b"from client A")
        .unwrap();

    // Client B reads it
    let data = client_b.read_file("/shared.txt").unwrap();
    assert_eq!(data, b"from client A");

    // Client B modifies
    client_b
        .write_file("/shared.txt", b"from client B")
        .unwrap();

    // Client A reads the modification
    let data = client_a.read_file("/shared.txt").unwrap();
    assert_eq!(data, b"from client B");
}

// ── Test 4: Directory operations ──

#[test]
fn test_directory_operations() {
    let tmp = TempDir::new("dir_ops");
    let client = make_client(&tmp);

    // Create directory
    client.mkdir("/subdir").expect("mkdir failed");

    // Verify it exists
    assert!(client.exists("/subdir").expect("exists failed"));

    // Create a file in the directory
    client
        .write_file("/subdir/nested.txt", b"nested content")
        .unwrap();

    // Read it back
    let data = client.read_file("/subdir/nested.txt").unwrap();
    assert_eq!(data, b"nested content");

    // List directory
    let dir = client.open_directory("/subdir").expect("open dir failed");
    let entries = dir.list().expect("list failed");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "nested.txt");
}

// ── Test 5: Create and remove ──

#[test]
fn test_create_and_remove() {
    let tmp = TempDir::new("create_remove");
    let client = make_client(&tmp);

    // Create file
    let file = client
        .create("/new_file.txt", 0o644)
        .expect("create failed");
    drop(file);

    assert!(client.exists("/new_file.txt").unwrap());

    // Remove file
    client.remove("/new_file.txt").expect("remove failed");
    assert!(!client.exists("/new_file.txt").unwrap());
}

// ── Test 6: Rename ──

#[test]
fn test_rename() {
    let tmp = TempDir::new("rename");
    fs::write(tmp.path().join("old.txt"), b"rename me").unwrap();

    let client = make_client(&tmp);

    client
        .rename("/old.txt", "/new.txt")
        .expect("rename failed");
    assert!(!client.exists("/old.txt").unwrap());
    assert!(client.exists("/new.txt").unwrap());

    let data = client.read_file("/new.txt").unwrap();
    assert_eq!(data, b"rename me");
}

// ── Test 7: Symlink and readlink ──

#[test]
fn test_symlink_and_readlink() {
    let tmp = TempDir::new("symlink");
    fs::write(tmp.path().join("target.txt"), b"symlink target").unwrap();

    let client = make_client(&tmp);

    client
        .symlink("/target.txt", "/link.txt")
        .expect("symlink failed");
    let target = client.readlink("/link.txt").expect("readlink failed");
    assert_eq!(target, "/target.txt");
}

// ── Test 8: Capabilities ──

#[test]
fn test_capabilities() {
    let tmp = TempDir::new("capabilities");
    let client = make_client(&tmp);

    let caps = client.capabilities();
    assert!(caps.seekable);
}

// ── Test 9: Inode operations ──

#[test]
fn test_inode_operations() {
    let tmp = TempDir::new("inode");
    fs::write(tmp.path().join("inode_test.txt"), b"inode data").unwrap();

    let client = make_client(&tmp);

    // Get inode (works — just calls stat and extracts the inode number)
    let ino = client.inode("/inode_test.txt").expect("inode failed");
    assert_ne!(ino, 0);

    // stat_by_inode and path_by_inode require an inode-native VFS backend
    // (e.g., OverlayFileSystem with delta tracking). NativeFs cannot reverse-lookup
    // an inode to a path, so these operations return "not supported".
    // The IPC transport correctly forwards the error.
    let result = client.stat_by_inode(ino);
    assert!(result.is_err(), "NativeFs should not support stat_by_inode");
}

// ── Test 10: ReadFile / WriteFile convenience ──

#[test]
fn test_read_write_file_convenience() {
    let tmp = TempDir::new("convenience");
    let client = make_client(&tmp);

    let data = vec![0xAB; 1024];
    client.write_file("/big.bin", &data).unwrap();

    let read = client.read_file("/big.bin").unwrap();
    assert_eq!(read.len(), 1024);
    assert_eq!(read, data);
}

// ── E5/E6: real IPC bus transport tests ──

use foundation_core::valtron::valtron_test;
use foundation_nativeapis::ipc::{LabelOp, Selector};
use foundation_nativeapis::native::vfs::{
    run_daemon_loop, ClientTransport, DaemonTransport, VfsIpcConfig,
};
use foundation_nativeapis::shared::vfs::ipc_messages::{
    VfsRequestBody, VfsResponse, VfsResponseBody,
};
use foundation_nativeapis::shared::vfs::AsyncVfsFileSystem;
use futures_lite::future::{yield_now, zip};
use futures_lite::FutureExt;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

static NEXT_BUS_ID: AtomicU64 = AtomicU64::new(0);

/// A unique bus name per test, so parallel tests never share a bus.
fn unique_identifier(tag: &str) -> String {
    let n = NEXT_BUS_ID.fetch_add(1, Ordering::SeqCst);
    format!("foundation-vfs-test-{tag}-{}-{n}", std::process::id())
}

/// A short poll timeout stops the reader thread and daemon loop quickly.
fn test_config(tag: &str) -> VfsIpcConfig {
    VfsIpcConfig::new()
        .with_identifier(unique_identifier(tag))
        .with_join_timeout(Duration::from_secs(5))
        .with_poll_timeout(Duration::from_millis(20))
}

/// Runs every future at once. The futures form a balanced zip tree, so one
/// poll reaches all of them instead of finishing them pair by pair.
async fn zip_all<T: Send + 'static>(futs: Vec<futures_lite::future::Boxed<T>>) -> Vec<T> {
    if futs.is_empty() {
        return Vec::new();
    }
    let mut level: Vec<futures_lite::future::Boxed<Vec<T>>> = futs
        .into_iter()
        .map(|f| async move { vec![f.await] }.boxed())
        .collect();
    while level.len() > 1 {
        let mut next = Vec::with_capacity((level.len() + 1) / 2);
        let mut iter = level.into_iter();
        while let Some(a) = iter.next() {
            match iter.next() {
                Some(b) => next.push(
                    async move {
                        let (mut va, vb) = zip(a, b).await;
                        va.extend(vb);
                        va
                    }
                    .boxed(),
                ),
                None => next.push(a),
            }
        }
        level = next;
    }
    level.remove(0).await
}

/// The client's request parks on its id pipe; a ticker on the same valtron
/// worker keeps ticking while a slow daemon holds the response.
#[valtron_test]
async fn ipc_request_awaits_without_blocking() {
    let config = test_config("await");
    let mut server = DaemonTransport::serve_with_config(
        config.clone(),
        Arc::new(foundation_core::valtron::MonotonicClock::new()),
    )
    .unwrap();
    let handle = std::thread::spawn(move || {
        let msg = server
            .recv_request_timeout(Duration::from_secs(10))
            .unwrap()
            .expect("one request");
        std::thread::sleep(Duration::from_millis(200));
        server
            .send_response(
                VfsResponse {
                    id: msg.payload.id,
                    body: VfsResponseBody::Data(b"hello".to_vec()),
                },
                Selector::multicast(LabelOp::True),
            )
            .unwrap();
    });

    let transport = ClientTransport::connect_with_config(
        config,
        None,
        Arc::new(foundation_core::valtron::MonotonicClock::new()),
    )
    .unwrap();

    let ticks = Arc::new(AtomicUsize::new(0));
    let done = Arc::new(AtomicBool::new(false));
    let ticker = {
        let (ticks, done) = (Arc::clone(&ticks), Arc::clone(&done));
        async move {
            while !done.load(Ordering::SeqCst) {
                ticks.fetch_add(1, Ordering::SeqCst);
                yield_now().await;
            }
        }
    };
    let read = {
        let done = Arc::clone(&done);
        async move {
            let response = transport
                .request(VfsRequestBody::ReadFile { path: "/a".into() })
                .await;
            done.store(true, Ordering::SeqCst);
            response
        }
    };

    let (response, ()) = zip(read, ticker).await;
    match response {
        VfsResponseBody::Data(data) => assert_eq!(data, b"hello"),
        other => panic!("unexpected response: {other:?}"),
    }
    assert!(
        ticks.load(Ordering::SeqCst) > 10,
        "the ticker must tick while the IPC request awaits, saw {}",
        ticks.load(Ordering::SeqCst)
    );
    handle.join().unwrap();
}

/// The transport config carries both timeouts with documented defaults.
#[test]
fn ipc_timeouts_come_from_config() {
    let default = VfsIpcConfig::new();
    assert_eq!(default.join_timeout(), Duration::from_secs(30));
    assert_eq!(default.poll_timeout(), Duration::from_secs(1));

    let join = Duration::from_millis(150);
    let poll = Duration::from_millis(25);
    let configured = VfsIpcConfig::new()
        .with_identifier("ipc-timeout-test")
        .with_join_timeout(join)
        .with_poll_timeout(poll);
    assert_eq!(configured.identifier(), "ipc-timeout-test");
    assert_eq!(configured.join_timeout(), join);
    assert_eq!(configured.poll_timeout(), poll);
}

/// A daemon that answers in reverse order: the client must match by id.
#[valtron_test]
async fn ipc_out_of_order_responses_match_by_id() {
    let config = test_config("ooo");
    let mut server = DaemonTransport::serve_with_config(
        config.clone(),
        Arc::new(foundation_core::valtron::MonotonicClock::new()),
    )
    .unwrap();
    let handle = std::thread::spawn(move || {
        let first = server
            .recv_request_timeout(Duration::from_secs(10))
            .unwrap()
            .expect("first request")
            .payload;
        let second = server
            .recv_request_timeout(Duration::from_secs(10))
            .unwrap()
            .expect("second request")
            .payload;
        let selector = Selector::multicast(LabelOp::True);
        // Reverse order: answer the second request first.
        server
            .send_response(
                VfsResponse {
                    id: second.id,
                    body: VfsResponseBody::Data(vec![second.id as u8; 4]),
                },
                selector.clone(),
            )
            .unwrap();
        server
            .send_response(
                VfsResponse {
                    id: first.id,
                    body: VfsResponseBody::Data(vec![first.id as u8; 4]),
                },
                selector,
            )
            .unwrap();
    });

    let transport = ClientTransport::connect_with_config(
        config,
        None,
        Arc::new(foundation_core::valtron::MonotonicClock::new()),
    )
    .unwrap();
    let (a, b) = zip(
        transport.request(VfsRequestBody::ReadFile { path: "/a".into() }),
        transport.request(VfsRequestBody::ReadFile { path: "/b".into() }),
    )
    .await;

    // The ids are 1 and 2 in send order, so the first request expects id 1's
    // marker and the second expects id 2's marker, whatever the arrival order.
    match a {
        VfsResponseBody::Data(data) => assert_eq!(data, vec![1; 4]),
        other => panic!("unexpected first response: {other:?}"),
    }
    match b {
        VfsResponseBody::Data(data) => assert_eq!(data, vec![2; 4]),
        other => panic!("unexpected second response: {other:?}"),
    }
    handle.join().unwrap();
}

/// Sixteen requests at once through one client each get their own response.
#[valtron_test]
async fn ipc_concurrent_requests() {
    let tmp = TempDir::new("ipc_concurrent");
    for i in 0..16 {
        std::fs::write(tmp.path().join(format!("f{i}")), format!("content-{i}")).unwrap();
    }

    let config = test_config("concurrent");
    let daemon = Arc::new(VfsDaemon::new(Arc::new(
        NativeFs::new(tmp.path()).expect("create NativeFs"),
    )));
    let running = Arc::new(AtomicBool::new(true));
    let mut server = DaemonTransport::serve_with_config(
        config.clone(),
        Arc::new(foundation_core::valtron::MonotonicClock::new()),
    )
    .unwrap();
    let (server_daemon, server_running) = (Arc::clone(&daemon), Arc::clone(&running));
    let server_handle = std::thread::spawn(move || {
        let _ = run_daemon_loop(&mut server, &server_daemon, &server_running);
    });

    let transport = Arc::new(
        ClientTransport::connect_with_config(
            config,
            None,
            Arc::new(foundation_core::valtron::MonotonicClock::new()),
        )
        .unwrap(),
    );
    let client = Arc::new(VfsClient::new(transport).await);

    let futs: Vec<futures_lite::future::Boxed<(usize, Vec<u8>)>> = (0..16)
        .map(|i| {
            let client = Arc::clone(&client);
            async move {
                let data = client
                    .read_file_async(format!("/f{i}"))
                    .await
                    .expect("read file");
                (i, data)
            }
            .boxed()
        })
        .collect();

    for (i, data) in zip_all(futs).await {
        assert_eq!(data, format!("content-{i}").into_bytes());
    }

    running.store(false, Ordering::SeqCst);
    server_handle.join().unwrap();
}
