//! Start the Todo service with a loopback listener and a persistent database.
use foundation_ui_wire::writer::WriterConfig;
use foundation_wasm_ui::server::{BacklogConfig, LiveChannel};
use std::path::PathBuf;

#[foundation_core::valtron::valtron(seed = 0)]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new(
                    "foundation_http::shared::middleware::logger=info",
                )
            }),
        )
        .try_init()?;
    let mut args = std::env::args_os().skip(1).peekable();
    let address = args
        .next_if(|value| value != "--store")
        .unwrap_or_else(|| "127.0.0.1:8180".into());
    let path = match args.next() {
        Some(flag) if flag == "--store" => {
            PathBuf::from(args.next().ok_or("Pass a database path after --store")?)
        }
        Some(_) => return Err("Use ui_todo [ADDRESS] [--store PATH]".into()),
        None => std::env::var_os("EWE_TODO_STORE")
            .map_or_else(|| PathBuf::from("todo.sqlite3"), PathBuf::from),
    };
    if args.next().is_some() {
        return Err("Unexpected server argument".into());
    }
    let listener = std::net::TcpListener::bind(address.to_str().ok_or("Invalid listen address")?)?;
    let address = listener.local_addr()?;
    if !address.ip().is_loopback() {
        return Err("The single-user example requires a loopback address".into());
    }
    let ctx = foundation_context::Ctx::try_host()?;
    foundation_core::env::install(&ctx, foundation_core::env::system)?;
    let storage_ctx = ctx.clone();
    let store = foundation_core::valtron::block_on_future(async move {
        ui_todo::TodoStore::open(&storage_ctx, &path).await
    })?;
    let channel = LiveChannel::resume(
        1,
        store.position(),
        BacklogConfig::default(),
        WriterConfig::default(),
    );
    let app = ui_todo::server::app_with_context(store, channel, ctx)?;
    println!("Open http://{address}/");
    app.server(&address.to_string()).serve_with_listener(
        &listener,
        &std::sync::Arc::new(foundation_http::ShutdownSignal::new()),
    );
    Ok(())
}
