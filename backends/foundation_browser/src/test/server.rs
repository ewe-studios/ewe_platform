//! # TestServer (spec-43 phase-1 §6 + feature 01) — on `foundation_http`
//!
//! WHY: The page under test must be served somewhere the browser can load it,
//! torn down with the test. We dogfood our OWN server (`foundation_http`) — its
//! `Serve` handlers OWN the connection, which the channel-A DOM-op stream route
//! (`ConnectionResult::Take`) needs. Response headers + the wire encoding are
//! declared up front (the browser runtime negotiates off them).
//!
//! WHAT: [`PageSource`] (inline HTML or a file), [`Encoding`] (json/columnar),
//! [`TestServer`] — boot an `HttpApp` serving the page at `/`, a static dir, and
//! the channel-A stream at [`STREAM_PATH`](crate::test::stream::STREAM_PATH);
//! `broadcaster()` pushes frames; RAII `Drop` stops the server.
//!
//! HOW: A `PageHandler` (`Serve`) reads the page + headers from the `ContextBag`
//! and writes them; `StaticFileHandler` serves a directory; [`StreamHandler`]
//! upgrades the owned connection to SSE and registers it with the
//! [`Broadcaster`]. Served via `serve_with_listener` on an ephemeral
//! `TcpListener` on a background thread, stopped by an `OnSignal`.

use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use foundation_core::io::ioutils::SharedByteBufferStream;
use foundation_http::native::handlers::static_asset::StaticAssetHandler;
use foundation_http::native::handlers::static_file::StaticFileHandler;
use foundation_http::native::server::ServerConfig;
use foundation_http::shared::app::HttpApp;
use foundation_http::shared::context::ContextBag;
use foundation_http::shared::serve::{ConnectionResult, Serve, ServeFactory};
use foundation_http::OnSignal;
use foundation_netio::netcap::ssl::SSLAcceptor;
use foundation_netio::netcap::RawStream;
use foundation_netio::simple_http::shared::{
    Http11, RenderHttp, SendSafeBody, SimpleHeader, SimpleIncomingRequest, SimpleOutgoingResponse,
    Status,
};
use zeroize::Zeroizing;

use foundation_wasm_ui::server::{BroadcastSink, BroadcastTx, Broadcaster};

use crate::error::{BrowserError, Result};
use crate::test::stream::{StreamHandler, STREAM_PATH};

/// The `foundation_wasm_ui` browser runtime (via its `embedded-js` feature) so
/// `<mount-stream>` works without a build step. Served at [`RUNTIME_PATH`].
const RUNTIME_JS: &str = foundation_wasm_ui::embedded::FOUNDATION_WASM_UI_JS;

/// The bundled Apache Arrow JS library — the Arrow IPC (wire v2) reader the page
/// registers as `ProtocolHandler.arrowIpcReader`. Served at [`ARROW_LIB_PATH`].
const APACHE_ARROW_JS: &str = foundation_wasm_ui::embedded::APACHE_ARROW_JS;

/// Where the embedded runtime is served (the page imports it).
pub const RUNTIME_PATH: &str = "/__primal/foundation-wasm-ui.js";

/// Where the bundled Apache Arrow library is served (the page loads it for the
/// Arrow IPC protocol).
pub const ARROW_LIB_PATH: &str = "/__primal/apache-arrow.js";

/// A self-signed `localhost`/`127.0.0.1` dev certificate, for the `https` mode.
/// Regenerate with `mise run test:cert:regen`. The browser is launched trusting
/// it (`--ignore-certificate-errors`), so it only needs to be a valid self-signed
/// pair — never use it outside tests.
const DEV_CERT_PEM: &[u8] = include_bytes!("fixtures/cert.pem");
const DEV_KEY_PEM: &[u8] = include_bytes!("fixtures/key.pem");

/// The wire encoding the App streams in (and that the browser runtime decodes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Encoding {
    /// UTF-8 JSON frames — debuggable; the recommended first cut over SSE.
    #[default]
    Json,
    /// Compact columnar binary frames (our owned wire, protocol 1 v1).
    Columnar,
    /// Apache Arrow IPC frames (protocol 1 v2) — ecosystem interop.
    Arrow,
}

