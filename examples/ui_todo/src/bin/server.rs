//! Start the example on a loopback address.
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let _pool = foundation_core::valtron::initialize_pool(0, None);
    let bind = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:8180".to_owned());
    let module = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/web/ui_todo.wasm");
    let listener = std::net::TcpListener::bind(bind)?;
    println!("Open http://{}/", listener.local_addr()?);
    if !listener.local_addr()?.ip().is_loopback() {
        return Err("The single-user example requires a loopback address".into());
    }
    let app = if std::env::args().nth(2).as_deref() == Some("--data-only") {
        ui_todo::server::data_app()?
    } else {
        ui_todo::server::app(&module)?
    };
    app.server(&listener.local_addr()?.to_string())
        .serve_with_listener(
            &listener,
            &std::sync::Arc::new(foundation_http::ShutdownSignal::new()),
        );
    Ok(())
}
