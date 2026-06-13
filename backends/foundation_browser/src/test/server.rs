//! # TestServer (spec-43 phase-1 §6) — on `foundation_http`
//!
//! WHY: The page under test must be served somewhere the browser can load it,
//! torn down with the test. We dogfood our OWN server (`foundation_http`) rather
//! than a raw socket — its `Serve` handlers OWN the connection, which is what the
//! later channel-A DOM-op stream route (`ConnectionResult::Take`) needs.
//!
//! WHAT: [`PageSource`] (inline HTML or an on-disk file), [`TestServer`] — boot
//! an `HttpApp` serving the page at `/` (+ an optional [`StaticFileHandler`]
//! directory for assets/wasm), RAII `Drop` (signal shutdown + join).
//!
//! HOW: A `PageHandler` (`Serve`) reads the mountable page from the `ContextBag`
//! and writes it; `StaticFileHandler::new(dir)` (reused from `foundation_http`)
//! serves a directory under a mount prefix. Served via `serve_with_listener` on
//! an ephemeral `TcpListener` on a background thread, stopped by an `OnSignal`.

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::native::handlers::static_file::StaticFileHandler;
use foundation_http::native::server::HttpServer;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{respond, ConnectionResult, Serve, ServeFactory};
use foundation_http::OnSignal;
use foundation_netio::netcap::RawStream;
use foundation_netio::simple_http::shared::SimpleIncomingRequest;

use crate::error::{BrowserError, Result};

/// What the `/` route renders: inline HTML or a file read at request time.
#[derive(Clone)]
pub enum PageSource {
    /// An inline HTML document.
    Html(String),
    /// An HTML file read from disk on each request.
    File(PathBuf),
}

impl PageSource {
    fn render(&self) -> String {
        match self {
            PageSource::Html(s) => s.clone(),
            PageSource::File(path) => std::fs::read_to_string(path)
                .unwrap_or_else(|e| format!("<!-- wasm_ui_server: failed to read {}: {e} -->", path.display())),
        }
    }
}

/// A static-directory mount: files under `dir` are served at `mount`/`*`.
#[derive(Clone)]
pub struct StaticMount {
    /// URL prefix (e.g. `"/assets"`).
    pub mount: String,
    /// Filesystem root.
    pub dir: PathBuf,
}

/// The mountable page, parked in the `ContextBag` for `PageHandler` to read.
struct PageBody(Arc<Mutex<PageSource>>);

/// Serves the current [`PageSource`] at its route.
struct PageHandler;

impl ServeFactory for PageHandler {
    fn create(_bag: &ContextBag) -> Self {
        PageHandler
    }
}

impl Serve for PageHandler {
    fn serve(
        &self,
        bag: Arc<ContextBag>,
        _req: SimpleIncomingRequest,
        mut conn: SharedByteBufferStream<RawStream>,
    ) -> ConnectionResult {
        let html = bag
            .get::<PageBody>()
            .map(|b| b.0.lock().expect("page body poisoned").render())
            .unwrap_or_default();
        respond::html(&mut conn, 200, &html)
            .map_or(ConnectionResult::Close(None), |()| ConnectionResult::Keep)
    }
}

/// A background `foundation_http` server serving the page under test.
pub struct TestServer {
    addr: std::net::SocketAddr,
    page: Arc<Mutex<PageSource>>,
    shutdown: Arc<OnSignal>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Start a server bound to `bind` serving `page` at `/`, plus any static
    /// directory mounts (assets/wasm).
    ///
    /// # Errors
    /// [`BrowserError::Io`] if the listener can't bind.
    pub fn start(bind: &str, page: PageSource, statics: &[StaticMount]) -> Result<Self> {
        let listener = TcpListener::bind(bind)?;
        let addr = listener.local_addr()?;
        let page = Arc::new(Mutex::new(page));

        let mut app = HttpApp::new_serve();
        app.context().store(PageBody(page.clone()));
        app.route_any::<PageHandler>("/");
        for sm in statics {
            let handler: Arc<dyn Serve> = Arc::new(StaticFileHandler::new(sm.dir.clone()));
            app.router.add_route_any(&format!("{}/*", sm.mount.trim_end_matches('/')), &handler);
        }

        let shutdown = Arc::new(OnSignal::new());
        let server: HttpServer = app.server(&addr.to_string());
        let handle = {
            let shutdown = shutdown.clone();
            std::thread::Builder::new()
                .name("test-server".into())
                .spawn(move || server.serve_with_listener(&listener, &shutdown))
                .map_err(|e| BrowserError::Launch(e.to_string()))?
        };

        Ok(Self { addr, page, shutdown, handle: Some(handle) })
    }

    /// The base URL (e.g. `http://127.0.0.1:54321/`).
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://{}/", self.addr)
    }

    /// Replace the served page (takes effect on the next navigation).
    pub fn mount(&self, page: PageSource) {
        *self.page.lock().expect("page poisoned") = page;
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.shutdown.turn_on();
        if let Some(handle) = self.handle.take() {
            if handle.join().is_err() {
                tracing::warn!("test server thread panicked during shutdown");
            }
        }
    }
}
