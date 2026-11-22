use std::sync::Arc;
use foundation_core::valtron::valtron;
use foundation_http::shared::app::HttpApp;
use foundation_http::ShutdownSignal;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[cfg(not(debug_assertions))]
#[derive(foundation_macros::EmbedDirectoryAs)]
#[source = "$EWE_STAGED_RESOURCES/."]
#[always_embed]
struct Assets;

#[valtron(seed = 10, threads = 10)]
fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let mut app = HttpApp::new_serve();
    #[cfg(debug_assertions)]
    let public = Some(foundation_resources::server::sibling_public(env!("CARGO_PKG_NAME")).expect("locate public directory"));
    #[cfg(not(debug_assertions))]
    let public: Option<std::path::PathBuf> = None;
    #[cfg(debug_assertions)]
    let embedded = |_: &str| None;
    #[cfg(not(debug_assertions))]
    let embedded = |name: &str| {
        use foundation_nostd::embeddable::DirectoryData;
        Assets.read_utf8_for(name)
    };
    foundation_resources::server::register(
        &mut app.router, include_str!(env!("EWE_RESOURCE_NAMES")), public.as_deref(), Some("index.html"), embedded,
    ).expect("register application resources");

    let server = app.server("127.0.0.1:3000");
    let shutdown = Arc::new(ShutdownSignal::new());
    server.serve(&shutdown);
}
