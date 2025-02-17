use core::str;
use core::time;

use axum::{
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use http::StatusCode;
use rust_embed::Embed;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use ewe_devserver::{
    types::{Http1, ProxyRemoteConfig},
    HttpDevService, ProjectDefinition, ProxyType, VecStringExt,
};
use std::collections::HashMap;
use tokio::sync::broadcast;

// use crate::jsdom::Packages;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Embed)]
#[folder = "src/sandbox_app/public/"]
#[prefix = "public/"]
struct Public;

async fn handler() -> Response {
    match Public::get("public/index.html") {
        Some(html_data) => {
            let content = String::from_utf8(html_data.data.to_vec()).expect("should generate str");
            Html(content).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 NOT FOUND").into_response(),
    }
}

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(
        clap::Command::new("sandbox_app")
            .about("runs a local server for running our sandbox applications and demos")
            .arg(
                clap::Arg::new("source_addr")
                    .long("source_addr")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(String))
                    .default_value("0.0.0.0"),
            )
            .arg(
                clap::Arg::new("source_port")
                    .long("source_port")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(usize))
                    .default_value("3050"),
            ),
    )
}

pub async fn run(args: &clap::ArgMatches) -> std::result::Result<(), BoxedError> {
    let source_addr = args
        .get_one::<String>("source_addr")
        .expect("should have source address");

    let source_port = args
        .get_one::<usize>("source_port")
        .expect("should have source port");

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let app = Router::new().route("/", get(handler));

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", source_addr, source_port))
        .await
        .unwrap();

    ewe_trace::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .await
        .map_err(|err| Box::new(err))?;

    Ok(())
}