impl Encoding {
    /// The token announced via the `X-Primal-Encoding` header.
    #[must_use]
    pub fn token(self) -> &'static str {
        match self {
            Encoding::Json => "json",
            Encoding::Columnar => "columnar",
            Encoding::Arrow => "arrow",
        }
    }
}

impl core::str::FromStr for Encoding {
    type Err = String;
    fn from_str(s: &str) -> core::result::Result<Self, Self::Err> {
        match s {
            "json" | "Json" | "JSON" => Ok(Encoding::Json),
            "columnar" | "Columnar" => Ok(Encoding::Columnar),
            "arrow" | "Arrow" => Ok(Encoding::Arrow),
            other => Err(format!("unknown encoding `{other}` (json|columnar|arrow)")),
        }
    }
}

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

/// The mountable page + response headers, parked in the `ContextBag`.
struct PageState {
    page: Mutex<PageSource>,
    headers: Vec<(String, String)>,
}

/// Serves the current [`PageSource`] with the configured headers.
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
        let Some(state) = bag.get::<SharedState>() else {
            return ConnectionResult::Close(None);
        };
        let html = state.page.lock().expect("page poisoned").render();
        let mut builder = SimpleOutgoingResponse::builder()
            .with_status(Status::OK)
            .add_header(SimpleHeader::CONTENT_TYPE, "text/html; charset=utf-8");
        for (name, value) in &state.headers {
            builder = builder.add_header(SimpleHeader::custom(name), value);
        }
        let Ok(response) = builder.with_body(SendSafeBody::Text(html)).build() else {
            return ConnectionResult::Close(None);
        };
        Http11::response(response)
            .http_render_to_writer(&mut conn)
            .map_or(ConnectionResult::Close(None), |_| ConnectionResult::Keep)
    }
}

/// A background `foundation_http` server serving the page + channel-A stream.
pub struct TestServer {
    addr: std::net::SocketAddr,
    https: bool,
    page: Arc<PageState>,
    broadcaster: Broadcaster,
    shutdown: Arc<OnSignal>,
    handle: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Start a server bound to `bind` serving `page` at `/`, static directory
    /// mounts, and the channel-A SSE stream. `headers` are attached to the page
    /// response; `encoding` adds the `X-Primal-Encoding` announce header. When
    /// `https` is true the server speaks TLS with the bundled localhost dev cert.
    ///
    /// # Errors
    /// [`BrowserError::Io`] if the listener can't bind; [`BrowserError::Launch`]
    /// if the TLS acceptor can't be built or the server thread can't spawn.
    pub fn start(
        bind: &str,
        page: PageSource,
        statics: &[StaticMount],
        mut headers: Vec<(String, String)>,
        encoding: Encoding,
        https: bool,
    ) -> Result<Self> {
        let listener = TcpListener::bind(bind)?;
        let addr = listener.local_addr()?;

        headers.push(("X-Primal-Encoding".into(), encoding.token().into()));
        let page_state = Arc::new(PageState { page: Mutex::new(page), headers });
        let broadcaster = Broadcaster::new();

        let mut app = HttpApp::new_serve();
        // PageState + Broadcaster are read out of the bag by the handlers; the
        // Arc/clone share the same state the TestServer holds.
        app.context().store(SharedState(page_state.clone()));
        app.context().store(broadcaster.clone());
        app.route_any::<PageHandler>("/");
        app.route_any::<StreamHandler>(STREAM_PATH);
        // Embedded JS assets via foundation_http's StaticAssetHandler — supply the
        // bytes + content type, register on a path. No per-asset Serve impl.
        let js_ct = "text/javascript; charset=utf-8";
        let runtime: Arc<dyn Serve> = Arc::new(StaticAssetHandler::new(RUNTIME_JS.as_bytes(), js_ct));
        let arrow_lib: Arc<dyn Serve> = Arc::new(StaticAssetHandler::new(APACHE_ARROW_JS.as_bytes(), js_ct));
        app.router.add_route_any(RUNTIME_PATH, &runtime);
        app.router.add_route_any(ARROW_LIB_PATH, &arrow_lib);
        for sm in statics {
            let handler: Arc<dyn Serve> = Arc::new(StaticFileHandler::new(sm.dir.clone()));
            app.router.add_route_any(&format!("{}/*", sm.mount.trim_end_matches('/')), &handler);
        }

        let shutdown = Arc::new(OnSignal::new());
        let addr_str = addr.to_string();
        let handle = {
            let shutdown = shutdown.clone();
            let builder = std::thread::Builder::new().name("test-server".into());
            if https {
                // TLS over the same caller-bound listener (ephemeral port already known).
                let acceptor = SSLAcceptor::from_pem(
                    DEV_CERT_PEM.to_vec(),
                    Zeroizing::new(DEV_KEY_PEM.to_vec()),
                )
                .map_err(|e| BrowserError::Launch(format!("dev TLS acceptor: {e}")))?;
                let server =
                    app.server_with_config(&addr_str, ServerConfig::default().with_tls(Arc::new(acceptor)));
                builder
                    .spawn(move || server.serve_tls_with_listener(&listener, &shutdown))
                    .map_err(|e| BrowserError::Launch(e.to_string()))?
            } else {
                let server = app.server(&addr_str);
                builder
                    .spawn(move || server.serve_with_listener(&listener, &shutdown))
                    .map_err(|e| BrowserError::Launch(e.to_string()))?
            }
        };

        Ok(Self { addr, https, page: page_state, broadcaster, shutdown, handle: Some(handle) })
    }

