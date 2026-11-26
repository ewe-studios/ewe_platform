//! Start a full Todo server with a bounded history for replay tests.
use foundation_http::ShutdownSignal;
use foundation_ui_wire::writer::WriterConfig;
use foundation_wasm_ui::server::{BacklogConfig, LiveChannel};
use std::{net::TcpListener, sync::Arc};
use ui_todo::{server::app_with_store, TodoStore};

#[foundation_core::valtron::valtron(seed = 0)]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut args = std::env::args().skip(1);
    let frames = args
        .next()
        .ok_or("Pass the number of replay frames")?
        .parse()?;
    if args.next().is_some() {
        return Err("Expected only the number of replay frames".into());
    }
    let channel = LiveChannel::new(
        1,
        BacklogConfig::default().max_frames(frames),
        WriterConfig::default(),
    );
    let server = app_with_store(TodoStore::default(), channel)?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    println!("Open http://{address}/");
    server
        .server(&address.to_string())
        .serve_with_listener(&listener, &Arc::new(ShutdownSignal::new()));
    Ok(())
}
