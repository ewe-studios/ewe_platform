//! Start the example on a loopback address.
#[path = "server/backup.rs"]
mod backup;

use std::io;
use std::path::Path;

#[foundation_core::valtron::valtron(seed = 0)]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut args = std::env::args_os().skip(1);
    let command = args.next();
    let limit = foundation_ui_wire::WireLimits::default().frame_bytes() as u64;
    if command.as_deref() == Some(std::ffi::OsStr::new("--convert-backup")) {
        let input = args
            .next()
            .ok_or("Pass the numeric backup and a new output path")?;
        let output = args
            .next()
            .ok_or("Pass a new output path for the UUID backup")?;
        if args.next().is_some() {
            return Err("Unexpected argument after the output path".into());
        }
        let ctx = foundation_context::Ctx::try_host()?;
        foundation_core::env::install(&ctx, foundation_core::env::system)?;
        return backup::convert(
            &ctx,
            Path::new(&input),
            Path::new(&output),
            limit,
            io::stdout().lock(),
        );
    }
    let bind = command
        .as_deref()
        .unwrap_or(std::ffi::OsStr::new("127.0.0.1:8180"))
        .to_str()
        .ok_or("The listen address must be valid text")?;
    let listener = std::net::TcpListener::bind(bind)?;
    if !listener.local_addr()?.ip().is_loopback() {
        return Err("The single-user example requires a loopback address".into());
    }
    let mode = args.next();
    let mode = mode.as_deref().and_then(std::ffi::OsStr::to_str);
    let app = if mode == Some("--data-only") {
        ui_todo::server::data_app()?
    } else {
        if mode == Some("--restore") {
            let path = args.next().ok_or("Pass the todo backup path")?;
            if args.next().is_some() {
                return Err("Unexpected argument after the restore path".into());
            }
            ui_todo::server::app_with_todos(backup::restore(Path::new(&path), limit)?)?
        } else if let Some(history) = std::env::var_os("EWE_TODO_HISTORY") {
            use foundation_wasm_ui::server::{BacklogConfig, LiveChannel};
            let limit = history
                .to_str()
                .ok_or("Expected a replay history length")?
                .parse()?;
            let channel = LiveChannel::new(
                1,
                BacklogConfig::default().max_frames(limit),
                foundation_ui_wire::writer::WriterConfig::default(),
            );
            ui_todo::server::app_with_channel(channel)?
        } else {
            ui_todo::server::app()?
        }
    };
    println!("Open http://{}/", listener.local_addr()?);
    app.server(&listener.local_addr()?.to_string())
        .serve_with_listener(
            &listener,
            &std::sync::Arc::new(foundation_http::ShutdownSignal::new()),
        );
    Ok(())
}
