//! Regression test for the keychain native server (spec 70 feature 44).
//!
//! A route whose store does not answer in the first poll must still answer.
//! The old `Serve::serve` polled each handler once with a no-op waker and
//! panicked on `Poll::Pending`. The store here yields `Pending` once and wakes
//! the task for real, so only a handler run as `ConnectionResult::Async` with a
//! real waker can pass.

// The valtron stream chain (`execute(from_future(..))`) nests past the default
// trait-solver recursion limit (128); see `foundation_db/src/lib.rs`.
#![recursion_limit = "256"]

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use foundation_core::valtron::{
    collect_one, execute, from_future, valtron_test, BlockingFutureExt,
};
use foundation_db::core::storage_provider::QueryStore;
use foundation_db::{AsyncQueryStream, DataValue, StorageBackend, StorageProvider, StorageResult};
use foundation_http::native::server::{HttpServer, KeepAliveConfig, ServerConfig};
use foundation_http::ShutdownSignal;
use foundation_keychain::core::store::apply_schema;
use foundation_keychain::server::native::KeychainServer;
use foundation_keychain::KeychainContext;
use foundation_netio::shared::client::body_reader::try_collect_bytes;
use foundation_netio::shared::http::SimpleHeader;
use foundation_netio::{HttpClientBuilder, PreparedRequestBuilder};
use tempfile::TempDir;

/// Yields `Pending` once, after waking its own task.
struct YieldOnce(bool);

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            return Poll::Ready(());
        }
        self.0 = true;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

/// A store that waits one poll before every call, then asks the real store.
struct YieldingStore {
    inner: Arc<dyn QueryStore>,
    yields: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl QueryStore for YieldingStore {
    async fn query(&self, sql: &str, params: &[DataValue]) -> StorageResult<AsyncQueryStream> {
        self.yields.fetch_add(1, Ordering::SeqCst);
        YieldOnce(false).await;
        self.inner.query(sql, params).await
    }

    async fn execute(&self, sql: &str, params: &[DataValue]) -> StorageResult<u64> {
        self.yields.fetch_add(1, Ordering::SeqCst);
        YieldOnce(false).await;
        self.inner.execute(sql, params).await
    }

    async fn execute_batch(&self, sql: &str) -> StorageResult<()> {
        self.yields.fetch_add(1, Ordering::SeqCst);
        YieldOnce(false).await;
        self.inner.execute_batch(sql).await
    }
}

struct TestServer {
    addr: std::net::SocketAddr,
    shutdown: Arc<ShutdownSignal>,
    yields: Arc<AtomicUsize>,
    _dir: TempDir,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.turn_on();
    }
}

fn drive<T, F>(future: F) -> T
where
    T: Send + 'static,
    F: Future<Output = T> + Send + 'static,
{
    collect_one(execute(from_future(future), None).expect("execute")).expect("result")
}

fn start_server() -> TestServer {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("kc.db");
    let provider = BlockingFutureExt::block({
        let url = db_path.to_str().unwrap().to_string();
        async move { StorageProvider::new(StorageBackend::Turso { url }).await }
    })
    .expect("init turso");
    let real: Arc<dyn QueryStore> = Arc::new(provider);
    let d = Arc::clone(&real);
    drive(async move { apply_schema(d.as_ref()).await }).expect("apply schema");

    let yields = Arc::new(AtomicUsize::new(0));
    let db: Arc<dyn QueryStore> = Arc::new(YieldingStore {
        inner: real,
        yields: Arc::clone(&yields),
    });
    let app = KeychainServer::new(KeychainContext::new(
        db,
        foundation_core::valtron::Ctx::host(),
    ))
    .http_app();

    let shutdown = Arc::new(ShutdownSignal::new());
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let addr = listener.local_addr().expect("addr");
    let cfg = ServerConfig::defaults()
        .with_keep_alive(KeepAliveConfig::defaults().with_idle_timeout(Duration::from_secs(3)));
    let server = HttpServer::with_config(app, &format!("127.0.0.1:{}", addr.port()), cfg);
    let sc = shutdown.clone();
    std::thread::spawn(move || server.serve_with_listener(&listener, &sc));
    std::thread::sleep(Duration::from_millis(150));

    TestServer {
        addr,
        shutdown,
        yields,
        _dir: dir,
    }
}

#[valtron_test(threads = 8, timeout = 60000)]
fn keychain_handler_that_awaits_does_not_panic() {
    let server = start_server();
    let client = HttpClientBuilder::new()
        .connect_timeout(Duration::from_secs(5))
        .read_timeout(Duration::from_secs(20))
        .write_timeout(Duration::from_secs(5))
        .expect_continue(false)
        .build();
    let base = format!("http://127.0.0.1:{}", server.addr.port());

    let resp = client
        .send(
            PreparedRequestBuilder::post(&format!("{base}/identity/accounts/prelogin"))
                .unwrap()
                .header(SimpleHeader::CONTENT_TYPE, "application/json")
                .body_text(r#"{"email":"nobody@example.com"}"#.to_string())
                .build(),
        )
        .expect("the server must answer, not drop the connection");
    let (status, _, body) = resp.into_parts();
    let text = String::from_utf8_lossy(&try_collect_bytes(body).unwrap_or_default()).to_string();
    assert_eq!(status.into_usize(), 200, "prelogin answered: {text}");
    assert!(
        server.yields.load(Ordering::SeqCst) > 0,
        "the handler must have reached the yielding store"
    );
}