    /// The base URL (e.g. `http://127.0.0.1:54321/`, or `https://…` in `https` mode).
    #[must_use]
    pub fn url(&self) -> String {
        let scheme = if self.https { "https" } else { "http" };
        format!("{scheme}://{}/", self.addr)
    }

    /// Replace the served page (takes effect on the next navigation).
    pub fn mount(&self, page: PageSource) {
        *self.page.page.lock().expect("page poisoned") = page;
    }

    /// A `Send` handle for streaming protocol frames to the connected browser
    /// (the App's broadcast sink pushes through this).
    #[must_use]
    pub fn broadcaster(&self) -> BroadcastTx {
        self.broadcaster.sender()
    }

    /// Convenience: push one raw frame to the browser (used by transport tests).
    pub fn push_frame(&self, frame: &[u8]) {
        self.broadcaster.sender().send(frame);
    }

    /// Convenience: push one HTML markup frame (the HTML protocol) — delivered as
    /// a plain SSE `data:` line the runtime routes through `routeHtml`.
    pub fn push_html(&self, markup: &str) {
        self.broadcaster.sender().send_text(markup);
    }

    /// Number of frames the App has streamed so far (diagnostics).
    #[must_use]
    pub fn frame_count(&self) -> usize {
        self.broadcaster.frame_count()
    }

    /// Number of registered SSE connections (diagnostics).
    #[must_use]
    pub fn conn_count(&self) -> usize {
        self.broadcaster.conn_count()
    }

    /// The App's protocol sink (Mode 1): wire it into `App::with_protocol(...)`,
    /// then `stabilize()` streams encoded frames to the browser's `<mount-stream>`.
    /// Columnar by default (matches `<mount-stream protocol="arrow">`).
    #[must_use]
    pub fn broadcast_sink(&self) -> BroadcastSink {
        BroadcastSink::new(self.broadcaster.sender())
    }
}

/// Newtype so `PageState` (which itself isn't the bag key) stores cleanly.
struct SharedState(Arc<PageState>);

// The bag stores `SharedState`; `PageHandler` looks up `PageState` via it.
impl core::ops::Deref for SharedState {
    type Target = PageState;
    fn deref(&self) -> &PageState {
        &self.0
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // Unblock the streaming handlers first (drop their senders → recv errors
        // → the handlers return → their valtron workers are freed), THEN stop
        // the accept loop and join.
        self.broadcaster.close();
        self.shutdown.turn_on();
        if let Some(handle) = self.handle.take() {
            if handle.join().is_err() {
                tracing::warn!("test server thread panicked during shutdown");
            }
        }
    }
}
